# SpecFlow

Spec-driven task automation CLI that bridges **Things 3** with **Gas Town** multi-agent orchestration. Takes tasks from your Things inbox, triages them into beads, dispatches them to Gas Town rigs via `gt sling`, and syncs completions back to Things.

## What it does

### Phase 1 — Triage
Reads your Things 3 inbox and processes agent-tagged tasks:
- Assigns incremental IDs (`#001`, `#002`, ...)
- Enhances titles and descriptions
- Generates implementation checklists
- Classifies into projects (Development, Business, Marketing, Customer Service)
- Schedules and moves tasks to the Agents area

### Phase 2 — Execute (Gas Town integrated)
Continuously scans for today's agent tasks and dispatches them through Gas Town:
- Finds the affected repository (checks GT rigs first, then local dirs)
- Creates a git worktree for the feature
- Writes a specification (`spec/`) with GIVEN/WHEN/THEN requirements
- Generates Gherkin test files
- Creates a bead in the matching Gas Town rig via `bd create`
- Pushes and creates a GitHub PR via `gh`
- Slings the bead to the rig for agent processing via `gt sling`

### Phase 3 — Sync
Watches for bead completions in Gas Town and marks the corresponding Things tasks complete:
- Maps Things task UUIDs to bead IDs (persisted in `~/.specflow/task_bead_map.json`)
- Detects closed beads via `bd list`
- Completes Things tasks via AppleScript
- Updates tags (`agent-done`) and notes with bead references

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
- [Gas Town](https://github.com/pimfm/gastown-tui) workspace at `~/gt/` with `gt` CLI installed
- `gh` CLI for GitHub PR creation
- `bd` CLI for bead management (comes with Gas Town)

## Usage

```bash
# Show combined Things + Gas Town status
specflow status

# Triage all inbox tasks
specflow triage

# Start the continuous agent loop (scans every 30s, syncs every 60s)
specflow run

# One-shot sync: check GT beads and complete Things tasks
specflow sync

# Launch the interactive TUI dashboard
specflow ui
```

### TUI Controls

| Key       | Action                              |
|-----------|-------------------------------------|
| Tab       | Switch between tabs                 |
| 1-5       | Jump to tab directly                |
| r         | Refresh all data                    |
| t         | Run triage on inbox                 |
| s         | Sync completions (GT -> Things)     |
| j/k       | Scroll up/down                      |
| q         | Quit                                |

### TUI Tabs

1. **Pipeline** — Dashboard with stat cards (inbox, today, active, beads, GT agents), task pipeline with bead/rig tracking, convoy progress, and rig overview
2. **Things** — Things 3 inbox and today's agent tasks
3. **Rigs** — Gas Town agent table and rig details (polecats, crews, witness, refinery, hooks, merge queue)
4. **Beads** — All user beads across rigs with status, priority, assignee
5. **Log** — Timestamped activity log

## Things 3 + Gas Town Workflow

1. Add a task to your Things 3 Inbox
2. Tag it with `agent-queued`
3. Run `specflow triage` (or press `t` in the TUI)
4. The task gets an ID, enhanced description, and moves to the right project
5. Run `specflow run` — the agent loop:
   - Picks up today's agent tasks from Things
   - Creates a bead in the matching Gas Town rig
   - Creates a worktree, writes specs, pushes a PR
   - Slings the bead to the rig for polecat processing
6. When a polecat completes the bead and closes it:
   - `specflow sync` (or press `s`) detects the closure
   - The corresponding Things task is marked complete with `agent-done` tag
   - Notes are updated with the bead reference

## Architecture

```
src/
├── main.rs           # CLI entry point (clap)
├── gastown/          # Gas Town integration
│   ├── client.rs     # gt/bd CLI wrapper (status, sling, beads, convoys)
│   └── model.rs      # GT data models (rigs, agents, beads, convoys)
├── sync/             # Bidirectional sync engine
│   ├── engine.rs     # Completion sync (GT beads -> Things tasks)
│   └── mapping.rs    # Task-to-bead ID mapping (persisted)
├── things/           # Things 3 integration
│   ├── db.rs         # SQLite database reader
│   ├── applescript.rs # AppleScript commands for mutations
│   └── model.rs      # Task/Project data models
├── triage/           # Phase 1: Inbox processing
│   ├── processor.rs  # Task analysis and enrichment
│   └── id_tracker.rs # Incremental ID counter
├── agent/            # Phase 2: Task execution
│   ├── executor.rs   # Single task pipeline (repo -> spec -> PR -> bead -> sling)
│   └── runner.rs     # Continuous scan loop with sync
├── spec/             # Specification generation
│   ├── writer.rs     # Spec markdown files
│   └── gherkin.rs    # Gherkin feature files
└── tui/              # Ratatui terminal UI (matches gastown-tui palette)
    ├── app.rs        # Application state, GT data, event handling
    └── ui.rs         # Widget rendering (pipeline, things, rigs, beads, log)
```
