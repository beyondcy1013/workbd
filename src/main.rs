mod agent;
mod checkpoint;
mod client;
mod compaction;
mod config;
mod diff;
mod image;
mod mcp;
mod models_catalog;
mod oauth;
mod repl;
mod skills;
mod subagent;
mod task_manager;
mod tools;
mod types;

use agent::AgentRunner;
use clap::Parser;
use client::ApiClient;
use colored::*;
use config::{AppConfig, PermissionMode};
use oauth::run_oauth_login;
use repl::ReplSession;
use skills::SkillRegistry;
use types::ChatMessage;

#[derive(Parser, Debug)]
#[command(name = "workbd")]
#[command(author = "DeepSeek & WorkBuddy Pair")]
#[command(version = "0.1.0")]
#[command(about = "Native Rust Autonomous Coding Agent for WorkBuddy / CodeBuddy AI", long_about = None)]
struct Cli {
    /// 一次性任务提示词（若提供则自主执行工具调用并输出结果后退出）
    #[arg(value_name = "PROMPT")]
    prompt: Option<String>,

    /// 指定使用的大模型（默认: deepseek-v4.1-flash）
    #[arg(short, long, value_name = "MODEL")]
    model: Option<String>,

    /// API 基址（默认: http://127.0.0.1:7863/v1）
    #[arg(long, value_name = "URL")]
    api_base: Option<String>,

    /// API 密钥（默认: LOCAL_TOKEN_CHANGE_ME）
    #[arg(long, value_name = "KEY")]
    api_key: Option<String>,

    /// 自定义系统提示词 (System Prompt)
    #[arg(short, long, value_name = "PROMPT")]
    system: Option<String>,

    /// 采样温度 (Temperature, 0.0 ~ 2.0)
    #[arg(short, long, value_name = "FLOAT")]
    temperature: Option<f32>,

    /// 禁用 Agent 工具调用权限（进入纯对话模式）
    #[arg(long)]
    no_agent: bool,

    /// 工具执行权限模式 (ask: 每次调用前询问确认, allow-all: 允许所有)
    #[arg(long, value_name = "MODE")]
    permission: Option<String>,

    /// 工具调用前强制询问确认 (等价于 --permission ask)
    #[arg(long)]
    ask: bool,

    /// 预先载入并激活指定技能（支持 Codex 技能或离线技能）
    #[arg(long, value_name = "SKILL_NAME")]
    skill: Option<String>,

    /// 列出所有可用的 Codex / 系统技能并退出
    #[arg(long)]
    list_skills: bool,

    /// 检索 Codex 与 700+ 离线技能库
    #[arg(long, value_name = "QUERY")]
    search_skills: Option<String>,

    /// 禁用自动注入技能清单到系统提示词中
    #[arg(long)]
    no_skills: bool,

    /// 发起 OAuth 设备授权登录（cn: 国内版, global: 国际版）
    #[arg(long, value_name = "REALM")]
    login: Option<Option<String>>,

    /// 附加分析本地图片或剪贴板图片（路径或 clipboard）
    #[arg(long, value_name = "IMAGE_PATH")]
    image: Option<String>,

    /// 查看当前登录账号、授权状态与系统信息并退出
    #[arg(long)]
    status: bool,

    /// 列出所有可用模型并退出
    #[arg(short = 'l', long)]
    list_models: bool,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // 检查是否为查看状态命令
    if cli.status {
        let config = AppConfig::load();
        let session = ReplSession::new(config);
        session.show_status();
        return;
    }

    // 检查是否为直接登录命令
    if let Some(realm_opt) = cli.login {
        let realm = realm_opt.unwrap_or_default();
        if let Err(e) = run_oauth_login(&realm).await {
            eprintln!("{} {}", "登录失败:".red().bold(), e);
            std::process::exit(1);
        }
        return;
    }

    let mut config = AppConfig::load();

    // 覆盖命令行参数
    if let Some(model) = cli.model {
        config.default_model = model;
    }
    if let Some(api_base) = cli.api_base {
        config.api_base = api_base;
    }
    if let Some(api_key) = cli.api_key {
        config.api_key = api_key;
    }
    if let Some(sys) = cli.system {
        config.system_prompt = sys;
    }
    if let Some(t) = cli.temperature {
        config.temperature = t;
    }
    if cli.no_agent {
        config.agent_mode = false;
    }
    if cli.no_skills {
        config.codex_skills = false;
    }
    if cli.ask {
        config.permission_mode = PermissionMode::Ask;
    } else if let Some(ref p) = cli.permission {
        match p.to_lowercase().as_str() {
            "ask" | "询问" | "confirm" => config.permission_mode = PermissionMode::Ask,
            "allow-all" | "allow_all" | "all" | "允许所有" | "auto" => {
                config.permission_mode = PermissionMode::AllowAll
            }
            _ => eprintln!("{} 未知权限模式 \"{}\"，使用默认配置", "⚠".yellow(), p),
        }
    }

    // 处理技能列表
    if cli.list_skills {
        let reg = SkillRegistry::default();
        let active = reg.list_active_skills();
        println!("{}", "── Codex & 系统活跃技能库 (Active Skills) ──".cyan().bold());
        if active.is_empty() {
            println!("  未在 /home/root/.codex/skills 或 /home/codes/.agents/skills 发现活跃技能。");
        } else {
            for s in active {
                println!("  {} {:<30} {}", "★".yellow(), s.name.green().bold(), s.description.dimmed());
            }
        }
        return;
    }

    // 处理技能检索
    if let Some(ref query) = cli.search_skills {
        let reg = SkillRegistry::default();
        let matches = reg.search_all_skills(query);
        println!("{}", format!("── 技能检索结果 (关键词: \"{}\", 共 {} 个匹配) ──", query, matches.len()).cyan().bold());
        if matches.is_empty() {
            println!("  未找到匹配 \"{}\" 的技能。", query);
        } else {
            for (idx, s) in matches.iter().take(25).enumerate() {
                let badge = if s.category == "active" { "[活跃]".green() } else { "[离线]".blue() };
                println!("  {:>2}. {} {:<28} {}", idx + 1, badge, s.name.yellow().bold(), s.description.dimmed());
            }
            if matches.len() > 25 {
                println!("  ... 还有 {} 个匹配结果被折叠", matches.len() - 25);
            }
        }
        return;
    }

    let client = ApiClient::new(&config.api_base, &config.api_key);

    // 列出模型
    if cli.list_models {
        println!("{}", "── 正在从网关获取模型列表 ──".cyan().bold());
        match client.list_models().await {
            Ok(models) => {
                models_catalog::display_models_catalog(&models, &config.default_model);
            }
            Err(e) => {
                eprintln!("{} {}", "获取模型列表失败:".red().bold(), e);
                std::process::exit(1);
            }
        }
        return;
    }

    // 自动发现并挂载外部 MCP 扩展服务
    crate::tools::get_mcp_registry().auto_load().await;

    // 单次任务执行模式 (Agent Runner)
    if let Some(prompt) = cli.prompt {
        let mut messages = Vec::new();
        let effective_sys = config.build_effective_system_prompt();
        if !effective_sys.trim().is_empty() {
            messages.push(ChatMessage::system(&effective_sys));
        }

        // 预载技能（如果有指定）
        if let Some(ref skill_name) = cli.skill {
            let reg = SkillRegistry::default();
            match reg.find_skill(skill_name) {
                Some(skill) => match reg.load_skill_markdown(&skill) {
                    Ok(content) => {
                        println!("✔ 已预载技能: {}", skill.name.green().bold());
                        messages.push(ChatMessage::user(&format!(
                            "【已载入技能规范: {}】\n\n{}\n\n请在接下来的任务执行中遵循此技能要求。",
                            skill.name, content
                        )));
                    }
                    Err(e) => eprintln!("{} 载入技能失败: {}", "⚠".yellow(), e),
                },
                None => eprintln!("{} 未找到指定技能 \"{}\"", "⚠".yellow(), skill_name),
            }
        }

        // 检测或载入图片
        let mut image_data = None;
        let mut final_prompt = prompt.clone();

        if let Some(ref img_arg) = cli.image {
            let res = if img_arg == "clipboard" || img_arg == "paste" {
                image::get_clipboard_image()
            } else {
                image::load_image_file(std::path::Path::new(img_arg))
            };
            match res {
                Ok(data) => image_data = Some(data),
                Err(e) => {
                    eprintln!("{} 载入图片失败: {}", "✖".red(), e);
                    std::process::exit(1);
                }
            }
        } else if let Some((img_path, remaining)) = image::detect_image_in_prompt(&prompt) {
            match image::load_image_file(&img_path) {
                Ok(data) => {
                    println!("{} 检测到图片路径: {}", "📷".cyan(), img_path.display().to_string().yellow());
                    image_data = Some(data);
                    if !remaining.trim().is_empty() {
                        final_prompt = remaining;
                    }
                }
                Err(e) => eprintln!("{} 读取图片失败: {}", "⚠".yellow(), e),
            }
        }

        if let Some((mime, b64)) = image_data {
            if !image::is_vision_model(&config.default_model) {
                let rec = image::recommended_vision_model();
                println!(
                    "{} 当前模型 {} 不支持多模态视觉，已自动切换为推荐视觉模型: {}",
                    "ℹ".blue().bold(),
                    config.default_model.yellow(),
                    rec.green().bold()
                );
                config.default_model = rec.to_string();
            }
            messages.push(ChatMessage::user_with_image(&final_prompt, &mime, &b64));
        } else {
            messages.push(ChatMessage::user(&final_prompt));
        }

        let mut runner = AgentRunner::new(
            client,
            config.default_model.clone(),
            config.temperature,
        );
        runner.tools_enabled = config.agent_mode;
        runner.permission_mode = config.permission_mode;

        if let Err(e) = runner.execute_turn(&mut messages).await {
            eprintln!("\n{} {}", "执行失败:".red().bold(), e);
            std::process::exit(1);
        }
        return;
    }

    // 交互式 REPL 模式
    let mut session = ReplSession::new(config);
    if let Some(ref skill_name) = cli.skill {
        session.use_skill(skill_name);
    }
    session.run().await;
}
