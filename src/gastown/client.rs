use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

use super::model::*;

/// Client for interacting with Gas Town via the `gt` and `bd` CLIs.
pub struct GtClient {
    gt_root: PathBuf,
}

impl GtClient {
    /// Discover the GT workspace at ~/gt
    pub fn discover() -> Result<Self> {
        let home = std::env::var("HOME").context("HOME not set")?;
        let gt_root = PathBuf::from(format!("{}/gt", home));
        if !gt_root.join("mayor").is_dir() {
            anyhow::bail!("Gas Town workspace not found at ~/gt");
        }
        Ok(Self { gt_root })
    }

    pub fn root(&self) -> &Path {
        &self.gt_root
    }

    /// Load rigs from ~/gt/rigs.json
    pub fn load_rigs(&self) -> Result<RigsFile> {
        let path = self.gt_root.join("rigs.json");
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        serde_json::from_str(&content).context("Failed to parse rigs.json")
    }

    /// Fetch town status via `gt status --json`
    pub fn status(&self) -> Result<TownStatus> {
        let output = self.run_gt(&["status", "--json"])?;
        serde_json::from_str(&output).context("Failed to parse gt status output")
    }

    /// List beads in a rig via `bd list --json --all`
    pub fn list_beads(&self, rig_name: &str) -> Result<Vec<Bead>> {
        let rig_dir = self.gt_root.join(rig_name);
        if !rig_dir.is_dir() {
            return Ok(vec![]);
        }
        let output = Command::new("bd")
            .current_dir(&rig_dir)
            .args(["list", "--json", "--all", "-n", "0"])
            .output()
            .context("Failed to run bd list")?;
        if !output.status.success() {
            return Ok(vec![]);
        }
        let text = String::from_utf8_lossy(&output.stdout);
        Ok(serde_json::from_str(&text).unwrap_or_default())
    }

    /// List HQ beads
    pub fn list_hq_beads(&self) -> Result<Vec<Bead>> {
        let output = Command::new("bd")
            .current_dir(&self.gt_root)
            .args(["list", "--json", "--all", "-n", "0"])
            .output()
            .context("Failed to run bd list on hq")?;
        if !output.status.success() {
            return Ok(vec![]);
        }
        let text = String::from_utf8_lossy(&output.stdout);
        Ok(serde_json::from_str(&text).unwrap_or_default())
    }

    /// Create a bead in a rig via `bd create`
    pub fn create_bead(
        &self,
        rig_name: &str,
        title: &str,
        issue_type: &str,
        priority: u8,
    ) -> Result<String> {
        let rig_dir = self.gt_root.join(rig_name);
        let output = Command::new("bd")
            .current_dir(&rig_dir)
            .args([
                "create",
                title,
                "-t",
                issue_type,
                "-p",
                &priority.to_string(),
                "--json",
                "--silent",
            ])
            .output()
            .context("Failed to run bd create")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("bd create failed: {}", stderr);
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Sling a bead to a rig via `gt sling <bead-id> <rig>`
    pub fn sling(&self, bead_id: &str, rig: &str) -> Result<String> {
        let output = self.run_gt(&["sling", bead_id, rig])?;
        Ok(output)
    }

    /// List convoys via `gt convoy list --json`
    pub fn list_convoys(&self) -> Result<Vec<Convoy>> {
        let output = self.run_gt(&["convoy", "list", "--json"]);
        match output {
            Ok(text) => Ok(serde_json::from_str(&text).unwrap_or_default()),
            Err(_) => Ok(vec![]),
        }
    }

    /// Get recent bead trail via `gt trail beads --json`
    pub fn trail_beads(&self, since: &str) -> Result<Vec<TrailBead>> {
        let output = self.run_gt(&["trail", "beads", "--since", since, "--json"]);
        match output {
            Ok(text) => Ok(serde_json::from_str(&text).unwrap_or_default()),
            Err(_) => Ok(vec![]),
        }
    }

    /// Find the best rig for a task based on keywords in title/notes
    pub fn find_rig_for_task(&self, title: &str, notes: &str) -> Result<Option<String>> {
        let rigs = self.load_rigs()?;
        let text = format!("{} {}", title, notes).to_lowercase();

        for (name, _rig) in &rigs.rigs {
            if text.contains(&name.to_lowercase()) {
                return Ok(Some(name.clone()));
            }
        }
        Ok(None)
    }

    /// Close a bead via `bd close`
    pub fn close_bead(&self, rig_name: &str, bead_id: &str) -> Result<()> {
        let rig_dir = self.gt_root.join(rig_name);
        let output = Command::new("bd")
            .current_dir(&rig_dir)
            .args(["close", bead_id])
            .output()
            .context("Failed to run bd close")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("bd close failed: {}", stderr);
        }
        Ok(())
    }

    /// Update bead description via `bd update`
    pub fn update_bead_description(
        &self,
        rig_name: &str,
        bead_id: &str,
        description: &str,
    ) -> Result<()> {
        let rig_dir = self.gt_root.join(rig_name);
        let output = Command::new("bd")
            .current_dir(&rig_dir)
            .args(["update", bead_id, "-d", description])
            .output()
            .context("Failed to run bd update")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("bd update failed: {}", stderr);
        }
        Ok(())
    }

    fn run_gt(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("gt")
            .current_dir(&self.gt_root)
            .args(args)
            .output()
            .with_context(|| format!("Failed to run gt {:?}", args))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("gt {:?} failed: {}", args, stderr);
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}
