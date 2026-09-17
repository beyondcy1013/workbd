use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub skill_md_path: PathBuf,
    pub skill_dir: PathBuf,
    pub category: String,
}

#[derive(Clone)]
pub struct SkillRegistry {
    pub search_paths: Vec<PathBuf>,
}

impl Default for SkillRegistry {
    fn default() -> Self {
        let mut paths = Vec::new();
        // 1. Codex 活跃技能目录
        let codex_skills = PathBuf::from("/home/root/.codex/skills");
        if codex_skills.exists() {
            paths.push(codex_skills);
        }
        // 2. 项目专属技能目录
        let agent_skills = PathBuf::from("/home/codes/.agents/skills");
        if agent_skills.exists() {
            paths.push(agent_skills);
        }
        // 3. 离线全量技能库
        let offline_skills = PathBuf::from("/home/codes/offlineSkills");
        if offline_skills.exists() {
            paths.push(offline_skills);
        }

        Self { search_paths: paths }
    }
}

impl SkillRegistry {
    pub fn new(custom_paths: Vec<PathBuf>) -> Self {
        let mut reg = Self::default();
        for p in custom_paths {
            if !reg.search_paths.contains(&p) && p.exists() {
                reg.search_paths.insert(0, p);
            }
        }
        reg
    }

    pub fn list_active_skills(&self) -> Vec<Skill> {
        let mut skills = Vec::new();
        let active_dirs = vec![
            PathBuf::from("/home/root/.codex/skills"),
            PathBuf::from("/home/codes/.agents/skills"),
        ];

        for dir in active_dirs {
            if !dir.exists() {
                continue;
            }
            self.scan_directory(&dir, &mut skills, 2, "active");
        }

        skills.sort_by(|a, b| a.name.cmp(&b.name));
        skills
    }

    pub fn search_all_skills(&self, query: &str) -> Vec<Skill> {
        let mut all = Vec::new();
        let query_lower = query.to_lowercase();

        for base in &self.search_paths {
            let cat = if base.to_string_lossy().contains("offlineSkills") {
                "offline"
            } else {
                "active"
            };
            self.scan_directory(base, &mut all, 4, cat);
        }

        all.into_iter()
            .filter(|s| {
                s.name.to_lowercase().contains(&query_lower)
                    || s.description.to_lowercase().contains(&query_lower)
                    || s.category.to_lowercase().contains(&query_lower)
            })
            .collect()
    }

    pub fn find_skill(&self, name: &str) -> Option<Skill> {
        let name_lower = name.to_lowercase();
        let active = self.list_active_skills();

        // 1. 精确匹配活跃技能
        if let Some(s) = active.iter().find(|s| s.name.eq_ignore_ascii_case(name)) {
            return Some(s.clone());
        }

        // 2. 前缀/模糊匹配活跃技能
        if let Some(s) = active.iter().find(|s| s.name.to_lowercase().contains(&name_lower)) {
            return Some(s.clone());
        }

        // 3. 在离线库中搜索
        let matches = self.search_all_skills(name);
        matches.first().cloned()
    }

    pub fn load_skill_markdown(&self, skill: &Skill) -> Result<String, String> {
        let raw = fs::read_to_string(&skill.skill_md_path)
            .map_err(|e| format!("无法读取技能文档 ({}): {}", skill.skill_md_path.display(), e))?;

        let mut out = format!("# Skill: {}\n", skill.name);
        out.push_str(&format!("**Description**: {}\n", skill.description));
        out.push_str(&format!("**Location**: {}\n\n", skill.skill_dir.display()));

        // 列出技能目录下的可执行脚本
        let scripts_dir = skill.skill_dir.join("scripts");
        if scripts_dir.exists() {
            out.push_str("### 关联脚本 (可直接使用 bash 执行):\n");
            if let Ok(entries) = fs::read_dir(&scripts_dir) {
                for e in entries.flatten() {
                    let p = e.path();
                    if p.is_file() {
                        out.push_str(&format!("- `{}`\n", p.display()));
                    }
                }
            }
            out.push('\n');
        }

        out.push_str("### 技能操作指引:\n");
        // 去除 frontmatter 后的正文
        let body = if let Some(stripped) = strip_frontmatter(&raw) {
            stripped
        } else {
            &raw
        };
        out.push_str(body);

        Ok(out)
    }

    pub fn format_system_prompt_skills_summary(&self) -> String {
        let active = self.list_active_skills();
        if active.is_empty() {
            return String::new();
        }

        let mut s = String::from("\n## 可用 Codex / 系统技能 (Codex Skills)\n");
        s.push_str("当用户任务符合技能场景时，你可以调用 `load_skill` 工具加载完整操作指引，并通过 `bash` 调用其脚本：\n");

        for skill in active {
            s.push_str(&format!("- **{}**: {} (路径: {})\n", skill.name, skill.description, skill.skill_dir.display()));
        }

        s
    }

    fn scan_directory(&self, dir: &Path, results: &mut Vec<Skill>, max_depth: usize, category: &str) {
        if max_depth == 0 {
            return;
        }

        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let skill_md = path.join("SKILL.md");
                if skill_md.exists() {
                    if let Some(skill) = parse_skill_md(&skill_md, &path, category) {
                        if !results.iter().any(|existing| existing.name == skill.name) {
                            results.push(skill);
                        }
                    }
                } else {
                    self.scan_directory(&path, results, max_depth - 1, category);
                }
            }
        }
    }
}

fn parse_skill_md(skill_md: &Path, skill_dir: &Path, category: &str) -> Option<Skill> {
    let content = fs::read_to_string(skill_md).ok()?;
    let (name, desc) = extract_frontmatter(&content);

    let default_name = skill_dir.file_name()?.to_string_lossy().to_string();
    let name = name.unwrap_or(default_name);
    let description = desc.unwrap_or_else(|| "无说明".to_string());

    Some(Skill {
        name,
        description,
        skill_md_path: skill_md.to_path_buf(),
        skill_dir: skill_dir.to_path_buf(),
        category: category.to_string(),
    })
}

fn extract_frontmatter(content: &str) -> (Option<String>, Option<String>) {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() || lines[0].trim() != "---" {
        return (None, None);
    }

    let mut name = None;
    let mut desc = None;

    for line in lines.iter().skip(1) {
        let trimmed = line.trim();
        if trimmed == "---" {
            break;
        }
        if let Some(val) = trimmed.strip_prefix("name:") {
            name = Some(val.trim().trim_matches('"').trim_matches('\'').to_string());
        } else if let Some(val) = trimmed.strip_prefix("description:") {
            desc = Some(val.trim().trim_matches('"').trim_matches('\'').to_string());
        }
    }

    (name, desc)
}

fn strip_frontmatter(content: &str) -> Option<&str> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }

    let after_first = &trimmed[3..];
    if let Some(end_idx) = after_first.find("---") {
        let body = &after_first[end_idx + 3..];
        Some(body.trim_start())
    } else {
        None
    }
}
