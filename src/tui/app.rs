use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::DefaultTerminal;
use std::time::Duration;

use crate::agent::runner::SharedAgentStates;
use crate::gastown::client::GtClient;
use crate::gastown::model::{Bead, Convoy, GtAgent, GtRig, TownSummary};
use crate::sync::engine::SyncEngine;
use crate::things::db::ThingsDb;
use crate::triage::processor::TriagedTask;

pub const TAB_COUNT: usize = 5;
pub const TAB_NAMES: [&str; TAB_COUNT] = ["Pipeline", "Things", "Rigs", "Beads", "Log"];

pub struct App {
    pub running: bool,
    pub tab: usize,
    pub scroll: usize,
    pub max_scroll: usize,
    pub tick: u64,

    // Things data
    pub inbox_tasks: Vec<InboxItem>,
    pub agent_today: Vec<InboxItem>,

    // Agent states (from background runner)
    pub agent_states: SharedAgentStates,
    pub triaged_tasks: Vec<TriagedTask>,

    // Gas Town data
    pub gt_connected: bool,
    pub town_name: String,
    pub rigs: Vec<GtRig>,
    pub agents: Vec<GtAgent>,
    pub summary: TownSummary,
    pub beads: Vec<(String, Bead)>, // (rig_name, bead)
    pub convoys: Vec<Convoy>,

    // Sync state
    pub last_sync_count: usize,
    pub sync_errors: Vec<String>,

    // Log
    pub log_messages: Vec<String>,

    // Status
    pub status_msg: Option<String>,

    db: ThingsDb,
}

#[derive(Debug, Clone)]
pub struct InboxItem {
    pub uuid: String,
    pub title: String,
    pub has_agent_tag: bool,
    pub tags: Vec<String>,
    pub notes: String,
    pub project: Option<String>,
}

impl App {
    pub fn new(agent_states: SharedAgentStates) -> Result<Self> {
        let db = ThingsDb::new()?;
        let gt_connected = GtClient::discover().is_ok();

        Ok(Self {
            running: true,
            tab: 0,
            scroll: 0,
            max_scroll: 0,
            tick: 0,
            inbox_tasks: Vec::new(),
            agent_today: Vec::new(),
            agent_states,
            triaged_tasks: Vec::new(),
            gt_connected,
            town_name: String::new(),
            rigs: Vec::new(),
            agents: Vec::new(),
            summary: TownSummary::default(),
            beads: Vec::new(),
            convoys: Vec::new(),
            last_sync_count: 0,
            sync_errors: Vec::new(),
            log_messages: vec!["SpecFlow started. Gas Town integration active.".to_string()],
            status_msg: None,
            db,
        })
    }

    pub fn refresh_all(&mut self) {
        self.refresh_inbox();
        self.refresh_agent_today();
        self.refresh_gt();
    }

    pub fn refresh_inbox(&mut self) {
        match self.db.inbox_tasks() {
            Ok(tasks) => {
                self.inbox_tasks = tasks
                    .into_iter()
                    .map(|t| InboxItem {
                        has_agent_tag: t.is_agent_task(),
                        uuid: t.uuid,
                        title: t.title,
                        tags: t.tags,
                        notes: t.notes,
                        project: t.project,
                    })
                    .collect();
            }
            Err(e) => {
                self.add_log(format!("Error refreshing inbox: {}", e));
            }
        }
    }

    pub fn refresh_agent_today(&mut self) {
        match self.db.agent_today_tasks() {
            Ok(tasks) => {
                self.agent_today = tasks
                    .into_iter()
                    .map(|t| InboxItem {
                        has_agent_tag: t.is_agent_task(),
                        uuid: t.uuid,
                        title: t.title,
                        tags: t.tags,
                        notes: t.notes,
                        project: t.project,
                    })
                    .collect();
            }
            Err(e) => {
                self.add_log(format!("Error refreshing today tasks: {}", e));
            }
        }
    }

    pub fn refresh_gt(&mut self) {
        let gt = match GtClient::discover() {
            Ok(gt) => gt,
            Err(_) => {
                self.gt_connected = false;
                return;
            }
        };
        self.gt_connected = true;

        // Fetch status
        if let Ok(status) = gt.status() {
            self.town_name = status.name;
            self.agents.clear();
            self.agents.extend(status.agents);
            for rig in &status.rigs {
                for agent in &rig.agents {
                    let mut a = agent.clone();
                    if !a.address.contains('/') {
                        a.address = format!("{}/{}", rig.name, a.name);
                    }
                    self.agents.push(a);
                }
            }
            self.rigs = status.rigs;
            if let Some(summary) = status.summary {
                self.summary = summary;
            }
        }

        // Fetch beads across all rigs
        self.beads.clear();
        if let Ok(hq_beads) = gt.list_hq_beads() {
            for bead in hq_beads {
                self.beads.push(("hq".to_string(), bead));
            }
        }
        let rig_names: Vec<String> = self.rigs.iter().map(|r| r.name.clone()).collect();
        for rig_name in &rig_names {
            if let Ok(rig_beads) = gt.list_beads(rig_name) {
                for bead in rig_beads {
                    self.beads.push((rig_name.clone(), bead));
                }
            }
        }

        // Fetch convoys
        if let Ok(convoys) = gt.list_convoys() {
            self.convoys = convoys;
        }
    }

    pub fn run_sync(&mut self) {
        self.add_log("Running sync...".to_string());
        match SyncEngine::new() {
            Ok(mut sync) => match sync.sync_completions() {
                Ok(results) => {
                    self.last_sync_count = results.len();
                    for r in &results {
                        self.add_log(format!(
                            "Sync: {} -> {} ({})",
                            r.bead_id, r.things_uuid, r.action
                        ));
                    }
                    if results.is_empty() {
                        self.add_log("Sync: no pending completions.".to_string());
                    }
                    self.refresh_inbox();
                    self.refresh_agent_today();
                }
                Err(e) => {
                    self.add_log(format!("Sync error: {}", e));
                    self.sync_errors.push(e.to_string());
                }
            },
            Err(e) => {
                self.add_log(format!("Sync engine init failed: {}", e));
            }
        }
    }

    pub fn add_log(&mut self, msg: String) {
        let timestamp = chrono::Local::now().format("%H:%M:%S").to_string();
        self.log_messages.push(format!("[{}] {}", timestamp, msg));
        if self.log_messages.len() > 1000 {
            self.log_messages.drain(0..500);
        }
    }

    pub fn user_beads(&self) -> Vec<&(String, Bead)> {
        self.beads.iter().filter(|(_, b)| b.is_user_task()).collect()
    }

    pub fn agents_running(&self) -> usize {
        self.agents.iter().filter(|a| a.running).count()
    }

    pub fn agents_with_work(&self) -> usize {
        self.agents.iter().filter(|a| a.has_work).count()
    }

    // Navigation
    pub fn next_tab(&mut self) {
        self.tab = (self.tab + 1) % TAB_COUNT;
        self.scroll = 0;
    }

    pub fn scroll_down(&mut self) {
        if self.scroll < self.max_scroll {
            self.scroll += 1;
        }
    }

    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    pub fn on_tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
    }

    pub async fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        self.refresh_all();

        let mut last_gt_refresh = std::time::Instant::now();
        let gt_refresh_interval = Duration::from_secs(10);

        while self.running {
            self.on_tick();

            // Periodic GT refresh
            if last_gt_refresh.elapsed() >= gt_refresh_interval {
                self.refresh_gt();
                self.refresh_agent_today();
                last_gt_refresh = std::time::Instant::now();
            }

            // Draw UI
            let agent_states = self.agent_states.lock().await.clone();
            terminal.draw(|frame| {
                super::ui::draw(frame, &mut self, &agent_states);
            })?;

            // Handle events
            if event::poll(Duration::from_millis(250))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') => self.running = false,
                            KeyCode::Tab => self.next_tab(),
                            KeyCode::Char('1') => { self.tab = 0; self.scroll = 0; }
                            KeyCode::Char('2') => { self.tab = 1; self.scroll = 0; }
                            KeyCode::Char('3') => { self.tab = 2; self.scroll = 0; }
                            KeyCode::Char('4') => { self.tab = 3; self.scroll = 0; }
                            KeyCode::Char('5') => { self.tab = 4; self.scroll = 0; }
                            KeyCode::Char('r') => {
                                self.refresh_all();
                                self.add_log("Refreshed all data.".to_string());
                            }
                            KeyCode::Char('t') => {
                                self.add_log("Running inbox triage...".to_string());
                                match crate::triage::processor::TriageProcessor::new() {
                                    Ok(processor) => match processor.process_inbox() {
                                        Ok(results) => {
                                            for r in &results {
                                                self.add_log(format!(
                                                    "Triaged: {} -> {} ({})",
                                                    r.original_title,
                                                    r.new_title,
                                                    r.project.as_deref().unwrap_or("Agents")
                                                ));
                                            }
                                            self.triaged_tasks.extend(results);
                                            self.refresh_all();
                                        }
                                        Err(e) => self.add_log(format!("Triage error: {}", e)),
                                    },
                                    Err(e) => self.add_log(format!("Triage init failed: {}", e)),
                                }
                            }
                            KeyCode::Char('s') => {
                                self.run_sync();
                            }
                            KeyCode::Char('j') | KeyCode::Down => self.scroll_down(),
                            KeyCode::Char('k') | KeyCode::Up => self.scroll_up(),
                            KeyCode::Esc => {
                                self.status_msg = None;
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
