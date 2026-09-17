use crate::client::{ApiClient, StreamEvent};
use crate::config::PermissionMode;
use crate::tools::{all_tools, execute_tool};
use crate::types::{ChatMessage, FunctionCall, ToolCall};
use colored::*;
use std::collections::HashMap;
use std::io::{self, Write};

pub struct AgentRunner {
    pub client: ApiClient,
    pub model: String,
    pub temperature: f32,
    pub tools_enabled: bool,
    pub permission_mode: PermissionMode,
    pub max_steps: usize,
}

impl AgentRunner {
    pub fn new(client: ApiClient, model: String, temperature: f32) -> Self {
        Self {
            client,
            model,
            temperature,
            tools_enabled: true,
            permission_mode: PermissionMode::AllowAll,
            max_steps: 25,
        }
    }

    pub async fn execute_turn(&mut self, messages: &mut Vec<ChatMessage>) -> Result<(), String> {
        let tools = if self.tools_enabled {
            Some(all_tools())
        } else {
            None
        };

        let mut step = 0;
        while step < self.max_steps {
            step += 1;

            let mut rx = self
                .client
                .stream_chat(
                    messages.clone(),
                    &self.model,
                    Some(self.temperature),
                    tools.clone(),
                )
                .await?;

            let mut full_content = String::new();
            let mut full_reasoning = String::new();
            let mut is_reasoning = false;
            let mut tool_calls_map: HashMap<usize, ToolCall> = HashMap::new();

            while let Some(event) = rx.recv().await {
                match event {
                    StreamEvent::Reasoning(r) => {
                        if !is_reasoning {
                            is_reasoning = true;
                            println!("\n{}", "╭─ [思考过程] ──────────────────────────".dimmed());
                        }
                        print!("{}", r.dimmed());
                        let _ = io::stdout().flush();
                        full_reasoning.push_str(&r);
                    }
                    StreamEvent::Content(c) => {
                        if is_reasoning {
                            is_reasoning = false;
                            println!("\n{}", "╰───────────────────────────────────────".dimmed());
                        }
                        print!("{}", c);
                        let _ = io::stdout().flush();
                        full_content.push_str(&c);
                    }
                    StreamEvent::ToolCallDelta {
                        index,
                        id,
                        name,
                        arguments,
                    } => {
                        if is_reasoning {
                            is_reasoning = false;
                            println!("\n{}", "╰───────────────────────────────────────".dimmed());
                        }
                        let entry = tool_calls_map.entry(index).or_insert_with(|| ToolCall {
                            id: String::new(),
                            tool_type: "function".to_string(),
                            function: FunctionCall {
                                name: String::new(),
                                arguments: String::new(),
                            },
                        });

                        if let Some(call_id) = id {
                            entry.id = call_id;
                        }
                        if let Some(fn_name) = name {
                            entry.function.name.push_str(&fn_name);
                        }
                        if let Some(fn_args) = arguments {
                            entry.function.arguments.push_str(&fn_args);
                        }
                    }
                    StreamEvent::Done => {
                        if is_reasoning {
                            println!("\n{}", "╰───────────────────────────────────────".dimmed());
                        }
                        break;
                    }
                    StreamEvent::Error(err) => {
                        if is_reasoning {
                            println!("\n{}", "╰───────────────────────────────────────".dimmed());
                        }
                        return Err(err);
                    }
                }
            }

            // 如果没有工具调用，说明本次回复完成
            if tool_calls_map.is_empty() {
                let content_opt = if full_content.is_empty() {
                    None
                } else {
                    Some(full_content)
                };
                let reasoning_opt = if full_reasoning.is_empty() {
                    None
                } else {
                    Some(full_reasoning)
                };
                messages.push(ChatMessage::assistant(content_opt, reasoning_opt, None));
                println!();
                return Ok(());
            }

            // 有工具调用，先按 index 排序转换为列表
            let mut sorted_indices: Vec<usize> = tool_calls_map.keys().cloned().collect();
            sorted_indices.sort_unstable();
            let tool_calls: Vec<ToolCall> = sorted_indices
                .into_iter()
                .filter_map(|idx| tool_calls_map.remove(&idx))
                .collect();

            // 存入 assistant 消息
            let content_opt = if full_content.is_empty() {
                None
            } else {
                Some(full_content)
            };
            let reasoning_opt = if full_reasoning.is_empty() {
                None
            } else {
                Some(full_reasoning)
            };
            messages.push(ChatMessage::assistant(
                content_opt,
                reasoning_opt,
                Some(tool_calls.clone()),
            ));

            println!();

            // 逐个执行工具并存入 tool 角色消息
            for call in tool_calls {
                let tool_name = &call.function.name;
                let preview = format_tool_preview(tool_name, &call.function.arguments);

                if self.permission_mode == PermissionMode::Ask {
                    println!(
                        "{} {} ➜ {}",
                        "⚡ [Agent 申请执行工具]".yellow().bold(),
                        tool_name.cyan().bold(),
                        preview.white()
                    );
                    print!(
                        "{} 是否允许执行此工具? [y: 允许 / n: 拒绝 / a: 允许后续所有]: ",
                        "?".yellow().bold()
                    );
                    let _ = io::stdout().flush();

                    let mut user_input = String::new();
                    let allowed = if io::stdin().read_line(&mut user_input).is_ok() {
                        let ans = user_input.trim().to_lowercase();
                        if ans == "a" || ans == "all" || ans == "允许所有" || ans == "始终允许" {
                            self.permission_mode = PermissionMode::AllowAll;
                            println!("  {}", "✔ 已切换为允许所有工具执行 (Allow All)。".green());
                            true
                        } else if ans == "y" || ans == "yes" || ans == "是" || ans.is_empty() {
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    if !allowed {
                        println!("  {}", "✖ 已拒绝执行该工具。".red().bold());
                        let denied_msg = format!("Error: User denied permission to execute tool '{}'. Please adjust your approach or ask the user for further instructions.", tool_name);
                        messages.push(ChatMessage::tool(call.id, denied_msg));
                        continue;
                    }
                } else {
                    println!(
                        "{} {} ➜ {}",
                        "⚡ [Agent 工具调用]".yellow().bold(),
                        tool_name.cyan().bold(),
                        preview.dimmed()
                    );
                }

                let start_time = std::time::Instant::now();
                let output = execute_tool(tool_name, &call.function.arguments).await;
                let elapsed = start_time.elapsed().as_millis();

                let line_count = output.lines().count();
                println!(
                    "  {} {} (耗时 {}ms, 共 {} 行)",
                    "└─".dimmed(),
                    "执行完成".green(),
                    elapsed,
                    line_count
                );

                messages.push(ChatMessage::tool(call.id, output));
            }

            // 下一轮模型会接收工具执行结果并继续推理
        }

        Err("Agent 达到最大执行步骤上限 (25 轮) 自动终止".to_string())
    }
}

fn format_tool_preview(tool_name: &str, args_json: &str) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(args_json) {
        match tool_name {
            "bash" => {
                if let Some(cmd) = v.get("command").and_then(|c| c.as_str()) {
                    return format!("执行终端命令: {}", cmd);
                }
            }
            "write_file" => {
                if let Some(path) = v.get("path").and_then(|p| p.as_str()) {
                    let bytes = v.get("content").and_then(|c| c.as_str()).map(|c| c.len()).unwrap_or(0);
                    return format!("写入文件: {} ({} 字节)", path, bytes);
                }
            }
            "replace_in_file" => {
                if let Some(path) = v.get("path").and_then(|p| p.as_str()) {
                    return format!("替换文件内容: {}", path);
                }
            }
            "read_file" => {
                if let Some(path) = v.get("path").and_then(|p| p.as_str()) {
                    return format!("读取文件: {}", path);
                }
            }
            "list_dir" => {
                let path = v.get("path").and_then(|p| p.as_str()).unwrap_or(".");
                return format!("浏览目录: {}", path);
            }
            "search_code" => {
                if let Some(q) = v.get("query").and_then(|q| q.as_str()) {
                    return format!("搜索代码: \"{}\"", q);
                }
            }
            "load_skill" => {
                if let Some(name) = v.get("name").and_then(|n| n.as_str()) {
                    return format!("加载技能: {}", name);
                }
            }
            "search_skills" => {
                if let Some(q) = v.get("query").and_then(|q| q.as_str()) {
                    return format!("检索技能库: \"{}\"", q);
                }
            }
            _ => {}
        }
    }
    if args_json.len() > 100 {
        format!("{}...", &args_json[..100])
    } else {
        args_json.to_string()
    }
}
