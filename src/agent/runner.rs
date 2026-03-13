use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use super::executor;
use crate::sync::engine::SyncEngine;
use crate::things::db::ThingsDb;

/// State of a running agent task
#[derive(Debug, Clone)]
pub struct AgentState {
    pub task_uuid: String,
    pub task_title: String,
    pub status: AgentStatus,
    pub bead_id: Option<String>,
    pub rig: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AgentStatus {
    Queued,
    Running,
    Dispatched,
    Completed,
    Failed,
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentStatus::Queued => write!(f, "QUEUED"),
            AgentStatus::Running => write!(f, "RUNNING"),
            AgentStatus::Dispatched => write!(f, "DISPATCHED"),
            AgentStatus::Completed => write!(f, "DONE"),
            AgentStatus::Failed => write!(f, "FAILED"),
        }
    }
}

pub type SharedAgentStates = Arc<Mutex<Vec<AgentState>>>;

/// Continuously scan for agent tasks, execute them via GT, and sync completions.
pub async fn run_agent_loop(states: SharedAgentStates) -> Result<()> {
    let db = ThingsDb::new()?;
    let scan_interval = Duration::from_secs(30);
    let sync_interval = Duration::from_secs(60);
    let mut last_sync = std::time::Instant::now();

    loop {
        info!("Scanning for agent tasks...");

        // Periodically sync completions from GT back to Things
        if last_sync.elapsed() >= sync_interval {
            match SyncEngine::new() {
                Ok(mut sync) => match sync.sync_completions() {
                    Ok(results) => {
                        for r in &results {
                            info!(
                                "Sync: bead {} -> Things {} ({})",
                                r.bead_id, r.things_uuid, r.action
                            );
                            // Update agent state if we have a match
                            let mut states_lock = states.lock().await;
                            if let Some(state) = states_lock
                                .iter_mut()
                                .find(|s| s.task_uuid == r.things_uuid)
                            {
                                state.status = AgentStatus::Completed;
                            }
                        }
                    }
                    Err(e) => warn!("Sync check failed: {}", e),
                },
                Err(e) => warn!("Could not init sync engine: {}", e),
            }
            last_sync = std::time::Instant::now();
        }

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
                            bead_id: None,
                            rig: None,
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

                        // Execute with GT integration
                        let sync_result = SyncEngine::new();
                        match sync_result {
                            Ok(mut sync) => {
                                match executor::execute_task(&task_clone, &mut sync).await {
                                    Ok(result) => {
                                        info!(
                                            "Task #{} dispatched: bead {} -> rig {}",
                                            result.task_id, result.bead_id, result.rig
                                        );
                                        let mut states_lock = states_clone.lock().await;
                                        if let Some(state) = states_lock
                                            .iter_mut()
                                            .find(|s| s.task_uuid == task_clone.uuid)
                                        {
                                            state.status = AgentStatus::Dispatched;
                                            state.bead_id = Some(result.bead_id);
                                            state.rig = Some(result.rig);
                                        }
                                    }
                                    Err(e) => {
                                        error!("Task '{}' failed: {}", task_clone.title, e);
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
                            }
                            Err(e) => {
                                error!("Could not init sync engine: {}", e);
                                let mut states_lock = states_clone.lock().await;
                                if let Some(state) = states_lock
                                    .iter_mut()
                                    .find(|s| s.task_uuid == task_clone.uuid)
                                {
                                    state.status = AgentStatus::Failed;
                                    state.error = Some(format!("GT not available: {}", e));
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
