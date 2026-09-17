use colored::*;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStatus {
    Running,
    Finished(i32),
    Killed,
    Failed(String),
}

pub struct BackgroundTask {
    pub id: String,
    pub command: String,
    pub started_at: chrono::DateTime<chrono::Local>,
    pub child: Option<Child>,
    pub logs: Arc<Mutex<Vec<String>>>,
    pub status: TaskStatus,
}

#[derive(Clone)]
pub struct TaskManager {
    tasks: Arc<Mutex<HashMap<String, BackgroundTask>>>,
    counter: Arc<Mutex<usize>>,
}

pub type SharedTaskManager = TaskManager;

impl Default for TaskManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskManager {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(HashMap::new())),
            counter: Arc::new(Mutex::new(1)),
        }
    }

    pub async fn spawn_task(&self, command: &str) -> Result<String, String> {
        let task_id = {
            let mut c = self.counter.lock().unwrap();
            let id = format!("task-{}", *c);
            *c += 1;
            id
        };

        let mut child = Command::new("bash")
            .arg("-c")
            .arg(command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("启动后台任务失败: {}", e))?;

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let logs = Arc::new(Mutex::new(Vec::<String>::new()));
        let logs_stdout = Arc::clone(&logs);
        let logs_stderr = Arc::clone(&logs);

        // 异步读取 stdout
        if let Some(out) = stdout {
            tokio::spawn(async move {
                let mut reader = BufReader::new(out).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let mut lg = logs_stdout.lock().unwrap();
                    if lg.len() > 1000 {
                        lg.remove(0);
                    }
                    lg.push(format!("[out] {}", line));
                }
            });
        }

        // 异步读取 stderr
        if let Some(err) = stderr {
            tokio::spawn(async move {
                let mut reader = BufReader::new(err).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let mut lg = logs_stderr.lock().unwrap();
                    if lg.len() > 1000 {
                        lg.remove(0);
                    }
                    lg.push(format!("[err] {}", line));
                }
            });
        }

        let bg_task = BackgroundTask {
            id: task_id.clone(),
            command: command.to_string(),
            started_at: chrono::Local::now(),
            child: Some(child),
            logs,
            status: TaskStatus::Running,
        };

        self.tasks.lock().unwrap().insert(task_id.clone(), bg_task);

        // 异步监听进程退出
        let tasks_ref = Arc::clone(&self.tasks);
        let tid = task_id.clone();
        tokio::spawn(async move {
            let mut maybe_child = {
                let mut lock = tasks_ref.lock().unwrap();
                lock.get_mut(&tid).and_then(|t| t.child.take())
            };

            if let Some(ref mut child) = maybe_child {
                let exit_status = child.wait().await;
                let mut lock = tasks_ref.lock().unwrap();
                if let Some(t) = lock.get_mut(&tid) {
                    match exit_status {
                        Ok(status) => {
                            let code = status.code().unwrap_or(-1);
                            t.status = TaskStatus::Finished(code);
                        }
                        Err(e) => {
                            t.status = TaskStatus::Failed(e.to_string());
                        }
                    }
                }
            }
        });

        Ok(task_id)
    }

    pub fn list_tasks(&self) -> Vec<(String, String, String, String)> {
        let lock = self.tasks.lock().unwrap();
        let mut list = Vec::new();
        for (id, t) in lock.iter() {
            let status_str = match &t.status {
                TaskStatus::Running => "运行中 (Running)".green().bold().to_string(),
                TaskStatus::Finished(c) => format!("已退出 [code: {}]", c).dimmed().to_string(),
                TaskStatus::Killed => "已终止 (Killed)".red().to_string(),
                TaskStatus::Failed(e) => format!("失败: {}", e).red().to_string(),
            };
            let time_str = t.started_at.format("%H:%M:%S").to_string();
            list.push((id.clone(), t.command.clone(), status_str, time_str));
        }
        list.sort_by(|a, b| a.0.cmp(&b.0));
        list
    }

    pub fn get_logs(&self, task_id: &str, limit: Option<usize>) -> Result<String, String> {
        let lock = self.tasks.lock().unwrap();
        let task = lock
            .get(task_id)
            .ok_or_else(|| format!("未找到任务 ID: {}", task_id))?;
        let logs = task.logs.lock().unwrap();
        let count = limit.unwrap_or(50);
        let start = logs.len().saturating_sub(count);
        let slice = &logs[start..];
        if slice.is_empty() {
            Ok("(暂无输出日志)".to_string())
        } else {
            Ok(slice.join("\n"))
        }
    }

    pub async fn kill_task(&self, task_id: &str) -> Result<String, String> {
        let child_opt = {
            let mut lock = self.tasks.lock().unwrap();
            let task = lock
                .get_mut(task_id)
                .ok_or_else(|| format!("未找到任务 ID: {}", task_id))?;
            if task.status != TaskStatus::Running {
                return Err(format!("任务 {} 当前不处于运行状态", task_id));
            }
            task.status = TaskStatus::Killed;
            task.child.take()
        };

        if let Some(mut child) = child_opt {
            let _ = child.kill().await;
            Ok(format!("✔ 已成功终止后台任务 {}", task_id.green().bold()))
        } else {
            Ok(format!("任务 {} 进程已结束", task_id))
        }
    }
}
