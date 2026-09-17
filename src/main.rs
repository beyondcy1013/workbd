mod agent;
mod client;
mod config;
mod oauth;
mod repl;
mod tools;
mod types;

use agent::AgentRunner;
use clap::Parser;
use client::ApiClient;
use colored::*;
use config::AppConfig;
use oauth::run_oauth_login;
use repl::ReplSession;
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

    /// 发起 OAuth 设备授权登录（cn: 国内版, global: 国际版）
    #[arg(long, value_name = "REALM")]
    login: Option<Option<String>>,

    /// 列出所有可用模型并退出
    #[arg(short = 'l', long)]
    list_models: bool,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // 检查是否为直接登录命令
    if let Some(realm_opt) = cli.login {
        let realm = realm_opt.unwrap_or_else(|| "cn".to_string());
        println!("正在发起 OAuth 登录流程 (域: {})...", realm);
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

    let client = ApiClient::new(&config.api_base, &config.api_key);

    // 列出模型
    if cli.list_models {
        println!("{}", "── 正在从网关获取模型列表 ──".cyan().bold());
        match client.list_models().await {
            Ok(models) => {
                for m in models {
                    let mark = if m.id == config.default_model {
                        "● [默认]".green().bold()
                    } else {
                        "○".dimmed()
                    };
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
            }
            Err(e) => {
                eprintln!("{} {}", "获取模型列表失败:".red().bold(), e);
                std::process::exit(1);
            }
        }
        return;
    }

    // 单次任务执行模式 (Agent Runner)
    if let Some(prompt) = cli.prompt {
        let mut messages = Vec::new();
        if !config.system_prompt.trim().is_empty() {
            messages.push(ChatMessage::system(&config.system_prompt));
        }
        messages.push(ChatMessage::user(prompt));

        let mut runner = AgentRunner::new(
            client,
            config.default_model.clone(),
            config.temperature,
        );
        runner.tools_enabled = config.agent_mode;

        if let Err(e) = runner.execute_turn(&mut messages).await {
            eprintln!("\n{} {}", "执行失败:".red().bold(), e);
            std::process::exit(1);
        }
        return;
    }

    // 交互式 REPL 模式
    let mut session = ReplSession::new(config);
    session.run().await;
}
