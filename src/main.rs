mod agent;
mod gastown;
mod spec;
mod sync;
mod things;
mod triage;
mod tui;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "specflow")]
#[command(about = "Spec-driven task automation bridging Things 3 with Gas Town")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Launch the interactive TUI dashboard
    Ui,

    /// Triage inbox tasks: assign IDs, enhance descriptions, classify projects
    Triage,

    /// Run the agent loop: scan for today's agent tasks and execute them
    Run,

    /// Sync completions: check GT beads and complete corresponding Things tasks
    Sync,

    /// Show current status (Things + Gas Town)
    Status,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("specflow=info".parse()?))
        .with_target(false)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Ui => run_ui().await,
        Commands::Triage => run_triage().await,
        Commands::Run => run_agents().await,
        Commands::Sync => run_sync().await,
        Commands::Status => show_status(),
    }
}

async fn run_ui() -> Result<()> {
    let agent_states = Arc::new(Mutex::new(Vec::new()));

    let states_clone = agent_states.clone();
    tokio::spawn(async move {
        if let Err(e) = agent::runner::run_agent_loop(states_clone).await {
            eprintln!("Agent loop error: {}", e);
        }
    });

    let mut terminal = ratatui::init();
    let app = tui::app::App::new(agent_states)?;
    let result = app.run(&mut terminal).await;
    ratatui::restore();

    result
}

async fn run_triage() -> Result<()> {
    let processor = triage::processor::TriageProcessor::new()?;
    let results = processor.process_inbox()?;

    if results.is_empty() {
        println!("No tasks in inbox.");
    } else {
        for r in &results {
            println!(
                "Triaged: {} -> {} (project: {})",
                r.original_title,
                r.new_title,
                r.project.as_deref().unwrap_or("Agents")
            );
        }
        println!("\n{} tasks triaged.", results.len());
    }

    Ok(())
}

async fn run_agents() -> Result<()> {
    let agent_states = Arc::new(Mutex::new(Vec::new()));

    println!("Starting agent loop with Gas Town integration.");
    println!("Scanning every 30s, syncing every 60s.");
    println!("Press Ctrl+C to stop.\n");

    agent::runner::run_agent_loop(agent_states).await
}

async fn run_sync() -> Result<()> {
    println!("Running Things <-> Gas Town sync...\n");

    let mut engine = sync::engine::SyncEngine::new()?;
    let results = engine.sync_completions()?;

    if results.is_empty() {
        println!("No pending completions to sync.");
    } else {
        for r in &results {
            println!(
                "  {} bead:{} rig:{} -> {}",
                r.things_uuid, r.bead_id, r.rig, r.action
            );
        }
        println!("\n{} items synced.", results.len());
    }

    Ok(())
}

fn show_status() -> Result<()> {
    // Things status
    let id_tracker = triage::id_tracker::IdTracker::new()?;
    let current = id_tracker.current()?;
    println!("=== Things 3 ===");
    println!("Task ID counter: #{:03}", current);

    let db = things::db::ThingsDb::new()?;
    let inbox = db.inbox_tasks()?;
    let agent_inbox = inbox.iter().filter(|t| t.is_agent_task()).count();
    println!(
        "Inbox: {} tasks ({} agent-tagged)",
        inbox.len(),
        agent_inbox
    );

    let today_tasks = db.agent_today_tasks()?;
    println!("Agent tasks today: {}", today_tasks.len());

    // Gas Town status
    println!("\n=== Gas Town ===");
    match gastown::client::GtClient::discover() {
        Ok(gt) => {
            match gt.status() {
                Ok(status) => {
                    println!("Town: {}", status.name);
                    if let Some(ref overseer) = status.overseer {
                        println!("Overseer: {}", overseer.name);
                    }
                    println!("Rigs: {}", status.rigs.len());

                    let total_agents: usize = status
                        .rigs
                        .iter()
                        .map(|r| r.agents.len())
                        .sum::<usize>()
                        + status.agents.len();
                    let running_agents: usize = status
                        .rigs
                        .iter()
                        .flat_map(|r| &r.agents)
                        .chain(&status.agents)
                        .filter(|a| a.running)
                        .count();
                    println!("Agents: {}/{} running", running_agents, total_agents);

                    if let Some(ref summary) = status.summary {
                        println!("Polecats: {}", summary.polecat_count);
                        println!("Active hooks: {}", summary.active_hooks);
                    }
                }
                Err(e) => println!("Could not fetch GT status: {}", e),
            }

            // Bead counts
            if let Ok(rigs) = gt.load_rigs() {
                let mut total_beads = 0;
                let mut open_beads = 0;
                for (name, _) in &rigs.rigs {
                    if let Ok(beads) = gt.list_beads(name) {
                        let user_beads: Vec<_> =
                            beads.iter().filter(|b| b.is_user_task()).collect();
                        total_beads += user_beads.len();
                        open_beads += user_beads
                            .iter()
                            .filter(|b| b.status == "open" || b.status == "in_progress")
                            .count();
                    }
                }
                println!("Beads: {}/{} open/in_progress", open_beads, total_beads);
            }

            // Sync map
            if let Ok(map) = sync::mapping::TaskBeadMap::load() {
                println!(
                    "\n=== Sync ===\nMapped tasks: {}",
                    map.things_to_bead.len()
                );
            }
        }
        Err(_) => {
            println!("Gas Town not connected (~/gt not found or gt CLI unavailable)");
        }
    }

    Ok(())
}
