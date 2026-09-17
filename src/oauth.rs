use colored::*;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, CONTENT_TYPE, ORIGIN, REFERER, USER_AGENT};
use serde::{Deserialize, Serialize};
use std::fs;
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
    code: i32,
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

pub async fn run_oauth_login(realm: &str) -> Result<String, String> {
    let realm = if realm.eq_ignore_ascii_case("global") {
        "global"
    } else {
        "cn"
    };

    let (base, origin) = if realm == "global" {
        (GLOBAL_BASE, GLOBAL_ORIGIN)
    } else {
        (CN_BASE, CN_ORIGIN)
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| format!("构建客户端失败: {}", e))?;

    let state_url = format!("{}/v2/plugin/auth/state?platform=CLI", base);
    let resp = client
        .post(&state_url)
        .headers(build_headers(origin))
        .json(&serde_json::json!({}))
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

    println!("{}", "============================================================".cyan().bold());
    println!("{}", format!("  WorkBuddy OAuth 登录 [{}]", realm.to_uppercase()).cyan().bold());
    println!("{}", "============================================================".cyan().bold());
    println!("\n请在浏览器中打开以下链接进行登录（扫码或账号授权）：\n");
    println!("  {}\n", auth_url.green().underline().bold());
    println!("{}", "正在等待浏览器登录完成（每 2 秒轮询一次，按 Ctrl+C 可取消）...".dimmed());

    // 轮询 token
    let token_url = format!("{}/v2/plugin/auth/token?state={}", base, state);
    let mut token_data: Option<TokenData> = None;

    for _ in 0..150 {
        tokio::time::sleep(Duration::from_secs(2)).await;

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

    let token_data = token_data.ok_or_else(|| "登录等待超时（5分钟未完成）".to_string())?;
    println!("{}", "✔ 检测到授权成功，正在拉取账号详情...".green());

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

    // 保存到网关 auths 目录
    let save_dirs = vec![
        PathBuf::from("/home/bin/workbuddy2api/auths"),
        PathBuf::from("/home/codes/third_party/workbuddy2api/auths"),
    ];

    for d in save_dirs {
        let _ = fs::create_dir_all(&d);
        let target_file = d.join(format!("workbuddy-{}.json", uid));
        let _ = fs::write(target_file, &json_str);
    }

    // 若为 CN 账号，自动触发每日签到领积分
    if realm == "cn" {
        println!("{}", "正在为国内版账号执行每日签到领取积分...".dimmed());
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
            .json(&serde_json::json!({}))
            .send()
            .await;
    }

    // 重置冷却状态并重启网关
    let _ = fs::write("/home/bin/workbuddy2api/data/state.json", r#"{"accounts":{}}"#);
    let _ = tokio::process::Command::new("systemctl")
        .args(["restart", "workbuddy2api"])
        .output()
        .await;

    println!("{}", format!("✔ 登录成功！账号 [{}] 已成功保存并挂载到网关。", nickname).green().bold());
    println!("{}", "✔ 网关已自动重载新凭据并解除冷却。".green());

    Ok(nickname)
}
