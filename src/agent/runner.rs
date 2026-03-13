use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{info, warn, error};

use crate::things::db::ThingsDb;
use super::executor;

/// State of a running agent task
#[derive(Debug, Clone)]
pub struct AgentState {
    pub task_uuid: String,
    pub task_title: String,
    pub status: AgentStatus,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AgentStatus {
    Queued,
    Running,
    Completed,
    Failed,
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentStatus::Queued => write!(f, "QUEUED"),
            AgentStatus::Running => write!(f, "RUNNING"),
            AgentStatus::Completed => write!(f, "DONE"),
            AgentStatus::Failed => write!(f, "FAILED"),
        }
    }
}

pub type SharedAgentStates = Arc<Mutex<Vec<AgentState>>>;

/// Continuously scan for agent tasks and execute them
pub async fn run_agent_loop(states: SharedAgentStates) -> Result<()> {
    let db = ThingsDb::new()?;
    let scan_interval = Duration::from_secs(30);

    loop {
        info!("Scanning for agent tasks...");

        match db.agent_today_tasks() {
            Ok(tasks) => {
                info!("Found {} tasks to process", tasks.len());

                for task in tasks {
                    // Check if already being processed
                    {
                        let states_lock = states.lock().await;
                        if states_lock.iter().any(|s| s.task_uuid == task.uuid) {
                            continue;
                        }
                    }

                    // Add to states as queued
                    {
                        let mut states_lock = states.lock().await;
                        states_lock.push(AgentState {
                            task_uuid: task.uuid.clone(),
                            task_title: task.title.clone(),
                            status: AgentStatus::Queued,
                            error: None,
                        });
                    }

                    // Spawn async task for execution
                    let states_clone = states.clone();
                    let task_clone = task.clone();
                    tokio::spawn(async move {
                        // Mark as running
                        {
                            let mut states_lock = states_clone.lock().await;
                            if let Some(state) = states_lock
                                .iter_mut()
                                .find(|s| s.task_uuid == task_clone.uuid)
                            {
                                state.status = AgentStatus::Running;
                            }
                        }

                        // Execute
                        match executor::execute_task(&task_clone).await {
                            Ok(result) => {
                                info!("Task #{} completed: MR {}", result.task_id, result.mr_url);
                                let mut states_lock = states_clone.lock().await;
                                if let Some(state) = states_lock
                                    .iter_mut()
                                    .find(|s| s.task_uuid == task_clone.uuid)
                                {
                                    state.status = AgentStatus::Completed;
                                }
                            }
                            Err(e) => {
                                error!("Task '{}' failed: {}", task_clone.title, e);
                                // Tag as error
                                let _ = crate::things::applescript::set_tags(
                                    &task_clone.uuid,
                                    &["agent-error"],
                                );
                                let mut states_lock = states_clone.lock().await;
                                if let Some(state) = states_lock
                                    .iter_mut()
                                    .find(|s| s.task_uuid == task_clone.uuid)
                                {
                                    state.status = AgentStatus::Failed;
                                    state.error = Some(e.to_string());
                                }
                            }
                        }
                    });
                }
            }
            Err(e) => {
                warn!("Failed to scan for tasks: {}", e);
            }
        }

        tokio::time::sleep(scan_interval).await;
    }
}
