use crate::checkpoint::{CheckpointManager, SharedCheckpointManager};
use crate::mcp::McpRegistry;
use crate::task_manager::{SharedTaskManager, TaskManager};
use crate::types::{FunctionDefinition, ToolDefinition};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

static GLOBAL_CHECKPOINT_MGR: OnceLock<SharedCheckpointManager> = OnceLock::new();
static GLOBAL_TASK_MGR: OnceLock<SharedTaskManager> = OnceLock::new();
static GLOBAL_MCP_REGISTRY: OnceLock<Arc<McpRegistry>> = OnceLock::new();

pub fn get_checkpoint_manager() -> SharedCheckpointManager {
    GLOBAL_CHECKPOINT_MGR
        .get_or_init(CheckpointManager::new_shared)
        .clone()
}

pub fn get_task_manager() -> SharedTaskManager {
    GLOBAL_TASK_MGR
        .get_or_init(TaskManager::new)
        .clone()
}

pub fn get_mcp_registry() -> Arc<McpRegistry> {
    GLOBAL_MCP_REGISTRY
        .get_or_init(|| Arc::new(McpRegistry::new()))
        .clone()
}

pub fn all_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: "bash".to_string(),
                description: "Execute a shell/bash command in the current workspace and return stdout, stderr, and exit status. Supports foreground command execution or background daemon tasks (is_background=true).".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "The command line string to execute."
                        },
                        "is_background": {
                            "type": "boolean",
                            "description": "Set to true to launch as a persistent background daemon task."
                        }
                    },
                    "required": ["command"]
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: "manage_task".to_string(),
                description: "Inspect or manage background tasks launched via bash (actions: 'list', 'logs', 'kill').".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["list", "logs", "kill"],
                            "description": "The action to perform: 'list' (show all tasks), 'logs' (tail output), 'kill' (stop task)."
                        },
                        "task_id": {
                            "type": "string",
                            "description": "The task ID to query or kill (e.g. 'task-1'). Required for 'logs' and 'kill'."
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Number of recent log lines to retrieve (default 50)."
                        }
                    },
                    "required": ["action"]
                }),
            },
        },
        crate::subagent::delegate_tool_definition(),
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: "read_file".to_string(),
                description: "Read the contents of a file at the given path, optionally specifying a line range.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Relative or absolute path to the file to read."
                        },
                        "start_line": {
                            "type": "integer",
                            "description": "Optional starting line number (1-indexed, inclusive)."
                        },
                        "end_line": {
                            "type": "integer",
                            "description": "Optional ending line number (1-indexed, inclusive)."
                        }
                    },
                    "required": ["path"]
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: "write_file".to_string(),
                description: "Create a new file or completely overwrite an existing file with the specified content. Automatically generates a colored diff and records a snapshot checkpoint for rollback.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file to write."
                        },
                        "content": {
                            "type": "string",
                            "description": "Complete text content to write into the file."
                        }
                    },
                    "required": ["path", "content"]
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: "replace_in_file".to_string(),
                description: "Replace exact target string with replacement string in an existing file. Automatically generates a colored diff and records a snapshot checkpoint for rollback.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file to edit."
                        },
                        "target_content": {
                            "type": "string",
                            "description": "The exact string sequence to find and replace in the file."
                        },
                        "replacement_content": {
                            "type": "string",
                            "description": "The new string sequence to replace the target with."
                        }
                    },
                    "required": ["path", "target_content", "replacement_content"]
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: "list_dir".to_string(),
                description: "List files and subdirectories within a directory path.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Directory path to list (defaults to current directory '.')."
                        }
                    }
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: "search_code".to_string(),
                description: "Search for text or regex pattern across files in the workspace (ripgrep/grep).".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Pattern or keyword to search for."
                        },
                        "path": {
                            "type": "string",
                            "description": "Optional directory to search within (defaults to '.')."
                        }
                    },
                    "required": ["query"]
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: "load_skill".to_string(),
                description: "Load a Codex / System skill by name to retrieve its full guidelines, documentation, and associated scripts paths.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "The exact name or keyword of the skill to load (e.g. 'webclx-compile-and-deploy', 'codex-db-maintenance')."
                        }
                    },
                    "required": ["name"]
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: "search_skills".to_string(),
                description: "Search across the 720+ system and offline skills library by keyword.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Keyword to search for across skill names and descriptions."
                        }
                    },
                    "required": ["query"]
                }),
            },
        },
    ]
}

pub async fn execute_tool(
    name: &str,
    args_json: &str,
    client: Option<&crate::client::ApiClient>,
    model: Option<&str>,
) -> String {
    let args: Value = match serde_json::from_str(args_json) {
        Ok(v) => v,
        Err(e) => return format!("错误: 参数 JSON 解析失败: {}", e),
    };

    // 优先检查是否为 MCP 外部工具
    if name.starts_with("mcp__") {
        let mcp = get_mcp_registry();
        if let Some(res) = mcp.call_tool(name, args_json).await {
            return res;
        }
    }

    match name {
        "bash" => {
            let cmd = match args.get("command").and_then(|v| v.as_str()) {
                Some(c) => c,
                None => return "错误: 缺少必填参数 command".to_string(),
            };
            let is_bg = args.get("is_background").and_then(|v| v.as_bool()).unwrap_or(false);
            run_bash(cmd, is_bg).await
        }
        "manage_task" => {
            let action = match args.get("action").and_then(|v| v.as_str()) {
                Some(a) => a,
                None => return "错误: 缺少必填参数 action".to_string(),
            };
            let task_id = args.get("task_id").and_then(|v| v.as_str());
            let limit = args.get("limit").and_then(|v| v.as_u64()).map(|v| v as usize);
            run_manage_task(action, task_id, limit).await
        }
        "delegate_task" => {
            let role = match args.get("role").and_then(|v| v.as_str()) {
                Some(r) => r,
                None => "explorer",
            };
            let prompt = match args.get("prompt").and_then(|v| v.as_str()) {
                Some(p) => p,
                None => return "错误: 缺少必填参数 prompt".to_string(),
            };
            match (client, model) {
                (Some(c), Some(m)) => {
                    let runner = crate::subagent::SubagentRunner::new(c.clone(), m.to_string(), role);
                    match runner.run_task(prompt).await {
                        Ok(res) => format!("── 子智能体 [{}] 汇报完成 ──\n{}", role, res),
                        Err(e) => format!("子智能体执行失败: {}", e),
                    }
                }
                _ => "错误: 当前环境缺少 ApiClient 或模型配置，无法派发子智能体".to_string(),
            }
        }
        "load_skill" => {
            let skill_name = match args.get("name").and_then(|v| v.as_str()) {
                Some(n) => n,
                None => return "错误: 缺少必填参数 name".to_string(),
            };
            run_load_skill(skill_name)
        }
        "search_skills" => {
            let query = match args.get("query").and_then(|v| v.as_str()) {
                Some(q) => q,
                None => return "错误: 缺少必填参数 query".to_string(),
            };
            run_search_skills(query)
        }
        "read_file" => {
            let path = match args.get("path").and_then(|v| v.as_str()) {
                Some(p) => p,
                None => return "错误: 缺少必填参数 path".to_string(),
            };
            let start = args.get("start_line").and_then(|v| v.as_u64()).map(|v| v as usize);
            let end = args.get("end_line").and_then(|v| v.as_u64()).map(|v| v as usize);
            run_read_file(path, start, end)
        }
        "write_file" => {
            let path = match args.get("path").and_then(|v| v.as_str()) {
                Some(p) => p,
                None => return "错误: 缺少必填参数 path".to_string(),
            };
            let content = match args.get("content").and_then(|v| v.as_str()) {
                Some(c) => c,
                None => return "错误: 缺少必填参数 content".to_string(),
            };
            run_write_file(path, content)
        }
        "replace_in_file" => {
            let path = match args.get("path").and_then(|v| v.as_str()) {
                Some(p) => p,
                None => return "错误: 缺少必填参数 path".to_string(),
            };
            let target = match args.get("target_content").and_then(|v| v.as_str()) {
                Some(t) => t,
                None => return "错误: 缺少必填参数 target_content".to_string(),
            };
            let replacement = match args.get("replacement_content").and_then(|v| v.as_str()) {
                Some(r) => r,
                None => return "错误: 缺少必填参数 replacement_content".to_string(),
            };
            run_replace_in_file(path, target, replacement)
        }
        "list_dir" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            run_list_dir(path)
        }
        "search_code" => {
            let query = match args.get("query").and_then(|v| v.as_str()) {
                Some(q) => q,
                None => return "错误: 缺少必填参数 query".to_string(),
            };
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            run_search_code(query, path)
        }
        _ => format!("未知工具: {}", name),
    }
}

async fn run_bash(command: &str, is_background: bool) -> String {
    if is_background {
        match get_task_manager().spawn_task(command).await {
            Ok(task_id) => {
                format!(
                    "✔ 已将命令放入后台作为守护进程启动 (Task ID: {})\n提示: 可使用 manage_task 工具或 /tasks 命令监控或终止该任务。",
                    task_id
                )
            }
            Err(e) => format!("启动后台守护任务失败: {}", e),
        }
    } else {
        let output = match tokio::time::timeout(
            Duration::from_secs(120),
            tokio::process::Command::new("bash")
                .arg("-c")
                .arg(command)
                .output(),
        )
        .await
        {
            Ok(Ok(out)) => out,
            Ok(Err(e)) => return format!("执行命令失败: {}", e),
            Err(_) => return "命令执行超时 (120 秒)".to_string(),
        };

        let exit_code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let mut result = format!("[退出码: {}]\n", exit_code);
        if !stdout.trim().is_empty() {
            result.push_str(&format!("--- 标准输出 (stdout) ---\n{}\n", stdout));
        }
        if !stderr.trim().is_empty() {
            result.push_str(&format!("--- 错误输出 (stderr) ---\n{}\n", stderr));
        }
        if stdout.trim().is_empty() && stderr.trim().is_empty() {
            result.push_str("(无输出)\n");
        }

        if result.len() > 16000 {
            format!("{}\n... (输出已截断，共 {} 字符)", &result[..16000], result.len())
        } else {
            result
        }
    }
}

async fn run_manage_task(action: &str, task_id: Option<&str>, limit: Option<usize>) -> String {
    let tm = get_task_manager();
    match action.to_lowercase().as_str() {
        "list" | "ls" => {
            let tasks = tm.list_tasks();
            if tasks.is_empty() {
                "当前没有后台运行中的任务。".to_string()
            } else {
                let mut out = String::from("── 当前后台任务清单 ──\n");
                for (id, cmd, status, started) in tasks {
                    out.push_str(&format!("  {} [{}] {} (启动时间: {})\n", id, status, cmd, started));
                }
                out
            }
        }
        "logs" | "log" | "output" => {
            let id = match task_id {
                Some(id) => id,
                None => return "错误: 查询日志必须提供 task_id".to_string(),
            };
            match tm.get_logs(id, limit) {
                Ok(logs) => format!("── 任务 {} 最近输出 ──\n{}", id, logs),
                Err(e) => e,
            }
        }
        "kill" | "stop" | "cancel" => {
            let id = match task_id {
                Some(id) => id,
                None => return "错误: 终止任务必须提供 task_id".to_string(),
            };
            match tm.kill_task(id).await {
                Ok(msg) => msg,
                Err(e) => e,
            }
        }
        _ => format!("未知 manage_task 动作: '{}', 可选: list, logs, kill", action),
    }
}

fn run_read_file(path: &str, start_line: Option<usize>, end_line: Option<usize>) -> String {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => return format!("无法读取文件 '{}': {}", path, e),
    };

    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();

    let start = start_line.unwrap_or(1).max(1);
    let end = end_line.unwrap_or(total_lines).min(total_lines);

    if start > total_lines {
        return format!("文件共 {} 行，指定的起始行 {} 超出范围", total_lines, start);
    }

    let mut result = format!("文件: {} (总行数: {}, 显示 {}-{} 行):\n", path, total_lines, start, end);
    for i in start..=end {
        result.push_str(&format!("{:4} | {}\n", i, lines[i - 1]));
    }

    result
}

fn run_write_file(path: &str, content: &str) -> String {
    let p = Path::new(path);
    let old_content = fs::read_to_string(p).ok();
    let diff_stats = crate::diff::compute_diff(path, old_content.as_deref().unwrap_or(""), content);

    if let Some(parent) = p.parent() {
        if !parent.exists() {
            if let Err(e) = fs::create_dir_all(parent) {
                return format!("创建父目录失败 '{}': {}", parent.display(), e);
            }
        }
    }

    match fs::write(path, content) {
        Ok(_) => {
            get_checkpoint_manager().lock().unwrap().record_file_change(
                p,
                old_content,
                Some(content.to_string()),
                &format!("write_file: {}", path),
            );
            format!(
                "成功写入文件 '{}' ({} 字节, +{} -{} 行)\n{}",
                path,
                content.len(),
                diff_stats.added,
                diff_stats.deleted,
                diff_stats.colored_diff
            )
        }
        Err(e) => format!("写入文件失败 '{}': {}", path, e),
    }
}

fn run_replace_in_file(path: &str, target_content: &str, replacement_content: &str) -> String {
    let p = Path::new(path);
    let content = match fs::read_to_string(p) {
        Ok(c) => c,
        Err(e) => return format!("读取文件失败 '{}': {}", path, e),
    };

    if !content.contains(target_content) {
        return format!("在文件 '{}' 中未找到目标文本序列，无法进行替换", path);
    }

    let count = content.matches(target_content).count();
    let new_content = content.replacen(target_content, replacement_content, 1);
    let diff_stats = crate::diff::compute_diff(path, &content, &new_content);

    match fs::write(path, &new_content) {
        Ok(_) => {
            get_checkpoint_manager().lock().unwrap().record_file_change(
                p,
                Some(content),
                Some(new_content),
                &format!("replace_in_file: {}", path),
            );
            let match_info = if count > 1 {
                format!("（共找到 {} 处匹配，已替换第 1 处）", count)
            } else {
                String::new()
            };
            format!(
                "成功在文件 '{}' 中完成精确替换 {} (+{} -{} 行)\n{}",
                path,
                match_info,
                diff_stats.added,
                diff_stats.deleted,
                diff_stats.colored_diff
            )
        }
        Err(e) => format!("写入文件失败 '{}': {}", path, e),
    }
}

fn run_list_dir(path: &str) -> String {
    let p = Path::new(path);
    if !p.exists() {
        return format!("目录不存在: '{}'", path);
    }
    if !p.is_dir() {
        return format!("路径不是一个目录: '{}'", path);
    }

    let entries = match fs::read_dir(p) {
        Ok(e) => e,
        Err(err) => return format!("无法读取目录 '{}': {}", path, err),
    };

    let mut dirs = Vec::new();
    let mut files = Vec::new();

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if let Ok(file_type) = entry.file_type() {
            if file_type.is_dir() {
                dirs.push(name);
            } else {
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                files.push((name, size));
            }
        }
    }

    dirs.sort();
    files.sort_by(|a, b| a.0.cmp(&b.0));

    let mut result = format!("目录列表: '{}' ({} 目录, {} 文件):\n", path, dirs.len(), files.len());
    for d in dirs {
        result.push_str(&format!("  [目录] {}/\n", d));
    }
    for (f, size) in files {
        result.push_str(&format!("  [文件] {:<30} ({} 字节)\n", f, size));
    }

    result
}

fn run_search_code(query: &str, path: &str) -> String {
    let rg_output = Command::new("rg")
        .arg("-n")
        .arg("--max-count")
        .arg("50")
        .arg(query)
        .arg(path)
        .output();

    match rg_output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if !stdout.trim().is_empty() {
                if stdout.len() > 10000 {
                    format!("{}\n... (搜索结果已截断)", &stdout[..10000])
                } else {
                    stdout.to_string()
                }
            } else {
                format!("在 '{}' 中未搜索到匹配 '{}' 的代码", path, query)
            }
        }
        Err(_) => {
            let grep_output = Command::new("grep")
                .arg("-rn")
                .arg(query)
                .arg(path)
                .output();

            match grep_output {
                Ok(out) => {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    if !stdout.trim().is_empty() {
                        stdout.to_string()
                    } else {
                        format!("未搜索到匹配 '{}' 的代码", query)
                    }
                }
                Err(e) => format!("搜索代码失败: {}", e),
            }
        }
    }
}

fn run_load_skill(name: &str) -> String {
    let reg = crate::skills::SkillRegistry::default();
    match reg.find_skill(name) {
        Some(skill) => match reg.load_skill_markdown(&skill) {
            Ok(doc) => doc,
            Err(e) => format!("加载技能失败: {}", e),
        },
        None => format!("未在活跃或离线技能库中找到名称为 '{}' 的技能。请使用 search_skills 检索。", name),
    }
}

fn run_search_skills(query: &str) -> String {
    let reg = crate::skills::SkillRegistry::default();
    let matches = reg.search_all_skills(query);
    if matches.is_empty() {
        return format!("在技能库中未搜索到匹配 '{}' 的技能。", query);
    }

    let mut out = format!("找到 {} 个匹配技能 (关键词: \"{}\"):\n", matches.len(), query);
    for (idx, s) in matches.iter().take(20).enumerate() {
        let cat = if s.category == "active" { "[活跃]" } else { "[离线]" };
        out.push_str(&format!("{}. {} {:<24} {}\n", idx + 1, cat, s.name, s.description));
    }
    if matches.len() > 20 {
        out.push_str(&format!("... 还有 {} 个匹配结果被省略\n", matches.len() - 20));
    }
    out.push_str("\n提示: 可调用 load_skill 加载所需技能的详细使用文档。");
    out
}
