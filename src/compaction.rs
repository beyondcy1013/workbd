use crate::types::ChatMessage;
use colored::*;

/// 估算当前消息列表占用的近似 Token 数量
pub fn estimate_tokens(messages: &[ChatMessage]) -> usize {
    let mut total_chars = 0;
    for m in messages {
        let text = m.text_content();
        if !text.is_empty() {
            total_chars += text.len();
        }
        if let Some(ref r) = m.reasoning_content {
            total_chars += r.len();
        }
        if let Some(ref tc) = m.tool_calls {
            for t in tc {
                total_chars += t.function.name.len() + t.function.arguments.len();
            }
        }
    }
    // 粗略按 3 个字符 1 个 Token 估算（兼顾中英文与代码）
    total_chars / 3
}

/// 检查是否需要触发上下文压缩修剪
pub fn should_compact(messages: &[ChatMessage], threshold_tokens: usize, threshold_turns: usize) -> bool {
    if messages.len() >= threshold_turns {
        return true;
    }
    estimate_tokens(messages) >= threshold_tokens
}

/// 对过长上下文执行智能压缩与摘要修剪
pub fn compact_context(messages: &mut Vec<ChatMessage>) -> bool {
    // 至少需要保留系统提示词 + 至少 4 条近期消息，若消息少于 8 条则不压缩
    if messages.len() < 8 {
        return false;
    }

    let has_system = !messages.is_empty() && messages[0].role == "system";
    let start_idx = if has_system { 1 } else { 0 };
    let keep_recent_count = 4;
    let end_idx = messages.len().saturating_sub(keep_recent_count);

    if end_idx <= start_idx {
        return false;
    }

    let before_tokens = estimate_tokens(messages);
    let slice_to_compact = &messages[start_idx..end_idx];

    // 提取被修剪轮次的核心摘要
    let mut user_goals = Vec::new();
    let mut tool_executions = Vec::new();

    for m in slice_to_compact {
        match m.role.as_str() {
            "user" => {
                let c = m.text_content();
                if !c.is_empty() {
                    let preview = c.lines().next().unwrap_or(c).chars().take(80).collect::<String>();
                    user_goals.push(preview);
                }
            }
            "assistant" => {
                if let Some(ref tc) = m.tool_calls {
                    for t in tc {
                        tool_executions.push(format!("{}({})", t.function.name, truncate_str(&t.function.arguments, 40)));
                    }
                }
            }
            "tool" => {
                // 工具输出内容通常较大，直接在此截断归纳
            }
            _ => {}
        }
    }

    let summary_content = format!(
        "[历史上下文智能压缩快照 / Context Compaction Snapshot]\n\
         前期累计完成 {} 轮对话与工具调用。\n\
         - 历史涉及任务线: {}\n\
         - 曾调用的工具序列: {}\n\
         - 提示: 早期历史消息的冗长工具原始返回与中间思考已被安全提炼压缩，核心状态与上下文已保留。",
        slice_to_compact.len(),
        if user_goals.is_empty() { "日常问答与代码维护".to_string() } else { user_goals.join(" ➔ ") },
        if tool_executions.is_empty() { "无".to_string() } else { tool_executions.into_iter().take(8).collect::<Vec<_>>().join(", ") }
    );

    let summary_message = ChatMessage::system(&summary_content);

    // 重新拼接消息
    let mut new_messages = Vec::new();
    if has_system {
        new_messages.push(messages[0].clone());
    }
    new_messages.push(summary_message);
    new_messages.extend_from_slice(&messages[end_idx..]);

    let after_tokens = estimate_tokens(&new_messages);
    *messages = new_messages;

    println!(
        "{} 已对历史上下文执行智能修剪压缩 (Token 估算: {} ➜ {})",
        "⚡ [Context Compaction]".cyan().bold(),
        before_tokens.to_string().yellow(),
        after_tokens.to_string().green().bold()
    );

    true
}

fn truncate_str(s: &str, max: usize) -> String {
    let clean = s.replace('\n', " ");
    if clean.chars().count() > max {
        let truncated: String = clean.chars().take(max).collect();
        format!("{}...", truncated)
    } else {
        clean
    }
}
