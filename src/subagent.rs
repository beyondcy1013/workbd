use crate::client::ApiClient;
use crate::tools::execute_tool;
use crate::types::{ChatMessage, FunctionCall, FunctionDefinition, ToolCall, ToolDefinition};
use colored::*;
use serde_json::json;
use std::collections::HashMap;

/// 只读工具清单（专供 Explorer / Reviewer 子智能体）
pub fn readonly_tools() -> Vec<ToolDefinition> {
    crate::tools::all_tools()
        .into_iter()
        .filter(|t| {
            matches!(
                t.function.name.as_str(),
                "read_file" | "list_dir" | "search_code" | "load_skill" | "search_skills"
            )
        })
        .collect()
}

pub fn delegate_tool_definition() -> ToolDefinition {
    ToolDefinition {
        tool_type: "function".to_string(),
        function: FunctionDefinition {
            name: "delegate_task".to_string(),
            description: "Delegate a sub-task or codebase exploration to a specialized subagent with an isolated context. Roles available: 'explorer' (read-only code survey), 'reviewer' (code audit & verification), 'general' (isolated execution).".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "role": {
                        "type": "string",
                        "enum": ["explorer", "reviewer", "general"],
                        "description": "Subagent specialized role ('explorer', 'reviewer', or 'general')."
                    },
                    "prompt": {
                        "type": "string",
                        "description": "Specific task instructions and questions for the subagent to answer."
                    }
                },
                "required": ["role", "prompt"]
            }),
        },
    }
}

pub struct SubagentRunner {
    client: ApiClient,
    model: String,
    role: String,
    system_prompt: String,
    max_steps: usize,
}

impl SubagentRunner {
    pub fn new(client: ApiClient, model: String, role: &str) -> Self {
        let (role_name, prompt) = match role.to_lowercase().as_str() {
            "explorer" | "research" | "researcher" => (
                "Explorer",
                "You are an expert read-only code exploration subagent. Survey files, search code patterns, and return a clear, concise distilled analysis. Do not hallucinate. Do not attempt destructive modifications."
            ),
            "reviewer" | "review" => (
                "Reviewer",
                "You are a strict code review subagent. Inspect the targeted files or diffs, detect bugs, edge cases, and report actionable feedback."
            ),
            _ => (
                "General",
                "You are an isolated helper subagent tasked with solving a specific sub-problem. Provide a direct and complete summary when done."
            ),
        };

        Self {
            client,
            model,
            role: role_name.to_string(),
            system_prompt: prompt.to_string(),
            max_steps: 10,
        }
    }

    pub async fn run_task(&self, task_prompt: &str) -> Result<String, String> {
        println!(
            "  {} 正在委派子智能体 [{}] 启动独立任务...",
            "↳ 🤖".cyan().bold(),
            self.role.yellow().bold()
        );

        let is_readonly = self.role == "Explorer" || self.role == "Reviewer";
        let tools = if is_readonly {
            readonly_tools()
        } else {
            crate::tools::all_tools()
        };

        let mut messages = vec![
            ChatMessage::system(&self.system_prompt),
            ChatMessage::user(task_prompt),
        ];

        let mut step = 0;
        while step < self.max_steps {
            step += 1;

            let mut rx = self
                .client
                .stream_chat(messages.clone(), &self.model, Some(0.3), Some(tools.clone()))
                .await?;

            let mut full_content = String::new();
            let mut tool_calls_map: HashMap<usize, ToolCall> = HashMap::new();

            while let Some(event) = rx.recv().await {
                match event {
                    crate::client::StreamEvent::Content(c) => {
                        full_content.push_str(&c);
                    }
                    crate::client::StreamEvent::ToolCallDelta {
                        index,
                        id,
                        name,
                        arguments,
                    } => {
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
                    crate::client::StreamEvent::Done => break,
                    crate::client::StreamEvent::Error(e) => return Err(e),
                    _ => {}
                }
            }

            if tool_calls_map.is_empty() {
                println!(
                    "  {} 子智能体 [{}] 任务完成并返回结果。",
                    "✔ 🤖".green().bold(),
                    self.role.yellow().bold()
                );
                return Ok(full_content);
            }

            let mut sorted_indices: Vec<usize> = tool_calls_map.keys().cloned().collect();
            sorted_indices.sort_unstable();
            let tool_calls: Vec<ToolCall> = sorted_indices
                .into_iter()
                .filter_map(|idx| tool_calls_map.remove(&idx))
                .collect();

            let content_opt = if full_content.is_empty() { None } else { Some(full_content) };
            messages.push(ChatMessage::assistant(content_opt, None, Some(tool_calls.clone())));

            for call in tool_calls {
                let tool_name = &call.function.name;
                println!(
                    "    {} [Subagent:{}] ➜ {}",
                    "•".cyan(),
                    self.role.dimmed(),
                    tool_name.green()
                );
                let output = Box::pin(execute_tool(
                    tool_name,
                    &call.function.arguments,
                    Some(&self.client),
                    Some(&self.model),
                ))
                .await;
                messages.push(ChatMessage::tool(call.id, output));
            }
        }

        Ok(format!("Subagent [{}] completed max allowed steps.", self.role))
    }
}
