use serde::Deserialize;

/// Rig entry from ~/gt/rigs.json
#[derive(Debug, Clone, Deserialize)]
pub struct RigEntry {
    pub git_url: String,
    pub added_at: String,
    pub beads: Option<BeadConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BeadConfig {
    pub repo: Option<String>,
    pub prefix: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RigsFile {
    pub version: u32,
    pub rigs: std::collections::HashMap<String, RigEntry>,
}

/// Output of `gt status --json`
#[derive(Debug, Clone, Deserialize)]
pub struct TownStatus {
    pub name: String,
    pub location: String,
    pub overseer: Option<Overseer>,
    pub daemon: Option<ServiceStatus>,
    pub dolt: Option<DoltStatus>,
    pub agents: Vec<GtAgent>,
    pub rigs: Vec<GtRig>,
    pub summary: Option<TownSummary>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Overseer {
    pub name: String,
    pub email: String,
    pub unread_mail: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServiceStatus {
    pub running: bool,
    pub pid: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DoltStatus {
    pub running: bool,
    pub pid: Option<u64>,
    pub port: Option<u16>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GtAgent {
    pub name: String,
    pub address: String,
    pub session: Option<String>,
    pub role: Option<String>,
    pub running: bool,
    pub has_work: bool,
    pub state: Option<String>,
    pub work_title: Option<String>,
    pub hook_bead: Option<String>,
    pub unread_mail: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GtRig {
    pub name: String,
    pub polecat_count: u32,
    pub crew_count: u32,
    pub has_witness: bool,
    pub has_refinery: bool,
    pub hooks: Vec<GtHook>,
    pub agents: Vec<GtAgent>,
    pub mq: Option<GtMq>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GtHook {
    pub agent: String,
    pub role: String,
    pub has_work: bool,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GtMq {
    pub pending: u32,
    pub in_flight: u32,
    pub blocked: u32,
    pub state: Option<String>,
    pub health: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TownSummary {
    pub rig_count: u32,
    pub polecat_count: u32,
    pub crew_count: u32,
    pub witness_count: u32,
    pub refinery_count: u32,
    pub active_hooks: u32,
}

/// Bead (issue) from `bd list --json`
#[derive(Debug, Clone, Deserialize)]
pub struct Bead {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub priority: i32,
    pub issue_type: String,
    pub assignee: Option<String>,
    pub owner: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub labels: Option<Vec<String>>,
}

impl Bead {
    /// Check if this is a user-created task (not GT infrastructure)
    pub fn is_user_task(&self) -> bool {
        self.issue_type != "molecule"
            && !self
                .labels
                .as_ref()
                .is_some_and(|l| l.iter().any(|l| l.starts_with("gt:")))
            && !self.id.contains("-rig-")
            && !self.title.ends_with("Patrol")
            && !self.id.contains("-witness")
            && !self.id.contains("-refinery")
    }
}

/// Convoy from `gt convoy list --json`
#[derive(Debug, Clone, Deserialize)]
pub struct Convoy {
    pub id: String,
    pub title: String,
    pub status: String,
    pub created_at: Option<String>,
    pub tracked: Option<Vec<ConvoyItem>>,
    pub completed: u32,
    pub total: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConvoyItem {
    pub id: String,
    pub title: String,
    pub status: String,
    pub assignee: Option<String>,
    pub worker: Option<String>,
    pub blocked: Option<bool>,
}

/// Trail entry from `gt trail beads --json`
#[derive(Debug, Clone, Deserialize)]
pub struct TrailBead {
    pub id: Option<String>,
    pub title: Option<String>,
    pub status: Option<String>,
    pub rig: Option<String>,
    pub timestamp: Option<String>,
}
