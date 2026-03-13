use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

/// Manages incremental task IDs, stored in a file on disk.
pub struct IdTracker {
    path: PathBuf,
}

impl IdTracker {
    pub fn new() -> Result<Self> {
        let home = std::env::var("HOME").context("HOME not set")?;
        let dir = PathBuf::from(format!("{}/.specflow", home));
        fs::create_dir_all(&dir)?;
        let path = dir.join("task_id_counter");
        Ok(Self { path })
    }

    /// Get the next ID and increment the counter
    pub fn next_id(&self) -> Result<u32> {
        let current = self.current()?;
        let next = current + 1;
        fs::write(&self.path, next.to_string())?;
        Ok(next)
    }

    /// Get the current counter value without incrementing
    pub fn current(&self) -> Result<u32> {
        if self.path.exists() {
            let content = fs::read_to_string(&self.path)?;
            Ok(content.trim().parse().unwrap_or(0))
        } else {
            fs::write(&self.path, "0")?;
            Ok(0)
        }
    }
}
