use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Maps Things task UUIDs to Gas Town bead IDs for bidirectional sync.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct TaskBeadMap {
    /// things_uuid -> bead_id
    pub things_to_bead: HashMap<String, BeadRef>,
    /// bead_id -> things_uuid
    pub bead_to_things: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeadRef {
    pub bead_id: String,
    pub rig: String,
    pub title: String,
    pub created_at: String,
}

impl TaskBeadMap {
    fn path() -> Result<PathBuf> {
        let home = std::env::var("HOME").context("HOME not set")?;
        let dir = PathBuf::from(format!("{}/.specflow", home));
        fs::create_dir_all(&dir)?;
        Ok(dir.join("task_bead_map.json"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&content).unwrap_or_default())
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        let content = serde_json::to_string_pretty(self)?;
        fs::write(&path, content)?;
        Ok(())
    }

    pub fn link(&mut self, things_uuid: &str, bead_id: &str, rig: &str, title: &str) {
        let now = chrono::Local::now().to_rfc3339();
        let bead_ref = BeadRef {
            bead_id: bead_id.to_string(),
            rig: rig.to_string(),
            title: title.to_string(),
            created_at: now,
        };
        self.things_to_bead
            .insert(things_uuid.to_string(), bead_ref);
        self.bead_to_things
            .insert(bead_id.to_string(), things_uuid.to_string());
    }

    pub fn bead_for_task(&self, things_uuid: &str) -> Option<&BeadRef> {
        self.things_to_bead.get(things_uuid)
    }

    pub fn task_for_bead(&self, bead_id: &str) -> Option<&str> {
        self.bead_to_things.get(bead_id).map(|s| s.as_str())
    }
}
