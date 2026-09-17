use colored::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct FileSnapshot {
    pub path: PathBuf,
    pub old_content: Option<String>,
    pub new_content: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Checkpoint {
    pub id: usize,
    pub description: String,
    pub created_at: chrono::DateTime<chrono::Local>,
    pub changes: Vec<FileSnapshot>,
}

#[derive(Debug, Clone, Default)]
pub struct CheckpointManager {
    checkpoints: Vec<Checkpoint>,
    next_id: usize,
}

pub type SharedCheckpointManager = Arc<Mutex<CheckpointManager>>;

impl CheckpointManager {
    pub fn new() -> Self {
        Self {
            checkpoints: Vec::new(),
            next_id: 1,
        }
    }

    pub fn new_shared() -> SharedCheckpointManager {
        Arc::new(Mutex::new(Self::new()))
    }

    /// 记录文件变更动作
    pub fn record_file_change(
        &mut self,
        path: &Path,
        old_content: Option<String>,
        new_content: Option<String>,
        action_desc: &str,
    ) {
        let snapshot = FileSnapshot {
            path: path.to_path_buf(),
            old_content,
            new_content,
        };

        // 如果最后一个检查点的时间很近（同一步骤多文件变更），可以合并；否则新建
        if let Some(last) = self.checkpoints.last_mut() {
            if last.description == action_desc {
                last.changes.push(snapshot);
                return;
            }
        }

        let id = self.next_id;
        self.next_id += 1;
        self.checkpoints.push(Checkpoint {
            id,
            description: action_desc.to_string(),
            created_at: chrono::Local::now(),
            changes: vec![snapshot],
        });
    }

    /// 回滚最后一个检查点（/undo）
    pub fn undo_last(&mut self) -> Result<String, String> {
        let checkpoint = self
            .checkpoints
            .pop()
            .ok_or_else(|| "当前没有可回滚的文件检查点（栈为空）".to_string())?;

        let mut restored = Vec::new();

        // 逆序还原
        for snap in checkpoint.changes.into_iter().rev() {
            let path_display = snap.path.display().to_string();
            match snap.old_content {
                Some(content) => {
                    if let Some(parent) = snap.path.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    fs::write(&snap.path, content)
                        .map_err(|e| format!("还原文件 '{}' 失败: {}", path_display, e))?;
                    restored.push(format!("已恢复修改: {}", path_display.green()));
                }
                None => {
                    // 原本不存在，说明是新建的文件，直接删除
                    if snap.path.exists() {
                        let _ = fs::remove_file(&snap.path);
                        restored.push(format!("已删除新增文件: {}", path_display.yellow()));
                    }
                }
            }
        }

        let time_str = checkpoint.created_at.format("%H:%M:%S").to_string();
        Ok(format!(
            "{} 成功回滚步骤 #{} [{}] ({})：\n  {}",
            "✔".green().bold(),
            checkpoint.id,
            checkpoint.description.cyan(),
            time_str.dimmed(),
            restored.join("\n  ")
        ))
    }

    pub fn len(&self) -> usize {
        self.checkpoints.len()
    }

    pub fn is_empty(&self) -> bool {
        self.checkpoints.is_empty()
    }
}
