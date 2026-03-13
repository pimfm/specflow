use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub uuid: String,
    pub title: String,
    pub notes: String,
    pub tags: Vec<String>,
    pub status: TaskStatus,
    pub project: Option<String>,
    pub project_uuid: Option<String>,
    pub area: Option<String>,
    pub area_uuid: Option<String>,
    pub checklist_items: Vec<ChecklistItem>,
    pub start_date: Option<i64>,
    pub creation_date: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Open,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistItem {
    pub uuid: String,
    pub title: String,
    pub completed: bool,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Project {
    pub uuid: String,
    pub title: String,
    pub area_uuid: Option<String>,
    pub area_name: Option<String>,
}

impl Task {
    pub fn is_agent_task(&self) -> bool {
        self.tags.iter().any(|t| t.starts_with("agent-"))
    }

    pub fn has_review_tag(&self) -> bool {
        self.tags.iter().any(|t| t == "agent-done")
    }
}
