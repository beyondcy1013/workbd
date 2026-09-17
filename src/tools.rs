use crate::types::{FunctionDefinition, ToolDefinition};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

pub fn all_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: "bash".to_string(),
                description: "Execute a shell/bash command in the current workspace and return stdout, stderr, and exit status. Use this to compile code, run tests, install packages, check git, or inspect system state.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "The command line string to execute."
                        }
                    },
                    "required": ["command"]
                }),
            },
        },
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
                description: "Create a new file or completely overwrite an existing file with the specified content.".to_string(),
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
                description: "Replace exact target string with replacement string in an existing file. This is preferred over rewriting entire large files.".to_string(),
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

pub async fn execute_tool(name: &str, args_json: &str) -> String {
    let args: Value = match serde_json::from_str(args_json) {
        Ok(v) => v,
        Err(e) => return format!("错误: 参数 JSON 解析失败: {}", e),
    };

    match name {
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
        "bash" => {
            let cmd = match args.get("command").and_then(|v| v.as_str()) {
                Some(c) => c,
                None => return "错误: 缺少必填参数 command".to_string(),
            };
            run_bash(cmd).await
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

async fn run_bash(command: &str) -> String {
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

    // 限制最大长度避免打爆上下文 (最大 16000 字符)
    if result.len() > 16000 {
        format!("{}\n... (输出已截断，共 {} 字符)", &result[..16000], result.len())
    } else {
        result
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
    if let Some(parent) = p.parent() {
        if !parent.exists() {
            if let Err(e) = fs::create_dir_all(parent) {
                return format!("创建父目录失败 '{}': {}", parent.display(), e);
            }
        }
    }

    match fs::write(path, content) {
        Ok(_) => format!("成功写入文件 '{}' ({} 字节)", path, content.len()),
        Err(e) => format!("写入文件失败 '{}': {}", path, e),
    }
}

fn run_replace_in_file(path: &str, target_content: &str, replacement_content: &str) -> String {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => return format!("读取文件失败 '{}': {}", path, e),
    };

    if !content.contains(target_content) {
        return format!("在文件 '{}' 中未找到目标文本序列，无法进行替换", path);
    }

    let count = content.matches(target_content).count();
    let new_content = content.replacen(target_content, replacement_content, 1);

    match fs::write(path, new_content) {
        Ok(_) => {
            if count > 1 {
                format!("成功替换文件 '{}' 中的第 1 处匹配（共找到 {} 处匹配）", path, count)
            } else {
                format!("成功在文件 '{}' 中完成精确替换", path)
            }
        }
        Err(e) => format!("写入文件失败 '{}': {}", path, e),
    }
}

fn run_list_dir(path: &str) -> String {
    let entries = match fs::read_dir(path) {
        Ok(e) => e,
        Err(e) => return format!("读取目录失败 '{}': {}", path, e),
    };

    let mut dirs = Vec::new();
    let mut files = Vec::new();

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') && name != "." {
            continue;
        }
        if let Ok(meta) = entry.metadata() {
            if meta.is_dir() {
                dirs.push(format!("📁 {}/", name));
            } else {
                files.push(format!("📄 {:<32} ({} B)", name, meta.len()));
            }
        }
    }

    dirs.sort();
    files.sort();

    let mut out = format!("目录: {}\n", path);
    for d in dirs {
        out.push_str(&format!("  {}\n", d));
    }
    for f in files {
        out.push_str(&format!("  {}\n", f));
    }
    out
}

fn run_search_code(query: &str, path: &str) -> String {
    // 优先尝试 ripgrep
    let out = Command::new("rg")
        .args(["--line-number", "--max-count", "20", "--max-columns", "200", query, path])
        .output();

    if let Ok(output) = out {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout.trim().is_empty() {
            return format!("搜索结果 (rg):\n{}", stdout);
        }
    }

    // 回落 grep
    let out = Command::new("grep")
        .args(["-rn", "-m", "20", query, path])
        .output();

    if let Ok(output) = out {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout.trim().is_empty() {
            return format!("搜索结果 (grep):\n{}", stdout);
        }
    }

    format!("在 '{}' 中未搜索到包含 '{}' 的内容", path, query)
}

fn run_load_skill(name: &str) -> String {
    let reg = crate::skills::SkillRegistry::default();
    if let Some(skill) = reg.find_skill(name) {
        match reg.load_skill_markdown(&skill) {
            Ok(md) => md,
            Err(e) => format!("加载技能失败: {}", e),
        }
    } else {
        format!("未找到名为 '{}' 的技能。请使用 search_skills 搜索全量离线技能库或检查技能名称拼写。", name)
    }
}

fn run_search_skills(query: &str) -> String {
    let reg = crate::skills::SkillRegistry::default();
    let matches = reg.search_all_skills(query);
    if matches.is_empty() {
        return format!("在系统和离线技能库中未搜索到包含 '{}' 的技能", query);
    }

    let mut out = format!("找到 {} 个匹配的技能 (显示前 15 个):\n", matches.len());
    for s in matches.iter().take(15) {
        out.push_str(&format!("- **{}** ({}): {}\n", s.name, s.category, s.description));
    }
    out
}
