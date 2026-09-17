use crate::agent::AgentRunner;
use crate::client::ApiClient;
use crate::config::{AppConfig, PermissionMode};
use crate::oauth::run_oauth_login;
use crate::skills::SkillRegistry;
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
        let raw_model = config.default_model.clone();
        let current_model = raw_model
            .strip_prefix("global:")
            .or_else(|| raw_model.strip_prefix("cn:"))
            .unwrap_or(&raw_model)
            .to_string();
        let agent_mode = config.agent_mode;
        let mut messages = Vec::new();
        let effective_sys = config.build_effective_system_prompt();
        if !effective_sys.trim().is_empty() {
            messages.push(ChatMessage::system(&effective_sys));
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
        let model_display = crate::models_catalog::find_official_model(&self.current_model)
            .map(|d| format!("{} ({})", d.name, self.current_model))
            .unwrap_or_else(|| self.current_model.clone());
        println!(
            "{} {}",
            "▶ 当前模型:".bold(),
            model_display.green().bold()
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
            "{} {}",
            "▶ 权限模式:".bold(),
            self.config.permission_mode.to_string().cyan().bold()
        );
        println!(
            "{}",
            "输入 /model 切换模型，/permission 设置权限，/agent 开关自主工具，/help 查看帮助。\n".dimmed()
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
            "/skills" => {
                self.list_skills();
            }
            "/skill" => {
                if args.is_empty() {
                    self.list_skills();
                } else {
                    let sub = args[0];
                    match sub {
                        "list" => self.list_skills(),
                        "search" | "find" => {
                            let query = args[1..].join(" ");
                            self.search_skills(&query);
                        }
                        "show" => {
                            let name = args[1..].join(" ");
                            self.show_skill(&name);
                        }
                        "use" | "load" => {
                            let name = args[1..].join(" ");
                            self.use_skill(&name);
                        }
                        _ => {
                            let name = args.join(" ");
                            self.use_skill(&name);
                        }
                    }
                }
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
            "/permission" | "/p" => {
                if args.is_empty() {
                    self.show_permission_status();
                } else {
                    let opt = args.join(" ").to_lowercase();
                    match opt.as_str() {
                        "ask" | "询问" | "confirm" | "prompt" => {
                            self.config.permission_mode = PermissionMode::Ask;
                            let _ = self.config.save();
                            println!("✔ 工具权限已设置为: {} (每次调用工具前将弹出确认请求)", "询问 (Ask)".green().bold());
                        }
                        "allow-all" | "allow_all" | "all" | "允许所有" | "允许" | "auto" | "yes" => {
                            self.config.permission_mode = PermissionMode::AllowAll;
                            let _ = self.config.save();
                            println!("✔ 工具权限已设置为: {} (所有工具自主调用，无需每次询问)", "允许所有 (Allow All)".green().bold());
                        }
                        _ => {
                            println!("{} 未知权限选项: \"{}\"", "✖".red(), opt);
                            self.show_permission_status();
                        }
                    }
                }
            }
            "/login" => {
                let realm = args.first().copied().unwrap_or("");
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
                println!("权限模式: {}", self.config.permission_mode);
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
                print!("\r");
                crate::models_catalog::display_models_catalog(&models, &self.current_model);
            }
            Err(e) => {
                println!("\r{} {}", "✖ 获取模型列表失败:".red(), e);
                println!(
                    "常见内置模型: {}, {}, {}, {}",
                    "auto".green(),
                    "deepseek-v4.1-flash".green(),
                    "gpt-5.6-sol".yellow(),
                    "gemini-3.5-flash".yellow()
                );
            }
        }
    }

    async fn switch_model(&mut self, target: &str) {
        let target = target.trim();
        let clean_target = match target.to_lowercase().as_str() {
            "auto" => "default-model",
            "fast" => "fast-model",
            "balanced" => "balanced-model",
            "primary" => "primary-model",
            "deep" => "deep-model",
            "deepseek" | "ds" => "deepseek-v4.1-flash",
            "kimi" => "kimi-k3",
            "glm" => "glm-5.3",
            "gemini" => "gemini-3.5-flash",
            _ => target,
        };

        let resolved_id = match self.client.list_models().await {
            Ok(models) => {
                if let Some(m) = models.iter().find(|m| m.clean_id().eq_ignore_ascii_case(clean_target)) {
                    m.clean_id().to_string()
                } else if let Some(m) = models.iter().find(|m| m.id.eq_ignore_ascii_case(target)) {
                    m.clean_id().to_string()
                } else if let Some(m) = models.iter().find(|m| m.clean_id().to_lowercase().contains(&clean_target.to_lowercase())) {
                    m.clean_id().to_string()
                } else {
                    clean_target.to_string()
                }
            }
            Err(_) => clean_target.to_string(),
        };

        self.current_model = resolved_id.clone();
        self.config.default_model = self.current_model.clone();
        let _ = self.config.save();

        let display_name = crate::models_catalog::find_official_model(&self.current_model)
            .map(|d| format!("{} ({})", d.name, self.current_model))
            .unwrap_or_else(|| self.current_model.clone());

        println!("✔ 已切换至模型: {}", display_name.green().bold());
    }

    async fn handle_chat(&mut self, query: &str) {
        self.messages.push(ChatMessage::user(query));

        let model_name = crate::models_catalog::find_official_model(&self.current_model)
            .map(|d| d.name)
            .unwrap_or(&self.current_model);

        let model_display = if model_name.contains("DeepSeek") {
            model_name.magenta().bold()
        } else if model_name.contains("Auto") || model_name.contains("Fast") || model_name.contains("Balanced") {
            model_name.cyan().bold()
        } else {
            model_name.blue().bold()
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
        runner.permission_mode = self.config.permission_mode;

        if let Err(e) = runner.execute_turn(&mut self.messages).await {
            println!("\n{} {}", "✖ 请求或执行失败:".red().bold(), e);
            // 发生致命错误时移除最后一条用户消息防污染
            self.messages.pop();
        }

        if runner.permission_mode != self.config.permission_mode {
            self.config.permission_mode = runner.permission_mode;
            let _ = self.config.save();
        }
    }

    fn list_skills(&self) {
        let reg = SkillRegistry::default();
        let active = reg.list_active_skills();

        println!("{}", "── Codex & 系统活跃技能库 (Active Skills) ──".cyan().bold());
        if active.is_empty() {
            println!("  {}", "未发现活跃技能。可在 /home/root/.codex/skills 或 /home/codes/.agents/skills 放置 SKILL.md".dimmed());
        } else {
            for s in &active {
                println!(
                    "  {} {:<30} {}",
                    "★".yellow(),
                    s.name.green().bold(),
                    s.description.dimmed()
                );
            }
        }
        println!(
            "\n提示: 输入 {} 加载技能到当前上下文，或 {} 搜索 700+ 离线技能库。",
            "/skill use <技能名>".bold(),
            "/skill search <关键词>".bold()
        );
    }

    fn search_skills(&self, query: &str) {
        if query.trim().is_empty() {
            println!("{} 请输入搜索关键词，例如: /skill search android 或 /skill search rust", "ℹ".blue());
            return;
        }

        let reg = SkillRegistry::default();
        let matches = reg.search_all_skills(query);

        println!("{}", format!("── 技能检索结果 (关键词: \"{}\", 共 {} 个匹配) ──", query, matches.len()).cyan().bold());
        if matches.is_empty() {
            println!("  未找到匹配 \"{}\" 的技能。", query);
            return;
        }

        for (idx, s) in matches.iter().take(20).enumerate() {
            let cat_badge = if s.category == "active" {
                "[活跃]".green()
            } else {
                "[离线]".blue()
            };
            println!(
                "  {:>2}. {} {:<28} {}",
                idx + 1,
                cat_badge,
                s.name.yellow().bold(),
                s.description.dimmed()
            );
        }

        if matches.len() > 20 {
            println!("  ... 还有 {} 个结果被折叠，可缩窄关键词", matches.len() - 20);
        }

        println!("\n输入 {} 加载指定技能，或 {} 查看技能文档正文。", "/skill use <名称>".green(), "/skill show <名称>".cyan());
    }

    fn show_skill(&self, name: &str) {
        let reg = SkillRegistry::default();
        match reg.find_skill(name) {
            Some(skill) => match reg.load_skill_markdown(&skill) {
                Ok(content) => {
                    println!("{}", format!("── 技能文档: {} ──", skill.name).cyan().bold());
                    println!("{}", content);
                }
                Err(e) => println!("{} {}", "✖ 加载技能失败:".red(), e),
            },
            None => {
                println!("{} 未找到名称为 \"{}\" 的技能。可使用 /skill search 搜索。", "✖".red(), name);
            }
        }
    }

    pub fn use_skill(&mut self, name: &str) {
        let reg = SkillRegistry::default();
        match reg.find_skill(name) {
            Some(skill) => match reg.load_skill_markdown(&skill) {
                Ok(content) => {
                    let prompt_injection = format!(
                        "【已载入技能规范: {}】\n\n{}\n\n请在接下来的对话与代码操作中严格遵循该技能的规范、指导原则和关联脚本路径。",
                        skill.name, content
                    );
                    self.messages.push(ChatMessage::user(&prompt_injection));
                    println!("✔ 技能 [{}] 已载入当前会话上下文！", skill.name.green().bold());
                    println!("  描述: {}", skill.description.dimmed());
                    println!("  路径: {}", skill.skill_dir.display().to_string().dimmed());
                    let scripts_dir = skill.skill_dir.join("scripts");
                    if scripts_dir.exists() {
                        println!("  可调用的脚本:");
                        if let Ok(entries) = std::fs::read_dir(&scripts_dir) {
                            for e in entries.flatten() {
                                println!("    - {}", e.path().display());
                            }
                        }
                    }
                }
                Err(e) => println!("{} {}", "✖ 加载技能失败:".red(), e),
            },
            None => {
                println!("{} 未找到名为 \"{}\" 的技能。可输入 /skills 查看可用技能，或 /skill search 搜索。", "✖".red(), name);
            }
        }
    }

    fn show_permission_status(&self) {
        println!("{}", "── Agent 工具执行权限状态 ──".cyan().bold());
        println!("当前权限模式: {}", self.config.permission_mode.to_string().yellow().bold());
        println!("\n可用权限选项:");
        println!("  {:<28} 每次调用工具前均弹出询问确认 [y: 允许 / n: 拒绝 / a: 允许后续所有]", "/permission ask".yellow());
        println!("  {:<28} 允许所有工具自主执行，无需每次询问", "/permission allow-all".yellow());
    }

    pub fn print_help(&self) {
        println!("{}", "── WorkBuddy Code Agent 命令帮助 ──".cyan().bold());
        println!("  {:<26} 显示当前可用模型列表", "/model, /m".yellow());
        println!(
            "  {:<26} 切换模型 (如 /model deepseek-v4.1-flash)",
            "/model <name>".yellow()
        );
        println!("  {:<26} 开启/关闭 Agent 自主编程工具权限", "/agent [on|off]".yellow());
        println!("  {:<26} 设置工具执行权限 (ask 询问 / allow-all 允许所有)", "/permission [mode]".yellow());
        println!("  {:<26} 查看所有 Codex / 系统活跃技能", "/skills, /skill list".yellow());
        println!("  {:<26} 检索 Codex 与 700+ 离线技能库", "/skill search <query>".yellow());
        println!("  {:<26} 加载技能并注入到当前对话上下文", "/skill use <name>".yellow());
        println!("  {:<26} 查看指定技能的完整文档正文", "/skill show <name>".yellow());
        println!("  {:<26} 发起 OAuth 设备授权登录 (支持 cn 或 global)", "/login [realm]".yellow());
        println!("  {:<26} 清空当前会话上下文", "/clear, /c".yellow());
        println!("  {:<26} 查看当前会话轮数及信息", "/history".yellow());
        println!("  {:<26} 查看或修改系统提示词", "/system [prompt]".yellow());
        println!("  {:<26} 查看当前配置信息", "/config".yellow());
        println!("  {:<26} 显示此帮助菜单", "/help, /?".yellow());
        println!("  {:<26} 退出程序", "/exit, /quit, /q".yellow());
    }
}
