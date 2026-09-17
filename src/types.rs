use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentPart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: ImageUrlDef },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrlDef {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<MessageContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: Some(MessageContent::Text(content.into())),
            reasoning_content: None,
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: Some(MessageContent::Text(content.into())),
            reasoning_content: None,
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn user_with_image(text: impl Into<String>, mime_type: &str, base64_data: &str) -> Self {
        let data_url = format!("data:{};base64,{}", mime_type, base64_data);
        let parts = vec![
            ContentPart::Text {
                text: text.into(),
            },
            ContentPart::ImageUrl {
                image_url: ImageUrlDef {
                    url: data_url,
                    detail: Some("auto".to_string()),
                },
            },
        ];
        Self {
            role: "user".into(),
            content: Some(MessageContent::Parts(parts)),
            reasoning_content: None,
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn assistant(
        content: Option<String>,
        reasoning: Option<String>,
        tool_calls: Option<Vec<ToolCall>>,
    ) -> Self {
        Self {
            role: "assistant".into(),
            content: content.map(MessageContent::Text),
            reasoning_content: reasoning,
            tool_calls,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn tool(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: "tool".into(),
            content: Some(MessageContent::Text(content.into())),
            reasoning_content: None,
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
            name: None,
        }
    }

    pub fn text_content(&self) -> &str {
        match &self.content {
            Some(MessageContent::Text(s)) => s,
            Some(MessageContent::Parts(parts)) => {
                for p in parts {
                    if let ContentPart::Text { text } = p {
                        return text;
                    }
                }
                ""
            }
            None => "",
        }
    }

    pub fn set_text_content(&mut self, text: impl Into<String>) {
        self.content = Some(MessageContent::Text(text.into()));
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionDefinition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatCompletionChunk {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub choices: Vec<ChunkChoice>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChunkChoice {
    pub delta: ChunkDelta,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ChunkDelta {
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub reasoning_content: Option<String>,
    #[serde(default)]
    pub tool_calls: Option<Vec<ChunkToolCall>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChunkToolCall {
    pub index: usize,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default, rename = "type")]
    pub tool_type: Option<String>,
    #[serde(default)]
    pub function: Option<ChunkFunctionCall>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChunkFunctionCall {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub arguments: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModelListResponse {
    pub data: Vec<ModelData>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelData {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub credits: Option<String>,
    #[serde(default)]
    pub vendor: Option<String>,
    #[serde(default)]
    pub context_length: Option<i64>,
}

#[allow(dead_code)]
impl ModelData {
    pub fn clean_id(&self) -> &str {
        self.id
            .strip_prefix("global:")
            .or_else(|| self.id.strip_prefix("cn:"))
            .unwrap_or(&self.id)
    }

    pub fn display_name(&self) -> String {
        if let Some(ref n) = self.name {
            if !n.trim().is_empty() {
                return n.clone();
            }
        }
        let clean = self.clean_id();
        crate::models_catalog::find_official_model(clean)
            .map(|def| def.name.to_string())
            .unwrap_or_else(|| clean.to_string())
    }

    pub fn display_credits(&self) -> String {
        if let Some(ref c) = self.credits {
            if !c.trim().is_empty() && c != "-" {
                return c.clone();
            }
        }
        let clean = self.clean_id();
        crate::models_catalog::find_official_model(clean)
            .map(|def| def.credits.to_string())
            .unwrap_or_else(|| "-".to_string())
    }

    pub fn display_description(&self) -> String {
        if let Some(ref d) = self.description {
            if !d.trim().is_empty() {
                return d.clone();
            }
        }
        let clean = self.clean_id();
        crate::models_catalog::find_official_model(clean)
            .map(|def| def.description.to_string())
            .unwrap_or_default()
    }
}
