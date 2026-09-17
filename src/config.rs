use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub api_base: String,
    pub api_key: String,
    pub default_model: String,
    pub system_prompt: String,
    pub temperature: f32,
    #[serde(default = "default_true")]
    pub agent_mode: bool,
    #[serde(default = "default_true")]
    pub codex_skills: bool,
    #[serde(default)]
    pub custom_skills_dirs: Vec<PathBuf>,
}

fn default_true() -> bool {
    true
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            api_base: "http://127.0.0.1:7863/v1".to_string(),
            api_key: "LOCAL_TOKEN_CHANGE_ME".to_string(),
            default_model: "deepseek-v4.1-flash".to_string(),
            system_prompt: "You are WorkBuddy Code, an expert autonomous AI software engineer. You have access to local workspace tools (bash, read_file, write_file, replace_in_file, list_dir, search_code, load_skill, search_skills). Always proactively inspect code, make changes, and verify with tests/builds via bash.".to_string(),
            temperature: 0.5,
            agent_mode: true,
            codex_skills: true,
            custom_skills_dirs: Vec::new(),
        }
    }
}

impl AppConfig {
    pub fn config_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".workbd")
    }

    pub fn config_file_path() -> PathBuf {
        Self::config_dir().join("config.json")
    }

    pub fn history_file_path() -> PathBuf {
        Self::config_dir().join("history.txt")
    }

    pub fn build_effective_system_prompt(&self) -> String {
        let mut sys = self.system_prompt.clone();
        if self.codex_skills {
            let reg = crate::skills::SkillRegistry::new(self.custom_skills_dirs.clone());
            let summary = reg.format_system_prompt_skills_summary();
            sys.push_str(&summary);
        }
        sys
    }

    pub fn load() -> Self {
        let path = Self::config_file_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(mut config) = serde_json::from_str::<AppConfig>(&content) {
                    config.apply_env_overrides();
                    return config;
                }
            }
        }

        let mut config = AppConfig::default();
        config.apply_env_overrides();
        let _ = config.save();
        config
    }

    pub fn save(&self) -> std::io::Result<()> {
        let dir = Self::config_dir();
        if !dir.exists() {
            fs::create_dir_all(&dir)?;
        }
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        fs::write(Self::config_file_path(), content)
    }

    fn apply_env_overrides(&mut self) {
        if let Ok(val) = std::env::var("WORKBD_API_BASE") {
            if !val.trim().is_empty() {
                self.api_base = val.trim().to_string();
            }
        }
        if let Ok(val) = std::env::var("WORKBD_API_KEY") {
            if !val.trim().is_empty() {
                self.api_key = val.trim().to_string();
            }
        }
        if let Ok(val) = std::env::var("WORKBD_MODEL") {
            if !val.trim().is_empty() {
                self.default_model = val.trim().to_string();
            }
        }
    }
}
