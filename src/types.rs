use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
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
            content: Some(content.into()),
            reasoning_content: None,
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: Some(content.into()),
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
            content,
            reasoning_content: reasoning,
            tool_calls,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn tool(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: "tool".into(),
            content: Some(content.into()),
            reasoning_content: None,
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
            name: None,
        }
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
