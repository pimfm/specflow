# SpecFlow

Spec-driven task automation CLI that bridges Things 3 with your development workflow. Inspired by [OpenSpec](https://openspec.dev/).

## What it does

SpecFlow automates the lifecycle of development tasks from idea to merge request:

**Phase 1 — Triage** reads your Things 3 inbox and processes agent-tagged tasks:
- Assigns incremental IDs (`#001`, `#002`, ...)
- Enhances titles and descriptions
- Generates implementation checklists
- Classifies into projects (Development, Business, Marketing, Customer Service)
- Schedules and moves tasks to the Agents area

**Phase 2 — Execute** continuously scans for today's agent tasks and runs them in parallel:
- Finds the affected code repository
- Creates a git worktree for the feature
- Writes a specification (`spec/`) with GIVEN/WHEN/THEN requirements
- Generates Gherkin test files
- Implements until tests pass
- Creates a GitLab merge request
- Marks the task complete with the MR link

## Installation

```bash
cargo install --path .
```

Or build directly:

```bash
cargo build --release
# Binary at ./target/release/specflow
```

## Requirements

- macOS with Things 3 installed
- Things 3 must have an "Agents" area with projects: Development, Business, Marketing, Customer Service
- Agent tags configured: `agent-queued`, `agent-running`, `agent-done`, `agent-error`
- For Phase 2: `glab` CLI for GitLab MR creation, Gradle for Kotlin projects

## Usage

```bash
# Check current status
specflow status

# Triage all agent-tagged inbox tasks
specflow triage

# Start the continuous agent execution loop
specflow run

# Launch the interactive TUI dashboard
specflow ui
```

### TUI Controls

| Key   | Action                     |
|-------|----------------------------|
| Tab   | Switch between tabs        |
| r     | Refresh inbox              |
| t     | Run triage on inbox        |
| q     | Quit                       |
| Up/Down | Scroll                   |

## Things 3 Workflow

1. Add a task to your Things 3 Inbox
2. Tag it with `agent-queued`
3. Run `specflow triage` (or press `t` in the TUI)
4. The task gets an ID, enhanced description, and moves to the right project
5. Run `specflow run` to have agents pick up and execute tasks
6. Tasks are completed with MR links when done

## Architecture

```
src/
├── main.rs           # CLI entry point (clap)
├── things/           # Things 3 integration
│   ├── db.rs         # SQLite database reader
│   ├── applescript.rs # AppleScript commands for mutations
│   └── model.rs      # Task/Project data models
├── triage/           # Phase 1: Inbox processing
│   ├── processor.rs  # Task analysis and enrichment
│   └── id_tracker.rs # Incremental ID counter
├── agent/            # Phase 2: Task execution
│   ├── executor.rs   # Single task execution pipeline
│   └── runner.rs     # Continuous scan loop
├── spec/             # Specification generation
│   ├── writer.rs     # Spec markdown files
│   └── gherkin.rs    # Gherkin feature files
└── tui/              # Ratatui terminal UI
    ├── app.rs        # Application state and event handling
    └── ui.rs         # Widget rendering
```
