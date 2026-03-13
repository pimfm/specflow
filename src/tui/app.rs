use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::DefaultTerminal;
use std::time::Duration;

use crate::agent::runner::{SharedAgentStates};
use crate::things::db::ThingsDb;
use crate::triage::processor::TriagedTask;

pub struct App {
    pub running: bool,
    pub selected_tab: Tab,
    pub inbox_tasks: Vec<InboxItem>,
    pub agent_states: SharedAgentStates,
    pub triaged_tasks: Vec<TriagedTask>,
    pub log_messages: Vec<String>,
    pub scroll_offset: usize,
    db: ThingsDb,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Tab {
    Inbox,
    Agents,
    Log,
}

#[derive(Debug, Clone)]
pub struct InboxItem {
    pub uuid: String,
    pub title: String,
    pub has_agent_tag: bool,
    pub tags: Vec<String>,
}

impl App {
    pub fn new(agent_states: SharedAgentStates) -> Result<Self> {
        let db = ThingsDb::new()?;
        Ok(Self {
            running: true,
            selected_tab: Tab::Inbox,
            inbox_tasks: Vec::new(),
            agent_states,
            triaged_tasks: Vec::new(),
            log_messages: vec!["SpecFlow started.".to_string()],
            scroll_offset: 0,
            db,
        })
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
                    })
                    .collect();
            }
            Err(e) => {
                self.log_messages
                    .push(format!("Error refreshing inbox: {}", e));
            }
        }
    }

    pub fn add_log(&mut self, msg: String) {
        self.log_messages.push(msg);
        // Keep last 1000 messages
        if self.log_messages.len() > 1000 {
            self.log_messages.drain(0..500);
        }
    }

    pub async fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        self.refresh_inbox();

        while self.running {
            // Draw UI
            let agent_states = self.agent_states.lock().await.clone();
            terminal.draw(|frame| {
                super::ui::draw(frame, &self, &agent_states);
            })?;

            // Handle events with timeout for periodic refresh
            if event::poll(Duration::from_millis(500))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') => self.running = false,
                            KeyCode::Tab => {
                                self.selected_tab = match self.selected_tab {
                                    Tab::Inbox => Tab::Agents,
                                    Tab::Agents => Tab::Log,
                                    Tab::Log => Tab::Inbox,
                                };
                            }
                            KeyCode::Char('r') => {
                                self.refresh_inbox();
                                self.add_log("Refreshed inbox.".to_string());
                            }
                            KeyCode::Char('t') => {
                                // Trigger triage
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
                                            self.refresh_inbox();
                                        }
                                        Err(e) => {
                                            self.add_log(format!("Triage error: {}", e));
                                        }
                                    },
                                    Err(e) => {
                                        self.add_log(format!("Failed to init triage: {}", e));
                                    }
                                }
                            }
                            KeyCode::Up => {
                                if self.scroll_offset > 0 {
                                    self.scroll_offset -= 1;
                                }
                            }
                            KeyCode::Down => {
                                self.scroll_offset += 1;
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
