use crate::types::{
    ChatCompletionChunk, ChatCompletionRequest, ChatMessage, ModelData, ModelListResponse,
    ToolDefinition,
};
use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct ApiClient {
    client: reqwest::Client,
    api_base: String,
    api_key: String,
}

#[derive(Debug, Clone)]
pub enum StreamEvent {
    Content(String),
    Reasoning(String),
    ToolCallDelta {
        index: usize,
        id: Option<String>,
        name: Option<String>,
        arguments: Option<String>,
    },
    Done,
    Error(String),
}

impl ApiClient {
    pub fn new(api_base: impl Into<String>, api_key: impl Into<String>) -> Self {
        let mut api_base = api_base.into();
        if api_base.ends_with('/') {
            api_base.pop();
        }

        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(300))
                .build()
                .unwrap_or_default(),
            api_base,
            api_key: api_key.into(),
        }
    }

    fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        if !self.api_key.trim().is_empty() {
            if let Ok(val) = HeaderValue::from_str(&format!("Bearer {}", self.api_key.trim())) {
                headers.insert(AUTHORIZATION, val);
            }
        }
        headers
    }

    pub async fn list_models(&self) -> Result<Vec<ModelData>, String> {
        let url = format!("{}/models", self.api_base);
        let resp = self
            .client
            .get(&url)
            .headers(self.headers())
            .send()
            .await
            .map_err(|e| format!("无法连接至模型接口 ({}): {}", url, e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_text = resp.text().await.unwrap_or_default();
            return Err(format!("接口返回错误 [{status}]: {err_text}"));
        }

        let resp_json = resp
            .json::<ModelListResponse>()
            .await
            .map_err(|e| format!("解析模型列表失败: {}", e))?;

        Ok(resp_json.data)
    }

    pub async fn resolve_model_id(&self, model: &str) -> String {
        let clean_target = match model.trim().to_lowercase().as_str() {
            "auto" => "default-model",
            "fast" => "fast-model",
            "balanced" => "balanced-model",
            "primary" => "primary-model",
            "deep" => "deep-model",
            "deepseek" | "ds" => "deepseek-v4.1-flash",
            "kimi" => "kimi-k3",
            "glm" => "glm-5.3",
            "gemini" => "gemini-3.5-flash",
            _ => model.trim(),
        };

        if let Ok(models) = self.list_models().await {
            // 1. clean_id 完全匹配
            if let Some(m) = models.iter().find(|m| m.clean_id().eq_ignore_ascii_case(clean_target)) {
                return m.id.clone();
            }
            // 2. 原始 ID 完全匹配
            if let Some(m) = models.iter().find(|m| m.id == model) {
                return m.id.clone();
            }
            // 3. global / cn 前缀匹配
            let global_id = format!("global:{}", clean_target);
            if let Some(m) = models.iter().find(|m| m.id == global_id) {
                return m.id.clone();
            }
            let cn_id = format!("cn:{}", clean_target);
            if let Some(m) = models.iter().find(|m| m.id == cn_id) {
                return m.id.clone();
            }
            // 4. 包含匹配
            if let Some(m) = models.iter().find(|m| {
                m.clean_id().contains(clean_target)
                    || m.display_name().to_lowercase().contains(&clean_target.to_lowercase())
            }) {
                return m.id.clone();
            }
        }
        model.to_string()
    }

    pub async fn stream_chat(
        &self,
        messages: Vec<ChatMessage>,
        model: &str,
        temperature: Option<f32>,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<mpsc::Receiver<StreamEvent>, String> {
        let actual_model = self.resolve_model_id(model).await;
        let url = format!("{}/chat/completions", self.api_base);
        let req_body = ChatCompletionRequest {
            model: actual_model,
            messages,
            stream: true,
            temperature,
            tools,
        };

        let resp = self
            .client
            .post(&url)
            .headers(self.headers())
            .json(&req_body)
            .send()
            .await
            .map_err(|e| format!("无法连接至网关 ({}): {}", url, e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_text = resp.text().await.unwrap_or_default();
            // 尝试解析嵌套的 JSON 错误信息
            let mut extracted_msg = err_text.clone();
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&err_text) {
                if let Some(msg) = v.pointer("/error/message").and_then(|s| s.as_str()) {
                    extracted_msg = msg.to_string();
                    if let Ok(nested) = serde_json::from_str::<serde_json::Value>(msg) {
                        if let Some(inner) = nested.pointer("/error/data/msg").and_then(|s| s.as_str()) {
                            extracted_msg = inner.to_string();
                        } else if let Some(inner) = nested.pointer("/msg").and_then(|s| s.as_str()) {
                            extracted_msg = inner.to_string();
                        }
                    }
                }
            }

            if extracted_msg.contains("Credits exhausted") || extracted_msg.contains("14018") {
                return Err(format!(
                    "上游账号积分已耗尽 (Credits exhausted)。当前绑定的 WorkBuddy 账号可用额度为 0，请前往官网充值积分或使用 ./login.sh 登录新账号。"
                ));
            }
            if extracted_msg.contains("all accounts are temporarily unavailable") {
                return Err(format!(
                    "网关无可用账号 (503)。当前账号池中无有效账号或处于冷却中，可通过 /status 查看账号状态。"
                ));
            }

            return Err(format!("上游报错 [{status}]: {extracted_msg}"));
        }

        let (tx, rx) = mpsc::channel(100);
        let mut byte_stream = resp.bytes_stream();

        tokio::spawn(async move {
            let mut buffer = String::new();

            while let Some(chunk_result) = byte_stream.next().await {
                match chunk_result {
                    Ok(bytes) => {
                        let text = String::from_utf8_lossy(&bytes);
                        buffer.push_str(&text);

                        while let Some(pos) = buffer.find('\n') {
                            let line = buffer[..pos].trim().to_string();
                            buffer.drain(..=pos);

                            if line.is_empty() || line.starts_with(':') {
                                continue;
                            }

                            if let Some(data) = line.strip_prefix("data:") {
                                let data = data.trim();
                                if data == "[DONE]" {
                                    let _ = tx.send(StreamEvent::Done).await;
                                    return;
                                }

                                if let Ok(chunk) = serde_json::from_str::<ChatCompletionChunk>(data) {
                                    for choice in chunk.choices {
                                        if let Some(reasoning) = choice.delta.reasoning_content {
                                            if !reasoning.is_empty() {
                                                let _ = tx.send(StreamEvent::Reasoning(reasoning)).await;
                                            }
                                        }
                                        if let Some(content) = choice.delta.content {
                                            if !content.is_empty() {
                                                let _ = tx.send(StreamEvent::Content(content)).await;
                                            }
                                        }
                                        if let Some(tcs) = choice.delta.tool_calls {
                                            for tc in tcs {
                                                let name = tc.function.as_ref().and_then(|f| f.name.clone());
                                                let arguments = tc.function.as_ref().and_then(|f| f.arguments.clone());
                                                let _ = tx.send(StreamEvent::ToolCallDelta {
                                                    index: tc.index,
                                                    id: tc.id,
                                                    name,
                                                    arguments,
                                                }).await;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(StreamEvent::Error(format!("流式传输错误: {}", e))).await;
                        return;
                    }
                }
            }

            let _ = tx.send(StreamEvent::Done).await;
        });

        Ok(rx)
    }
}
