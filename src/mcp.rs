use crate::types::{FunctionDefinition, ToolDefinition};
use colored::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    #[serde(default, rename = "mcpServers")]
    pub mcp_servers: HashMap<String, McpServerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

pub struct ActiveMcpServer {
    pub name: String,
    pub stdin: Arc<Mutex<ChildStdin>>,
    pub reader: Arc<Mutex<BufReader<tokio::process::ChildStdout>>>,
    pub child: Arc<Mutex<Child>>,
    pub tools: Vec<ToolDefinition>,
    req_id: AtomicU64,
}

#[derive(Clone, Default)]
pub struct McpRegistry {
    servers: Arc<Mutex<HashMap<String, Arc<ActiveMcpServer>>>>,
}

impl McpRegistry {
    pub fn new() -> Self {
        Self {
            servers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 自动从常用路径发现并加载 MCP 配置文件
    pub async fn auto_load(&self) {
        let candidate_paths = vec![
            dirs::home_dir().map(|h| h.join(".workbd").join("mcp.json")),
            dirs::home_dir().map(|h| h.join(".codebuddy").join("mcp.json")),
            Some(PathBuf::from("./mcp.json")),
        ];

        for path_opt in candidate_paths.into_iter().flatten() {
            if path_opt.exists() {
                self.load_from_file(&path_opt).await;
            }
        }
    }

    pub async fn load_from_file(&self, path: &Path) {
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(cfg) = serde_json::from_str::<McpConfig>(&content) {
                for (name, srv_cfg) in cfg.mcp_servers {
                    if let Err(e) = self.start_server(&name, &srv_cfg).await {
                        eprintln!("{} 启动 MCP 服务 [{}] 失败: {}", "⚠".yellow(), name, e);
                    }
                }
            }
        }
    }

    pub async fn start_server(&self, name: &str, cfg: &McpServerConfig) -> Result<(), String> {
        let mut cmd = Command::new(&cfg.command);
        cmd.args(&cfg.args);
        for (k, v) in &cfg.env {
            cmd.env(k, v);
        }
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::null());

        let mut child = cmd.spawn().map_err(|e| format!("启动进程失败: {}", e))?;
        let stdin = child.stdin.take().ok_or("无法捕获 stdin")?;
        let stdout = child.stdout.take().ok_or("无法捕获 stdout")?;

        let server = Arc::new(ActiveMcpServer {
            name: name.to_string(),
            stdin: Arc::new(Mutex::new(stdin)),
            reader: Arc::new(Mutex::new(BufReader::new(stdout))),
            child: Arc::new(Mutex::new(child)),
            tools: Vec::new(),
            req_id: AtomicU64::new(1),
        });

        // 执行 MCP 初始化握手 (initialize)
        let init_req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "workbd",
                    "version": "0.1.0"
                }
            }
        });
        let _ = server.send_request(init_req).await?;

        // 发送 initialized 通知
        let notify = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });
        server.send_notification(notify).await?;

        // 获取工具清单 (tools/list)
        let list_req = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        });
        let list_res = server.send_request(list_req).await?;

        let mut tools = Vec::new();
        if let Some(tool_list) = list_res.get("result").and_then(|r| r.get("tools")).and_then(|t| t.as_array()) {
            for t in tool_list {
                if let Some(tool_name) = t.get("name").and_then(|n| n.as_str()) {
                    let desc = t.get("description").and_then(|d| d.as_str()).unwrap_or("");
                    let input_schema = t.get("inputSchema").cloned().unwrap_or_else(|| json!({"type": "object"}));

                    let full_name = format!("mcp__{}__{}", name, tool_name);
                    tools.push(ToolDefinition {
                        tool_type: "function".to_string(),
                        function: FunctionDefinition {
                            name: full_name,
                            description: format!("[MCP: {}] {}", name, desc),
                            parameters: input_schema,
                        },
                    });
                }
            }
        }

        println!(
            "  {} 已挂载 MCP 服务 [{}]，发现 {} 个外部工具",
            "🔌".green(),
            name.cyan().bold(),
            tools.len().to_string().yellow().bold()
        );

        let mut srv_mut = Arc::try_unwrap(server).unwrap_or_else(|arc| (*arc).clone());
        srv_mut.tools = tools;
        self.servers.lock().await.insert(name.to_string(), Arc::new(srv_mut));

        Ok(())
    }

    pub async fn all_mcp_tools(&self) -> Vec<ToolDefinition> {
        let lock = self.servers.lock().await;
        let mut res = Vec::new();
        for s in lock.values() {
            res.extend(s.tools.clone());
        }
        res
    }

    pub async fn call_tool(&self, full_name: &str, arguments_json: &str) -> Option<String> {
        let parts: Vec<&str> = full_name.split("__").collect();
        if parts.len() < 3 || parts[0] != "mcp" {
            return None;
        }

        let server_name = parts[1];
        let tool_name = parts[2..].join("__");

        let srv = {
            let lock = self.servers.lock().await;
            lock.get(server_name).cloned()?
        };

        let args: Value = serde_json::from_str(arguments_json).unwrap_or_else(|_| json!({}));
        let req_id = srv.req_id.fetch_add(1, Ordering::SeqCst);

        let call_req = json!({
            "jsonrpc": "2.0",
            "id": req_id,
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": args
            }
        });

        match srv.send_request(call_req).await {
            Ok(res) => {
                if let Some(content) = res.get("result").and_then(|r| r.get("content")).and_then(|c| c.as_array()) {
                    let mut out = String::new();
                    for item in content {
                        if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                            out.push_str(text);
                            out.push('\n');
                        }
                    }
                    Some(out)
                } else if let Some(err) = res.get("error") {
                    Some(format!("MCP Error: {}", err))
                } else {
                    Some(serde_json::to_string_pretty(&res).unwrap_or_default())
                }
            }
            Err(e) => Some(format!("调用 MCP 服务失败: {}", e)),
        }
    }

    pub async fn list_servers_info(&self) -> Vec<(String, usize)> {
        let lock = self.servers.lock().await;
        let mut list = Vec::new();
        for (k, v) in lock.iter() {
            list.push((k.clone(), v.tools.len()));
        }
        list
    }
}

impl Clone for ActiveMcpServer {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            stdin: Arc::clone(&self.stdin),
            reader: Arc::clone(&self.reader),
            child: Arc::clone(&self.child),
            tools: self.tools.clone(),
            req_id: AtomicU64::new(self.req_id.load(Ordering::SeqCst)),
        }
    }
}

impl ActiveMcpServer {
    pub async fn send_request(&self, req: Value) -> Result<Value, String> {
        let line = serde_json::to_string(&req).map_err(|e| e.to_string())? + "\n";
        {
            let mut stdin = self.stdin.lock().await;
            stdin.write_all(line.as_bytes()).await.map_err(|e| e.to_string())?;
            stdin.flush().await.map_err(|e| e.to_string())?;
        }

        let mut reader = self.reader.lock().await;
        let mut resp_line = String::new();
        reader.read_line(&mut resp_line).await.map_err(|e| e.to_string())?;

        serde_json::from_str::<Value>(&resp_line).map_err(|e| format!("解析 JSON-RPC 失败: {} ({})", e, resp_line))
    }

    pub async fn send_notification(&self, note: Value) -> Result<(), String> {
        let line = serde_json::to_string(&note).map_err(|e| e.to_string())? + "\n";
        let mut stdin = self.stdin.lock().await;
        stdin.write_all(line.as_bytes()).await.map_err(|e| e.to_string())?;
        stdin.flush().await.map_err(|e| e.to_string())?;
        Ok(())
    }
}
