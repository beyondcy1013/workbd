use colored::*;
use crate::types::ModelData;

/// 官方 WorkBuddy / CodeBuddy 模型目录及元数据定义（对齐官方 product.json 与 /v2/models 规范）
#[derive(Debug, Clone)]
pub struct OfficialModelDef {
    pub id: &'static str,
    pub name: &'static str,
    pub credits: &'static str,
    pub description: &'static str,
    pub category: &'static str, // "preset", "flagship", "other"
    pub alias: Option<&'static str>,
}

pub const OFFICIAL_MODELS: &[OfficialModelDef] = &[
    // ── 核心预设分档模型 (Core Presets) ──
    OfficialModelDef {
        id: "default-model",
        name: "Auto",
        credits: "x0.79 credits",
        description: "优秀的编码模型，日常工作智能分流首选",
        category: "preset",
        alias: Some("auto"),
    },
    OfficialModelDef {
        id: "fast-model",
        name: "Fast",
        credits: "x0.34 credits",
        description: "响应快，极速推理，适合轻量与简单任务",
        category: "preset",
        alias: Some("fast"),
    },
    OfficialModelDef {
        id: "balanced-model",
        name: "Balanced",
        credits: "x0.59 credits",
        description: "速度与质量兼顾，日常复杂编程开发首选",
        category: "preset",
        alias: Some("balanced"),
    },
    OfficialModelDef {
        id: "primary-model",
        name: "Primary",
        credits: "x3.31 credits",
        description: "高质量输出，擅长胜任极高难度复杂任务",
        category: "preset",
        alias: Some("primary"),
    },
    OfficialModelDef {
        id: "deep-model",
        name: "Deep",
        credits: "x3.33 credits",
        description: "深度思维链推理，适合深度架构分析与算法难题",
        category: "preset",
        alias: Some("deep"),
    },

    // ── 深度思考与热门旗舰模型 (Flagship & Reasoning Models) ──
    OfficialModelDef {
        id: "deepseek-v4.1-flash",
        name: "DeepSeek-V4.1-Flash",
        credits: "免费 x0.00",
        description: "高性价比推理模型，支持超长思维链与深度推导",
        category: "flagship",
        alias: Some("deepseek"),
    },
    OfficialModelDef {
        id: "gpt-6-astra",
        name: "GPT-6-Astra",
        credits: "x6.67 credits",
        description: "OpenAI 旗舰模型，擅长超复杂深度推理与长程任务",
        category: "flagship",
        alias: None,
    },
    OfficialModelDef {
        id: "gpt-5.6-sol",
        name: "GPT-5.6-Sol",
        credits: "x3.47 credits",
        description: "OpenAI 旗舰模型，擅长复杂推理与长程任务",
        category: "flagship",
        alias: None,
    },
    OfficialModelDef {
        id: "gpt-5.6-terra",
        name: "GPT-5.6-Terra",
        credits: "x1.39 credits",
        description: "OpenAI 均衡模型，兼顾能力、速度与成本",
        category: "flagship",
        alias: None,
    },
    OfficialModelDef {
        id: "gpt-5.6-luna",
        name: "GPT-5.6-Luna",
        credits: "x0.14 credits",
        description: "OpenAI 轻量模型，响应快速，适合日常任务",
        category: "flagship",
        alias: None,
    },
    OfficialModelDef {
        id: "gpt-5.5",
        name: "GPT-5.5",
        credits: "x3.31 credits",
        description: "OpenAI 旗舰编码模型，擅长长程工程任务",
        category: "flagship",
        alias: None,
    },
    OfficialModelDef {
        id: "gpt-5.4",
        name: "GPT-5.4",
        credits: "x1.65 credits",
        description: "OpenAI 旗舰编码模型，长程上下文能力出众",
        category: "flagship",
        alias: None,
    },
    OfficialModelDef {
        id: "gpt-5.3-codex",
        name: "GPT-5.3-Codex",
        credits: "x1.25 credits",
        description: "OpenAI 代码专用模型，非常擅长处理复杂编码任务",
        category: "flagship",
        alias: None,
    },
    OfficialModelDef {
        id: "gemini-3.5-flash",
        name: "Gemini-3.5-Flash",
        credits: "x0.99 credits",
        description: "Google 均衡模型，多模态与日常开发首选",
        category: "flagship",
        alias: Some("gemini"),
    },
    OfficialModelDef {
        id: "glm-5.3",
        name: "GLM-5.3",
        credits: "x0.79 credits",
        description: "能力均衡，适合日常编程与代码分析",
        category: "flagship",
        alias: Some("glm"),
    },
    OfficialModelDef {
        id: "glm-5.2",
        name: "GLM-5.2",
        credits: "x0.79 credits",
        description: "1M 超大上下文，擅长超长代码库全局理解",
        category: "flagship",
        alias: None,
    },
    OfficialModelDef {
        id: "kimi-k3",
        name: "Kimi-K3",
        credits: "x1.62 credits",
        description: "擅长长程自主任务，前端开发与科研推理表现出色",
        category: "flagship",
        alias: Some("kimi"),
    },
    OfficialModelDef {
        id: "kimi-k2.8-preview",
        name: "Kimi-K2.8-Preview",
        credits: "x0.77 credits",
        description: "擅长处理复杂的长程自主任务，前端与科研推理出众",
        category: "flagship",
        alias: None,
    },
    OfficialModelDef {
        id: "kimi-k2.6",
        name: "Kimi-K2.6",
        credits: "x0.52 credits",
        description: "多模态模型，支持视觉与复杂指令",
        category: "flagship",
        alias: None,
    },
    OfficialModelDef {
        id: "hy4-preview",
        name: "Hy4-Preview",
        credits: "免费 x0.00",
        description: "混元最新思考模型，具有增强的代码推理能力",
        category: "flagship",
        alias: None,
    },
    OfficialModelDef {
        id: "hy3",
        name: "Hy3",
        credits: "免费 x0.00",
        description: "混元思考模型，深度推理与免费配额",
        category: "flagship",
        alias: None,
    },
    OfficialModelDef {
        id: "minimax-m3",
        name: "MiniMax-M3",
        credits: "x0.25 credits",
        description: "原生多模态，擅长代码及智能体任务",
        category: "flagship",
        alias: None,
    },
];

pub fn find_official_model(clean_id: &str) -> Option<&'static OfficialModelDef> {
    let lower = clean_id.to_lowercase();
    OFFICIAL_MODELS.iter().find(|m| {
        m.id.eq_ignore_ascii_case(clean_id)
            || m.name.eq_ignore_ascii_case(clean_id)
            || (clean_id.eq_ignore_ascii_case("auto") && m.id == "default-model")
            || (clean_id.eq_ignore_ascii_case("fast") && m.id == "fast-model")
            || (clean_id.eq_ignore_ascii_case("balanced") && m.id == "balanced-model")
            || (clean_id.eq_ignore_ascii_case("primary") && m.id == "primary-model")
            || (clean_id.eq_ignore_ascii_case("deep") && m.id == "deep-model")
            || (lower.contains("deepseek") && m.id == "deepseek-v4.1-flash")
    })
}

pub fn is_model_selected(m: &ModelData, current_model: &str) -> bool {
    let clean = m.clean_id();
    if clean.eq_ignore_ascii_case(current_model) || m.id.eq_ignore_ascii_case(current_model) {
        return true;
    }
    let cur_clean = current_model
        .strip_prefix("global:")
        .or_else(|| current_model.strip_prefix("cn:"))
        .unwrap_or(current_model);
    if clean.eq_ignore_ascii_case(cur_clean) {
        return true;
    }
    match cur_clean.to_lowercase().as_str() {
        "auto" => clean == "default-model",
        "fast" => clean == "fast-model",
        "balanced" => clean == "balanced-model",
        "primary" => clean == "primary-model",
        "deep" => clean == "deep-model",
        "deepseek" | "ds" => clean == "deepseek-v4.1-flash",
        "kimi" => clean == "kimi-k3",
        "glm" => clean == "glm-5.3",
        "gemini" => clean == "gemini-3.5-flash",
        _ => false,
    }
}

/// 按照官方规范展示模型列表（分组显示：核心预设分档、深度推理旗舰、扩展模型）
pub fn display_models_catalog(models: &[ModelData], current_model: &str) {
    let mut presets = Vec::new();
    let mut flagships = Vec::new();
    let mut others = Vec::new();

    // 用于去重同一 clean_id（例如避免重复展示 -sg 后缀或重复前缀）
    let mut seen_ids = std::collections::HashSet::new();

    for m in models {
        let clean = m.clean_id();
        if !seen_ids.insert(clean.to_string()) {
            continue;
        }

        if let Some(def) = find_official_model(clean) {
            if def.category == "preset" {
                presets.push(m);
            } else {
                flagships.push(m);
            }
        } else {
            others.push(m);
        }
    }

    // 1. 打印官方核心预设分档
    println!(
        "\n{}",
        "── 🌟 官方核心预设分档 (Core Presets / 智能分流) ──".cyan().bold()
    );
    for m in presets {
        print_model_item(m, current_model);
    }

    // 2. 打印深度思考与旗舰模型
    println!(
        "\n{}",
        "── 🚀 深度思考与热门旗舰模型 (Flagship & Reasoning Models) ──".cyan().bold()
    );
    for m in flagships {
        print_model_item(m, current_model);
    }

    // 3. 其他可用模型（如有）
    if !others.is_empty() {
        println!(
            "\n{}",
            "── 📦 扩展与其他可用模型 (Other Available Models) ──".cyan().bold()
        );
        for m in others {
            print_model_item(m, current_model);
        }
    }

    println!(
        "\n{}",
        "💡 切换指令: /model <名称或别名>，例如 /model auto 或 /model deepseek 或 /model gpt-5.6-sol"
            .dimmed()
    );
}

fn print_model_item(m: &ModelData, current_model: &str) {
    let clean = m.clean_id();
    let is_current = is_model_selected(m, current_model);
    let mark = if is_current {
        "● [当前]".green().bold()
    } else {
        "○".dimmed()
    };

    let def = find_official_model(clean);
    let display_name = def.map(|d| d.name).unwrap_or(&clean);
    let credits = def.map(|d| d.credits).unwrap_or("-");
    let desc = def.map(|d| d.description).unwrap_or("");
    let alias_tag = def
        .and_then(|d| d.alias)
        .map(|a| format!("(快捷: {})", a).cyan())
        .unwrap_or_else(|| "".normal());

    let credits_colored = if credits.contains("免费") || credits.contains("0.00") {
        format!("[{}]", credits).green().bold()
    } else {
        format!("[{}]", credits).yellow()
    };

    println!(
        "  {} {:<22} {:<24} {:<15} {} {}",
        mark,
        display_name.bold(),
        clean.dimmed(),
        credits_colored,
        desc.normal(),
        alias_tag
    );
}
