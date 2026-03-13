mod agent;
mod spec;
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
#[command(about = "Spec-driven task automation powered by Things 3")]
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

    /// Show current task ID counter
    Status,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("specflow=info".parse()?))
        .with_target(false)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Ui => run_ui().await,
        Commands::Triage => run_triage().await,
        Commands::Run => run_agents().await,
        Commands::Status => show_status(),
    }
}

async fn run_ui() -> Result<()> {
    let agent_states = Arc::new(Mutex::new(Vec::new()));

    // Start agent loop in background
    let states_clone = agent_states.clone();
    tokio::spawn(async move {
        if let Err(e) = agent::runner::run_agent_loop(states_clone).await {
            eprintln!("Agent loop error: {}", e);
        }
    });

    // Run TUI
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

    println!("Starting agent loop. Scanning every 30 seconds...");
    println!("Press Ctrl+C to stop.\n");

    agent::runner::run_agent_loop(agent_states).await
}

fn show_status() -> Result<()> {
    let id_tracker = triage::id_tracker::IdTracker::new()?;
    let current = id_tracker.current()?;
    println!("Current task ID counter: #{:03}", current);

    let db = things::db::ThingsDb::new()?;

    let inbox = db.inbox_tasks()?;
    println!("Inbox: {} tasks pending triage", inbox.len());

    let today_tasks = db.agent_today_tasks()?;
    println!("Agent tasks for today: {}", today_tasks.len());

    let projects = db.get_projects_in_agents()?;
    println!("\nAgents area projects:");
    for p in &projects {
        println!("  - {}", p.title);
    }

    Ok(())
}
