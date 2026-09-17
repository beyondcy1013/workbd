use crate::agent::AgentRunner;
use crate::client::ApiClient;
use crate::config::AppConfig;
use crate::oauth::run_oauth_login;
use crate::types::ChatMessage;
use colored::*;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::io::{self, Write};

pub struct ReplSession {
    config: AppConfig,
    client: ApiClient,
    current_model: String,
    messages: Vec<ChatMessage>,
    agent_mode: bool,
}

impl ReplSession {
    pub fn new(config: AppConfig) -> Self {
        let client = ApiClient::new(&config.api_base, &config.api_key);
        let current_model = config.default_model.clone();
        let agent_mode = config.agent_mode;
        let mut messages = Vec::new();
        if !config.system_prompt.trim().is_empty() {
            messages.push(ChatMessage::system(&config.system_prompt));
        }

        Self {
            config,
            client,
            current_model,
            messages,
            agent_mode,
        }
    }

    pub async fn run(&mut self) {
        println!("{}", "╔══════════════════════════════════════════════════════════════╗".cyan());
        println!("{}", "║              WorkBuddy Code (workbd Autonomous Agent)        ║".cyan().bold());
        println!("{}", "║        Native Rust Agent for WorkBuddy / CodeBuddy AI        ║".cyan());
        println!("{}", "╚══════════════════════════════════════════════════════════════╝".cyan());
        println!(
            "{} {}",
            "▶ 当前模型:".bold(),
            self.current_model.green().bold()
        );
        println!(
            "{} {}",
            "▶ API 基址:".bold(),
            self.config.api_base.dimmed()
        );
        let agent_status_text = if self.agent_mode {
            "开启 (具备读写文件、执行 bash、代码搜索自主权限)".green().bold()
        } else {
            "关闭 (纯对话模式)".yellow()
        };
        println!("{} {}", "▶ Agent 模式:".bold(), agent_status_text);
        println!(
            "{}",
            "输入 /model 切换模型，/agent 开关工具权限，/login 登录，/help 命令帮助，/exit 退出。\n".dimmed()
        );

        let history_file = AppConfig::history_file_path();
        let mut rl = DefaultEditor::new().unwrap_or_else(|_| {
            eprintln!("{}", "警告: 终端行编辑器初始化失败，进入普通模式".yellow());
            DefaultEditor::new().unwrap()
        });

        let _ = rl.load_history(&history_file);

        loop {
            let prompt = format!(
                "{} {} ",
                format!("[{}]", self.current_model).blue().bold(),
                "❯".bright_green()
            );

            let readline = rl.readline(&prompt);
            match readline {
                Ok(line) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }

                    let _ = rl.add_history_entry(trimmed);

                    if trimmed.starts_with('/') {
                        let should_continue = self.handle_command(trimmed).await;
                        if !should_continue {
                            break;
                        }
                    } else {
                        self.handle_chat(trimmed).await;
                    }
                }
                Err(ReadlineError::Interrupted) => {
                    println!("{}", "^C (再次按 Ctrl+C 或输入 /exit 退出)".dimmed());
                }
                Err(ReadlineError::Eof) => {
                    println!("{}", "\n再见！".green());
                    break;
                }
                Err(err) => {
                    eprintln!("{} {:?}", "读取输入出错:".red(), err);
                    break;
                }
            }
        }

        let _ = rl.save_history(&history_file);
    }

    async fn handle_command(&mut self, cmd: &str) -> bool {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        let command = parts[0];
        let args = &parts[1..];

        match command {
            "/exit" | "/quit" | "/q" => {
                println!("{}", "已退出 WorkBuddy CLI。".green());
                return false;
            }
            "/help" | "/h" | "/?" => {
                self.print_help();
            }
            "/clear" | "/c" => {
                self.messages.clear();
                if !self.config.system_prompt.trim().is_empty() {
                    self.messages.push(ChatMessage::system(&self.config.system_prompt));
                }
                println!("{}", "✔ 会话上下文已清空。".green());
            }
            "/agent" => {
                if args.is_empty() {
                    self.agent_mode = !self.agent_mode;
                } else {
                    let opt = args[0].to_lowercase();
                    self.agent_mode = opt == "on" || opt == "true" || opt == "1" || opt == "enable";
                }
                self.config.agent_mode = self.agent_mode;
                let _ = self.config.save();
                if self.agent_mode {
                    println!("{}", "✔ Agent 自主编程工具已启用 (支持 bash / 读写文件 / 搜索代码)。".green().bold());
                } else {
                    println!("{}", "✔ Agent 自主工具已关闭，切换至纯对话模式。".yellow());
                }
            }
            "/login" => {
                let realm = args.first().copied().unwrap_or("cn");
                println!("正在发起 OAuth 登录流程 (域: {})...", realm);
                if let Err(e) = run_oauth_login(realm).await {
                    println!("{} {}", "✖ 登录出错:".red().bold(), e);
                }
            }
            "/model" | "/m" => {
                if args.is_empty() {
                    self.list_and_show_models().await;
                } else {
                    let target = args.join(" ");
                    self.switch_model(&target).await;
                }
            }
            "/history" => {
                let user_count = self.messages.iter().filter(|m| m.role == "user").count();
                let tool_count = self.messages.iter().filter(|m| m.role == "tool").count();
                println!(
                    "{} 目前包含 {} 条上下文消息 (共 {} 轮对话, {} 次工具调用)。",
                    "ℹ".blue().bold(),
                    self.messages.len(),
                    user_count,
                    tool_count
                );
            }
            "/system" => {
                if args.is_empty() {
                    println!("{} 当前 System Prompt:\n{}", "ℹ".blue(), self.config.system_prompt);
                } else {
                    let new_sys = args.join(" ");
                    self.config.system_prompt = new_sys.clone();
                    if let Some(first) = self.messages.first_mut() {
                        if first.role == "system" {
                            first.content = Some(new_sys);
                        } else {
                            self.messages.insert(0, ChatMessage::system(new_sys));
                        }
                    } else {
                        self.messages.push(ChatMessage::system(new_sys));
                    }
                    println!("{}", "✔ System Prompt 已更新。".green());
                }
            }
            "/config" => {
                println!("{}", "── 当前配置 ──".cyan().bold());
                println!("配置文件: {}", AppConfig::config_file_path().display());
                println!("API 基址: {}", self.config.api_base);
                println!("当前模型: {}", self.current_model);
                println!("Agent 模式: {}", if self.agent_mode { "已开启" } else { "已关闭" });
                println!("温度参数: {}", self.config.temperature);
            }
            _ => {
                println!(
                    "{} 未知命令: {}。输入 /help 查看支持的命令。",
                    "✖".red(),
                    command
                );
            }
        }

        true
    }

    async fn list_and_show_models(&self) {
        print!("{}", "正在获取可用模型列表... ".dimmed());
        let _ = io::stdout().flush();

        match self.client.list_models().await {
            Ok(models) => {
                println!("\r{}", "── 可用模型列表 ──".cyan().bold());
                for m in models {
                    let is_current = m.id == self.current_model
                        || (m.id.starts_with("global:") && m.id.strip_prefix("global:") == Some(&self.current_model));
                    let mark = if is_current { "● [当前]".green().bold() } else { "○".dimmed() };
                    let credits = m.credits.unwrap_or_else(|| "-".into());
                    let desc = m.description.unwrap_or_default();
                    println!(
                        "  {} {:<32} {:<10} {}",
                        mark,
                        m.id.yellow().bold(),
                        format!("[{}]", credits).dimmed(),
                        desc.dimmed()
                    );
                }
                println!(
                    "\n提示: 输入 {} 切换模型，例如 {}",
                    "/model <模型名称>".bold(),
                    "/model deepseek-v4.1-flash".green()
                );
            }
            Err(e) => {
                println!("\r{} {}", "✖ 获取模型列表失败:".red(), e);
                println!(
                    "常见内置模型: {}, {}, {}, {}",
                    "deepseek-v4.1-flash".green(),
                    "global:deepseek-v4.1-flash".green(),
                    "gpt-5.5".yellow(),
                    "gemini-3.5-flash".yellow()
                );
            }
        }
    }

    async fn switch_model(&mut self, target: &str) {
        let target = target.trim();
        match self.client.list_models().await {
            Ok(models) => {
                // 1. 完全精确匹配
                if let Some(m) = models.iter().find(|m| m.id == target) {
                    self.current_model = m.id.clone();
                    self.config.default_model = self.current_model.clone();
                    let _ = self.config.save();
                    println!("✔ 已切换至模型: {}", self.current_model.green().bold());
                    return;
                }
                // 2. 模糊匹配
                if let Some(m) = models.iter().find(|m| {
                    m.id.strip_prefix("global:").unwrap_or(&m.id) == target
                        || m.id.strip_prefix("cn:").unwrap_or(&m.id) == target
                        || m.id.contains(target)
                }) {
                    self.current_model = m.id.clone();
                    self.config.default_model = self.current_model.clone();
                    let _ = self.config.save();
                    println!("✔ 匹配并切换至模型: {}", self.current_model.green().bold());
                    return;
                }
            }
            Err(_) => {}
        }

        self.current_model = target.to_string();
        self.config.default_model = self.current_model.clone();
        let _ = self.config.save();
        println!("✔ 已指定切换至模型: {}", self.current_model.green().bold());
    }

    async fn handle_chat(&mut self, query: &str) {
        self.messages.push(ChatMessage::user(query));

        let model_display = if self.current_model.contains("deepseek") {
            "DeepSeek".magenta().bold()
        } else {
            "AI".blue().bold()
        };

        let mode_tag = if self.agent_mode { "[Agent]" } else { "[Chat]" };
        print!("{} {}: ", mode_tag.dimmed(), model_display);
        let _ = io::stdout().flush();

        let mut runner = AgentRunner::new(
            self.client.clone(),
            self.current_model.clone(),
            self.config.temperature,
        );
        runner.tools_enabled = self.agent_mode;

        if let Err(e) = runner.execute_turn(&mut self.messages).await {
            println!("\n{} {}", "✖ 请求或执行失败:".red().bold(), e);
            // 发生致命错误时移除最后一条用户消息防污染
            self.messages.pop();
        }
    }

    pub fn print_help(&self) {
        println!("{}", "── WorkBuddy Code Agent 命令帮助 ──".cyan().bold());
        println!("  {:<26} 显示当前可用模型列表", "/model, /m".yellow());
        println!(
            "  {:<26} 切换模型 (如 /model deepseek-v4.1-flash)",
            "/model <name>".yellow()
        );
        println!("  {:<26} 开启/关闭 Agent 自主编程工具权限", "/agent [on|off]".yellow());
        println!("  {:<26} 发起 OAuth 设备授权登录 (支持 cn 或 global)", "/login [realm]".yellow());
        println!("  {:<26} 清空当前会话上下文", "/clear, /c".yellow());
        println!("  {:<26} 查看当前会话轮数及信息", "/history".yellow());
        println!("  {:<26} 查看或修改系统提示词", "/system [prompt]".yellow());
        println!("  {:<26} 查看当前配置信息", "/config".yellow());
        println!("  {:<26} 显示此帮助菜单", "/help, /?".yellow());
        println!("  {:<26} 退出程序", "/exit, /quit, /q".yellow());
    }
}
