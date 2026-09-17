use colored::*;

#[derive(Debug, Clone)]
pub struct DiffStats {
    pub added: usize,
    pub deleted: usize,
    pub colored_diff: String,
}

/// 纯 Rust LCS 差异计算算法，生成带语法高亮和行号的 Unified Diff
pub fn compute_diff(filename: &str, old_content: &str, new_content: &str) -> DiffStats {
    let old_lines: Vec<&str> = old_content.lines().collect();
    let new_lines: Vec<&str> = new_content.lines().collect();

    // 如果新旧完全一致
    if old_lines == new_lines {
        return DiffStats {
            added: 0,
            deleted: 0,
            colored_diff: "(文件内容无变更)".dimmed().to_string(),
        };
    }

    // 计算 LCS 矩阵
    let n = old_lines.len();
    let m = new_lines.len();

    // 限制过大文件的 LCS 计算防卡顿
    if n * m > 4_000_000 {
        return fast_diff_fallback(filename, &old_lines, &new_lines);
    }

    let mut dp = vec![vec![0u32; m + 1]; n + 1];
    for i in 0..n {
        for j in 0..m {
            if old_lines[i] == new_lines[j] {
                dp[i + 1][j + 1] = dp[i][j] + 1;
            } else {
                dp[i + 1][j + 1] = dp[i + 1][j].max(dp[i][j + 1]);
            }
        }
    }

    // 回溯构造编辑动作
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Op<'a> {
        Keep(&'a str),
        Delete(&'a str),
        Insert(&'a str),
    }

    let mut ops = Vec::new();
    let mut i = n;
    let mut j = m;
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && old_lines[i - 1] == new_lines[j - 1] {
            ops.push(Op::Keep(old_lines[i - 1]));
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || dp[i][j - 1] >= dp[i - 1][j]) {
            ops.push(Op::Insert(new_lines[j - 1]));
            j -= 1;
        } else if i > 0 {
            ops.push(Op::Delete(old_lines[i - 1]));
            i -= 1;
        }
    }
    ops.reverse();

    let mut added = 0;
    let mut deleted = 0;
    for op in &ops {
        match op {
            Op::Insert(_) => added += 1,
            Op::Delete(_) => deleted += 1,
            _ => {}
        }
    }

    // 格式化输出带上下文的统一 Diff
    let mut output = String::new();
    output.push_str(&format!("--- a/{}\n", filename).red().to_string());
    output.push_str(&format!("+++ b/{}\n", filename).green().to_string());

    // 分块聚合 (Hunks)
    let context = 3;
    let mut hunk_indices = Vec::new();
    for (idx, op) in ops.iter().enumerate() {
        if !matches!(op, Op::Keep(_)) {
            hunk_indices.push(idx);
        }
    }

    if hunk_indices.is_empty() {
        return DiffStats {
            added: 0,
            deleted: 0,
            colored_diff: "(文件内容无变更)".dimmed().to_string(),
        };
    }

    let mut hunks: Vec<(usize, usize)> = Vec::new();
    let mut cur_start = hunk_indices[0].saturating_sub(context);
    let mut cur_end = (hunk_indices[0] + context).min(ops.len() - 1);

    for &idx in &hunk_indices[1..] {
        let next_start = idx.saturating_sub(context);
        let next_end = (idx + context).min(ops.len() - 1);
        if next_start <= cur_end + 1 {
            cur_end = cur_end.max(next_end);
        } else {
            hunks.push((cur_start, cur_end));
            cur_start = next_start;
            cur_end = next_end;
        }
    }
    hunks.push((cur_start, cur_end));

    let mut old_line_no = 1;
    let mut new_line_no = 1;
    let mut op_idx = 0;

    for (h_start, h_end) in hunks {
        // 先快进到 h_start 统计行号
        while op_idx < h_start {
            match ops[op_idx] {
                Op::Keep(_) => {
                    old_line_no += 1;
                    new_line_no += 1;
                }
                Op::Delete(_) => old_line_no += 1,
                Op::Insert(_) => new_line_no += 1,
            }
            op_idx += 1;
        }

        let hunk_old_start = old_line_no;
        let hunk_new_start = new_line_no;
        let mut old_count = 0;
        let mut new_count = 0;

        for k in h_start..=h_end {
            match ops[k] {
                Op::Keep(_) => {
                    old_count += 1;
                    new_count += 1;
                }
                Op::Delete(_) => old_count += 1,
                Op::Insert(_) => new_count += 1,
            }
        }

        output.push_str(&format!(
            "{}\n",
            format!("@@ -{},{} +{},{} @@", hunk_old_start, old_count, hunk_new_start, new_count)
                .cyan()
                .bold()
        ));

        for k in h_start..=h_end {
            match ops[k] {
                Op::Keep(line) => {
                    output.push_str(&format!(" {:>4} │  {}\n", old_line_no, line).dimmed().to_string());
                    old_line_no += 1;
                    new_line_no += 1;
                }
                Op::Delete(line) => {
                    output.push_str(&format!(" {:>4} │ -{}\n", old_line_no, line).red().to_string());
                    old_line_no += 1;
                }
                Op::Insert(line) => {
                    output.push_str(&format!(" {:>4} │ +{}\n", new_line_no, line).green().to_string());
                    new_line_no += 1;
                }
            }
            op_idx = k + 1;
        }
    }

    DiffStats {
        added,
        deleted,
        colored_diff: output,
    }
}

fn fast_diff_fallback(filename: &str, old_lines: &[&str], new_lines: &[&str]) -> DiffStats {
    let added = new_lines.len();
    let deleted = old_lines.len();
    let mut out = format!("--- a/{}\n+++ b/{}\n", filename, filename);
    out.push_str(&format!("@@ 大文件快速覆写统计: -{} 行, +{} 行 @@\n", deleted, added));
    DiffStats {
        added,
        deleted,
        colored_diff: out.yellow().to_string(),
    }
}
