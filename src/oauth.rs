use colored::*;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, CONTENT_TYPE, ORIGIN, REFERER, USER_AGENT};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::Duration;

const CN_BASE: &str = "https://copilot.tencent.com";
const CN_ORIGIN: &str = "https://www.codebuddy.cn";

const GLOBAL_BASE: &str = "https://www.workbuddy.ai";
const GLOBAL_ORIGIN: &str = "https://www.workbuddy.ai";

const CLIENT_UA: &str = "CLI/2.63.2 CodeBuddy/2.63.2";

#[derive(Debug, Deserialize)]
struct StateResponse {
    code: i32,
    msg: Option<String>,
    data: Option<StateData>,
}

#[derive(Debug, Deserialize)]
struct StateData {
    state: String,
    #[serde(rename = "authUrl")]
    auth_url: Option<String>,
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    code: i32,
    #[allow(dead_code)]
    msg: Option<String>,
    data: Option<TokenData>,
}

#[derive(Debug, Deserialize)]
struct TokenData {
    #[serde(rename = "accessToken")]
    access_token: String,
    #[serde(rename = "refreshToken")]
    refresh_token: String,
    #[serde(rename = "expiresIn")]
    expires_in: i64,
}

#[derive(Debug, Deserialize)]
struct AccountResponse {
    #[allow(dead_code)]
    code: i32,
    #[allow(dead_code)]
    msg: Option<String>,
    data: Option<AccountData>,
}

#[derive(Debug, Deserialize)]
struct AccountData {
    uid: String,
    nickname: Option<String>,
    domain: Option<String>,
    #[serde(rename = "enterpriseId")]
    enterprise_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct AuthFileModel {
    account: AuthFileAccount,
    auth: AuthFileDetails,
}

#[derive(Debug, Serialize)]
struct AuthFileAccount {
    uid: String,
    #[serde(rename = "enterpriseId")]
    enterprise_id: String,
    nickname: String,
}

#[derive(Debug, Serialize)]
struct AuthFileDetails {
    #[serde(rename = "accessToken")]
    access_token: String,
    #[serde(rename = "refreshToken")]
    refresh_token: String,
    #[serde(rename = "expiresAt")]
    expires_at: i64,
    domain: String,
    realm: String,
}

fn build_headers(origin: &str) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    h.insert(ACCEPT, HeaderValue::from_static("application/json, text/plain, */*"));
    h.insert(ORIGIN, HeaderValue::from_str(origin).unwrap());
    h.insert(REFERER, HeaderValue::from_str(&format!("{}/", origin)).unwrap());
    h.insert(USER_AGENT, HeaderValue::from_static(CLIENT_UA));
    h.insert("X-Requested-With", HeaderValue::from_static("XMLHttpRequest"));
    h
}

fn copy_to_clipboard(text: &str) -> bool {
    if let Ok(mut child) = std::process::Command::new("xclip")
        .args(["-selection", "clipboard"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        if child.wait().map(|s| s.success()).unwrap_or(false) {
            return true;
        }
    }

    if let Ok(mut child) = std::process::Command::new("xsel")
        .args(["--clipboard", "--input"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        if child.wait().map(|s| s.success()).unwrap_or(false) {
            return true;
        }
    }

    false
}

pub fn prompt_realm_choice() -> &'static str {
    println!("{}", "── WorkBuddy 官方原生授权登录 ──".cyan().bold());
    println!("请选择登录平台与域 (Realm):");
    println!("  {} 国内版 (CN - 微信扫码授权 / www.codebuddy.cn) {}", "1)".yellow().bold(), "[默认]".green());
    println!("  {} 国际版 (Global - Google/GitHub/邮箱授权 / www.workbuddy.ai)", "2)".yellow().bold());
    print!("\n请输入选项 [1 或 2，直接回车默认 1]: ");
    let _ = io::stdout().flush();

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_ok() {
        let trimmed = input.trim().to_lowercase();
        if trimmed == "2" || trimmed == "global" || trimmed == "国际版" {
            return "global";
        }
    }
    "cn"
}

fn sync_to_official_codebuddy_settings(token: &str) {
    let settings_paths = vec![
        dirs::home_dir().map(|h| h.join(".codebuddy").join("settings.json")),
        Some(PathBuf::from("/home/root/.codebuddy/settings.json")),
        Some(PathBuf::from("/home/codes/.codebuddy/settings.json")),
    ];

    for path_opt in settings_paths.into_iter().flatten() {
        let mut obj = if path_opt.exists() {
            fs::read_to_string(&path_opt)
                .ok()
                .and_then(|c| serde_json::from_str::<Value>(&c).ok())
                .unwrap_or_else(|| json!({}))
        } else {
            json!({})
        };

        if !obj.is_object() {
            obj = json!({});
        }

        if let Some(map) = obj.as_object_mut() {
            let env_val = map.entry("env").or_insert_with(|| json!({}));
            if let Some(env_map) = env_val.as_object_mut() {
                env_map.insert("CODEBUDDY_AUTH_TOKEN".to_string(), json!(token));
            }
        }

        if let Some(parent) = path_opt.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(formatted) = serde_json::to_string_pretty(&obj) {
            let _ = fs::write(&path_opt, formatted);
        }
    }
}

pub async fn run_oauth_login(realm_arg: &str) -> Result<String, String> {
    let realm = if realm_arg.trim().is_empty() || realm_arg == "ask" {
        prompt_realm_choice()
    } else if realm_arg.eq_ignore_ascii_case("global") || realm_arg == "2" {
        "global"
    } else {
        "cn"
    };

    let (base, origin, platform_name) = if realm == "global" {
        (GLOBAL_BASE, GLOBAL_ORIGIN, "国际版 (Global - www.workbuddy.ai)")
    } else {
        (CN_BASE, CN_ORIGIN, "国内版 (CN - www.codebuddy.cn)")
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| format!("构建客户端失败: {}", e))?;

    let state_url = format!("{}/v2/plugin/auth/state?platform=CLI", base);
    let resp = client
        .post(&state_url)
        .headers(build_headers(origin))
        .json(&json!({}))
        .send()
        .await
        .map_err(|e| format!("请求 auth/state 失败: {}", e))?;

    let status = resp.status();
    let resp_text = resp.text().await.unwrap_or_default();
    let state_res: StateResponse = serde_json::from_str(&resp_text)
        .map_err(|e| format!("解析 auth/state 响应失败 [{}]: {} ({})", status, e, resp_text))?;

    if state_res.code != 0 {
        return Err(format!("获取登录 URL 失败: {:?}", state_res.msg));
    }

    let state_data = state_res.data.ok_or_else(|| "响应缺少 data 字段".to_string())?;
    let state = state_data.state;
    let auth_url = state_data
        .auth_url
        .or(state_data.url)
        .unwrap_or_else(|| format!("{}/login?platform=CLI&state={}", base, state));

    println!("\n{}", "╔══════════════════════════════════════════════════════════════╗".cyan().bold());
    println!("{}", "║               WorkBuddy 官方原生设备流授权登录               ║".cyan().bold());
    println!("{}", "╚══════════════════════════════════════════════════════════════╝".cyan().bold());
    println!("▶ 登录平台: {}", platform_name.green().bold());
    println!("▶ 授权链接 (请在浏览器中打开):\n");
    println!("  {}\n", auth_url.green().underline().bold());

    let copied = copy_to_clipboard(&auth_url);
    if copied {
        println!("  {}", "(✔ 授权链接已自动复制到系统剪贴板)".dimmed());
    }

    println!("{}", "正在等待授权完成...".cyan());
    println!("{}", "提示: 在浏览器完成登录授权后，可直接按回车或输入 y 立即检测，系统亦在后台每 2 秒自动轮询。".dimmed());

    // 轮询 token，同时监听回车立即核验
    let token_url = format!("{}/v2/plugin/auth/token?state={}", base, state);
    let mut token_data: Option<TokenData> = None;

    let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(1);
    tokio::task::spawn_blocking(move || {
        let mut line = String::new();
        while io::stdin().read_line(&mut line).is_ok() {
            let _ = tx.blocking_send(());
            line.clear();
        }
    });

    for _ in 0..300 {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(2)) => {},
            _ = rx.recv() => {
                print!("  正在核验登录状态... ");
                let _ = io::stdout().flush();
            }
        }

        let poll_resp = match client
            .get(&token_url)
            .headers(build_headers(origin))
            .send()
            .await
        {
            Ok(r) => r,
            Err(_) => continue,
        };

        if let Ok(text) = poll_resp.text().await {
            if let Ok(tok_res) = serde_json::from_str::<TokenResponse>(&text) {
                if tok_res.code == 0 {
                    if let Some(td) = tok_res.data {
                        token_data = Some(td);
                        break;
                    }
                }
            }
        }
    }

    let token_data = token_data.ok_or_else(|| "登录等待超时（10分钟未完成）".to_string())?;
    println!("\n{}", "✔ 检测到授权成功，正在拉取官方账号详情...".green().bold());

    // 拉取 account
    let account_url = format!("{}/v2/plugin/login/account?state={}", base, state);
    let mut headers = build_headers(origin);
    headers.insert(
        "Authorization",
        HeaderValue::from_str(&format!("Bearer {}", token_data.access_token)).unwrap(),
    );

    let acc_resp = client
        .get(&account_url)
        .headers(headers.clone())
        .send()
        .await
        .map_err(|e| format!("请求 login/account 失败: {}", e))?;

    let acc_text = acc_resp.text().await.unwrap_or_default();
    let acc_res: AccountResponse = serde_json::from_str(&acc_text)
        .map_err(|e| format!("解析 login/account 响应失败: {} ({})", e, acc_text))?;

    let acc_data = acc_res.data.ok_or_else(|| "无法获取账号信息".to_string())?;
    let uid = acc_data.uid;
    let nickname = acc_data.nickname.unwrap_or_else(|| uid[..8].to_string());
    let domain = acc_data.domain.unwrap_or_else(|| origin.trim_start_matches("https://").to_string());
    let enterprise_id = acc_data.enterprise_id.unwrap_or_default();

    let now_sec = chrono::Utc::now().timestamp();
    let expires_at = now_sec + token_data.expires_in;

    let auth_file_content = AuthFileModel {
        account: AuthFileAccount {
            uid: uid.clone(),
            enterprise_id: enterprise_id.clone(),
            nickname: nickname.clone(),
        },
        auth: AuthFileDetails {
            access_token: token_data.access_token.clone(),
            refresh_token: token_data.refresh_token.clone(),
            expires_at,
            domain: domain.clone(),
            realm: realm.to_string(),
        },
    };

    let json_str = serde_json::to_string_pretty(&auth_file_content)
        .map_err(|e| format!("序列化凭证失败: {}", e))?;

    // 保存到网关与本地账号目录
    let save_dirs = vec![
        PathBuf::from("/home/bin/workbuddy2api/auths"),
        PathBuf::from("/home/codes/third_party/workbuddy2api/auths"),
        dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".workbd").join("accounts"),
    ];

    for d in save_dirs {
        let _ = fs::create_dir_all(&d);
        let target_file = d.join(format!("workbuddy-{}.json", uid));
        let _ = fs::write(target_file, &json_str);
    }

    // 同步到官方 CodeBuddy CLI 配置 (~/.codebuddy/settings.json)
    sync_to_official_codebuddy_settings(&token_data.access_token);

    // 若为 CN 账号，自动执行每日签到领取积分
    if realm == "cn" {
        println!("{}", "正在为国内版账号执行每日签到领取免费积分...".dimmed());
        let checkin_url = "https://www.codebuddy.cn/v2/billing/meter/daily-checkin";
        let mut ck_headers = build_headers(CN_ORIGIN);
        ck_headers.insert(
            "Authorization",
            HeaderValue::from_str(&format!("Bearer {}", token_data.access_token)).unwrap(),
        );
        ck_headers.insert("X-User-Id", HeaderValue::from_str(&uid).unwrap());
        if !enterprise_id.is_empty() {
            ck_headers.insert("X-Enterprise-Id", HeaderValue::from_str(&enterprise_id).unwrap());
        }

        let _ = client
            .post(checkin_url)
            .headers(ck_headers)
            .json(&json!({}))
            .send()
            .await;
    }

    // 重置冷却状态并重启本地网关
    let _ = fs::write("/home/bin/workbuddy2api/data/state.json", r#"{"accounts":{}}"#);
    let _ = tokio::process::Command::new("systemctl")
        .args(["restart", "workbuddy2api"])
        .output()
        .await;

    println!("{}", "┌──────────────────────────────────────────────────────────────┐".green().bold());
    println!("│ 账号昵称: {:<50} │", nickname.yellow().bold());
    println!("│ 用户 UID: {:<50} │", uid.dimmed());
    println!("│ 登录平台: {:<50} │", platform_name.green());
    println!("│ 凭证同步: {:<50} │", "已同时持久化到 workbd、官方 CLI 及本地网关".green());
    println!("{}", "└──────────────────────────────────────────────────────────────┘".green().bold());

    Ok(nickname)
}
