use anyhow::Result;
use tracing::{info, warn};

use crate::gastown::client::GtClient;
use crate::things::{applescript, db::ThingsDb};

use super::mapping::TaskBeadMap;

/// The sync engine bridges Things 3 tasks and Gas Town beads.
///
/// Flow:
///   Things inbox -> triage -> create bead -> sling to rig -> polecat works
///   -> bead closed -> sync detects closure -> complete Things task
pub struct SyncEngine {
    pub gt: GtClient,
    pub db: ThingsDb,
    pub map: TaskBeadMap,
}

impl SyncEngine {
    pub fn new() -> Result<Self> {
        Ok(Self {
            gt: GtClient::discover()?,
            db: ThingsDb::new()?,
            map: TaskBeadMap::load()?,
        })
    }

    /// Scan all mapped beads and complete any Things tasks whose beads are closed.
    pub fn sync_completions(&mut self) -> Result<Vec<SyncResult>> {
        let mut results = Vec::new();
        let rigs = self.gt.load_rigs()?;

        // Collect bead_id -> things_uuid pairs to check
        let pairs: Vec<(String, String, String)> = self
            .map
            .things_to_bead
            .iter()
            .map(|(things_uuid, bead_ref)| {
                (
                    things_uuid.clone(),
                    bead_ref.bead_id.clone(),
                    bead_ref.rig.clone(),
                )
            })
            .collect();

        for (things_uuid, bead_id, rig) in pairs {
            // Check if bead is closed
            let beads = self.gt.list_beads(&rig)?;
            if let Some(bead) = beads.iter().find(|b| b.id == bead_id) {
                if bead.status == "closed" {
                    // Complete the Things task
                    info!(
                        "Bead {} is closed, completing Things task {}",
                        bead_id, things_uuid
                    );
                    match applescript::complete_task(&things_uuid) {
                        Ok(()) => {
                            // Update notes with completion info
                            let note = format!(
                                "Completed via Gas Town bead {}\nRig: {}",
                                bead_id, rig
                            );
                            let _ = applescript::set_notes(&things_uuid, &note);
                            let _ = applescript::set_tags(&things_uuid, &["agent-done"]);

                            results.push(SyncResult {
                                things_uuid: things_uuid.clone(),
                                bead_id: bead_id.clone(),
                                rig: rig.clone(),
                                action: SyncAction::Completed,
                            });
                        }
                        Err(e) => {
                            warn!("Failed to complete Things task {}: {}", things_uuid, e);
                            results.push(SyncResult {
                                things_uuid,
                                bead_id,
                                rig,
                                action: SyncAction::Error(e.to_string()),
                            });
                        }
                    }
                } else if bead.status == "in_progress" {
                    // Update Things tag to reflect progress
                    let _ = applescript::set_tags(&things_uuid, &["agent-running"]);
                }
            }
        }

        // Also check for new completed beads that might match Things tasks by title
        for (rig_name, _) in &rigs.rigs {
            let beads = self.gt.list_beads(rig_name)?;
            for bead in &beads {
                if bead.status == "closed" && self.map.task_for_bead(&bead.id).is_none() {
                    // Not mapped — skip, but log for visibility
                    info!(
                        "Closed bead {} in {} has no Things mapping (title: {})",
                        bead.id, rig_name, bead.title
                    );
                }
            }
        }

        self.map.save()?;
        Ok(results)
    }

    /// Create a bead from a Things task and link them.
    pub fn create_bead_for_task(
        &mut self,
        things_uuid: &str,
        title: &str,
        description: &str,
        rig: &str,
        priority: u8,
    ) -> Result<String> {
        let bead_id = self.gt.create_bead(rig, title, "task", priority)?;
        info!("Created bead {} in rig {} for Things task", bead_id, rig);

        // Update bead description with Things context
        let full_desc = format!(
            "{}\n\n---\nSource: Things 3 task {}\nSynced by SpecFlow",
            description, things_uuid
        );
        let _ = self
            .gt
            .update_bead_description(rig, &bead_id, &full_desc);

        // Link in our map
        self.map.link(things_uuid, &bead_id, rig, title);
        self.map.save()?;

        // Update Things notes with bead reference
        let note_update = format!("Bead: {}\nRig: {}\n\n{}", bead_id, rig, description);
        applescript::set_notes(things_uuid, &note_update)?;

        Ok(bead_id)
    }

    /// Sling a bead (dispatch to a rig for agent work).
    pub fn sling_bead(&self, bead_id: &str, rig: &str) -> Result<String> {
        self.gt.sling(bead_id, rig)
    }
}

#[derive(Debug, Clone)]
pub struct SyncResult {
    pub things_uuid: String,
    pub bead_id: String,
    pub rig: String,
    pub action: SyncAction,
}

#[derive(Debug, Clone)]
pub enum SyncAction {
    Completed,
    Error(String),
}

impl std::fmt::Display for SyncAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncAction::Completed => write!(f, "completed"),
            SyncAction::Error(e) => write!(f, "error: {}", e),
        }
    }
}
