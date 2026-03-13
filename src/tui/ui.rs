use ratatui::{
    layout::{Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Cell, LineGauge, List, ListItem, Padding, Paragraph, Row,
        Scrollbar, ScrollbarOrientation, ScrollbarState, Table, Tabs, Wrap,
    },
    Frame,
};

use crate::agent::runner::{AgentState, AgentStatus};

use super::app::{App, TAB_NAMES};

// ── Color palette (matches gastown-tui) ──────────────────────────────

const BG: Color = Color::Rgb(22, 22, 30);
const SURFACE: Color = Color::Rgb(30, 30, 42);
const BORDER: Color = Color::Rgb(60, 60, 80);
const ACCENT: Color = Color::Rgb(130, 170, 255);
const ACCENT_DIM: Color = Color::Rgb(80, 110, 180);
const GREEN: Color = Color::Rgb(120, 220, 140);
const YELLOW: Color = Color::Rgb(240, 200, 80);
const RED: Color = Color::Rgb(240, 100, 100);
const ORANGE: Color = Color::Rgb(240, 160, 80);
const MUTED: Color = Color::Rgb(100, 100, 120);
const TEXT: Color = Color::Rgb(200, 200, 220);
const TEXT_BRIGHT: Color = Color::Rgb(240, 240, 255);
const CYAN: Color = Color::Rgb(100, 220, 230);
const PURPLE: Color = Color::Rgb(180, 140, 255);

pub fn draw(frame: &mut Frame, app: &mut App, agent_states: &[AgentState]) {
    let area = frame.area();
    frame.render_widget(Block::new().style(Style::default().bg(BG)), area);

    let [header_area, body_area, footer_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(10),
        Constraint::Length(1),
    ])
    .areas(area);

    draw_header(frame, app, header_area);
    match app.tab {
        0 => draw_pipeline(frame, app, agent_states, body_area),
        1 => draw_things(frame, app, body_area),
        2 => draw_rigs(frame, app, body_area),
        3 => draw_beads(frame, app, body_area),
        4 => draw_log(frame, app, body_area),
        _ => {}
    }
    draw_footer(frame, app, agent_states, footer_area);
}

// ── Header ───────────────────────────────────────────────────────────

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let [title_area, tabs_area] =
        Layout::horizontal([Constraint::Length(24), Constraint::Min(40)]).areas(area);

    let spinner = ['◐', '◓', '◑', '◒'];
    let spin_char = spinner[(app.tick as usize / 2) % spinner.len()];

    let gt_indicator = if app.gt_connected {
        Span::styled(" GT", Style::default().fg(GREEN))
    } else {
        Span::styled(" GT", Style::default().fg(RED))
    };

    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            format!(" {spin_char} "),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "SPECFLOW",
            Style::default()
                .fg(TEXT_BRIGHT)
                .add_modifier(Modifier::BOLD),
        ),
        gt_indicator,
    ]))
    .block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(BORDER))
            .border_type(BorderType::Rounded),
    );
    frame.render_widget(title, title_area);

    let tab_titles: Vec<Line> = TAB_NAMES
        .iter()
        .enumerate()
        .map(|(i, t)| {
            Line::from(vec![
                Span::styled(
                    format!("{}", i + 1),
                    Style::default()
                        .fg(ACCENT_DIM)
                        .add_modifier(Modifier::DIM),
                ),
                Span::raw(":"),
                Span::styled(t.to_string(), Style::default().fg(TEXT)),
            ])
        })
        .collect();

    let tabs = Tabs::new(tab_titles)
        .select(app.tab)
        .highlight_style(
            Style::default()
                .fg(ACCENT)
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::UNDERLINED),
        )
        .divider(Span::styled(" | ", Style::default().fg(BORDER)))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(BORDER))
                .border_type(BorderType::Rounded),
        );
    frame.render_widget(tabs, tabs_area);
}

// ── Footer ───────────────────────────────────────────────────────────

fn draw_footer(frame: &mut Frame, app: &App, agent_states: &[AgentState], area: Rect) {
    let running = agent_states
        .iter()
        .filter(|s| s.status == AgentStatus::Running || s.status == AgentStatus::Dispatched)
        .count();
    let completed = agent_states
        .iter()
        .filter(|s| s.status == AgentStatus::Completed)
        .count();

    let gt_status = if app.gt_connected {
        Span::styled(" GT ", Style::default().fg(GREEN).bg(Color::Rgb(30, 50, 30)))
    } else {
        Span::styled(" GT ", Style::default().fg(RED).bg(Color::Rgb(50, 30, 30)))
    };

    let help = if let Some(ref msg) = app.status_msg {
        Line::from(vec![
            Span::styled(format!(" {msg} "), Style::default().fg(YELLOW)),
            Span::styled("  Esc:dismiss", Style::default().fg(MUTED)),
        ])
    } else {
        Line::from(vec![
            gt_status,
            Span::raw(" "),
            Span::styled(
                format!("Inbox:{} ", app.inbox_tasks.len()),
                Style::default().fg(TEXT),
            ),
            Span::styled(format!("Active:{running} "), Style::default().fg(CYAN)),
            Span::styled(format!("Done:{completed} "), Style::default().fg(GREEN)),
            Span::styled(
                format!("Rigs:{} ", app.rigs.len()),
                Style::default().fg(PURPLE),
            ),
            Span::styled(" | ", Style::default().fg(BORDER)),
            Span::styled("q", Style::default().fg(ACCENT)),
            Span::styled(":quit ", Style::default().fg(MUTED)),
            Span::styled("Tab", Style::default().fg(ACCENT)),
            Span::styled(":switch ", Style::default().fg(MUTED)),
            Span::styled("r", Style::default().fg(ACCENT)),
            Span::styled(":refresh ", Style::default().fg(MUTED)),
            Span::styled("t", Style::default().fg(ACCENT)),
            Span::styled(":triage ", Style::default().fg(MUTED)),
            Span::styled("s", Style::default().fg(ACCENT)),
            Span::styled(":sync", Style::default().fg(MUTED)),
        ])
    };

    let footer = Paragraph::new(help).style(Style::default().bg(SURFACE));
    frame.render_widget(footer, area);
}

// ── Pipeline tab (dashboard) ─────────────────────────────────────────

fn draw_pipeline(frame: &mut Frame, app: &mut App, agent_states: &[AgentState], area: Rect) {
    let [top, mid, bottom] = Layout::vertical([
        Constraint::Length(5),
        Constraint::Min(8),
        Constraint::Min(8),
    ])
    .areas(area);

    // Stat cards
    let [card1, card2, card3, card4, card5] = Layout::horizontal([
        Constraint::Percentage(20),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
    ])
    .areas(top);

    let inbox_count = app.inbox_tasks.len();
    let agent_inbox = app.inbox_tasks.iter().filter(|t| t.has_agent_tag).count();
    draw_stat_card(
        frame,
        card1,
        "Things Inbox",
        &inbox_count.to_string(),
        &format!("{} agent-tagged", agent_inbox),
        if agent_inbox > 0 { YELLOW } else { MUTED },
    );

    let today_count = app.agent_today.len();
    draw_stat_card(
        frame,
        card2,
        "Today",
        &today_count.to_string(),
        "agent tasks",
        if today_count > 0 { CYAN } else { MUTED },
    );

    let dispatched = agent_states
        .iter()
        .filter(|s| s.status == AgentStatus::Dispatched || s.status == AgentStatus::Running)
        .count();
    draw_stat_card(
        frame,
        card3,
        "Active",
        &dispatched.to_string(),
        "dispatched",
        if dispatched > 0 { GREEN } else { MUTED },
    );

    let user_beads = app.user_beads();
    let open_beads = user_beads.iter().filter(|(_, b)| b.status == "open").count();
    let in_prog = user_beads
        .iter()
        .filter(|(_, b)| b.status == "in_progress")
        .count();
    draw_stat_card(
        frame,
        card4,
        "Beads",
        &format!("{}/{}", in_prog, user_beads.len()),
        &format!("{open_beads} open"),
        ACCENT,
    );

    draw_stat_card(
        frame,
        card5,
        "GT Agents",
        &format!("{}/{}", app.agents_running(), app.agents.len()),
        &format!("{} working", app.agents_with_work()),
        if app.agents_running() > 0 { GREEN } else { MUTED },
    );

    // Agent task pipeline
    let [pipeline_area, convoy_area] =
        Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)]).areas(mid);
    draw_agent_pipeline(frame, agent_states, pipeline_area);
    draw_convoy_summary(frame, app, convoy_area);

    // Rig overview
    draw_rig_overview(frame, app, bottom);
}

fn draw_stat_card(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    value: &str,
    suffix: &str,
    color: Color,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
        .style(Style::default().bg(SURFACE))
        .padding(Padding::horizontal(1));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [label_area, value_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(2)]).areas(inner);

    frame.render_widget(
        Paragraph::new(Span::styled(label, Style::default().fg(MUTED))),
        label_area,
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                value,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!(" {suffix}"), Style::default().fg(MUTED)),
        ])),
        value_area,
    );
}

fn draw_agent_pipeline(frame: &mut Frame, agent_states: &[AgentState], area: Rect) {
    let block = Block::default()
        .title(Span::styled(
            " Task Pipeline ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
        .style(Style::default().bg(SURFACE))
        .padding(Padding::new(1, 1, 0, 0));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if agent_states.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "No tasks in pipeline. Tag tasks with 'agent-queued' in Things, then press 't' to triage.",
                Style::default().fg(MUTED),
            ))
            .wrap(Wrap { trim: false }),
            inner,
        );
        return;
    }

    let items: Vec<ListItem> = agent_states
        .iter()
        .map(|state| {
            let (status_color, status_icon) = match state.status {
                AgentStatus::Queued => (YELLOW, "..."),
                AgentStatus::Running => (CYAN, ">>>"),
                AgentStatus::Dispatched => (GREEN, " > "),
                AgentStatus::Completed => (GREEN, " + "),
                AgentStatus::Failed => (RED, " X "),
            };

            let bead_str = state
                .bead_id
                .as_ref()
                .map(|b| format!(" [{}]", b))
                .unwrap_or_default();

            let rig_str = state
                .rig
                .as_ref()
                .map(|r| format!(" @{}", r))
                .unwrap_or_default();

            let error_str = state
                .error
                .as_ref()
                .map(|e| {
                    let truncated = if e.len() > 40 {
                        format!("{}...", &e[..40])
                    } else {
                        e.clone()
                    };
                    format!(" - {}", truncated)
                })
                .unwrap_or_default();

            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("[{}] ", status_icon),
                    Style::default()
                        .fg(status_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{:<10} ", state.status),
                    Style::default().fg(status_color),
                ),
                Span::styled(&state.task_title, Style::default().fg(TEXT_BRIGHT)),
                Span::styled(bead_str, Style::default().fg(ACCENT_DIM)),
                Span::styled(rig_str, Style::default().fg(PURPLE)),
                Span::styled(error_str, Style::default().fg(RED)),
            ]))
        })
        .collect();

    frame.render_widget(List::new(items), inner);
}

fn draw_convoy_summary(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(Span::styled(
            " Convoys ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
        .style(Style::default().bg(SURFACE))
        .padding(Padding::new(1, 1, 0, 0));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let active: Vec<_> = app.convoys.iter().filter(|c| c.status == "open").collect();

    if active.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "No active convoys.",
                Style::default().fg(MUTED),
            )),
            inner,
        );
        return;
    }

    let constraints: Vec<Constraint> = active
        .iter()
        .take(inner.height as usize / 3)
        .map(|_| Constraint::Length(3))
        .collect();
    let rows = Layout::vertical(constraints).split(inner);

    for (i, convoy) in active.iter().enumerate() {
        if i >= rows.len() {
            break;
        }
        let [line1, line2, _] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(rows[i]);

        let progress = if convoy.total > 0 {
            convoy.completed as f64 / convoy.total as f64
        } else {
            0.0
        };

        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(&convoy.id, Style::default().fg(ACCENT_DIM)),
                Span::raw(" "),
                Span::styled(
                    &convoy.title,
                    Style::default()
                        .fg(TEXT_BRIGHT)
                        .add_modifier(Modifier::BOLD),
                ),
            ])),
            line1,
        );

        let gauge_color = match progress {
            p if p >= 1.0 => GREEN,
            p if p >= 0.5 => CYAN,
            _ => ORANGE,
        };
        let gauge = LineGauge::default()
            .filled_style(Style::default().fg(gauge_color))
            .unfilled_style(Style::default().fg(Color::Rgb(40, 40, 55)))
            .line_set(symbols::line::THICK)
            .label(Span::styled(
                format!(
                    " {}/{} ({:.0}%)",
                    convoy.completed,
                    convoy.total,
                    progress * 100.0
                ),
                Style::default().fg(TEXT),
            ))
            .ratio(progress);
        frame.render_widget(gauge, line2);
    }
}

fn draw_rig_overview(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(Span::styled(
            " Gas Town Rigs ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
        .style(Style::default().bg(SURFACE));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.rigs.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                if app.gt_connected {
                    "No rigs registered."
                } else {
                    "Gas Town not connected. Install gt and run 'gt init'."
                },
                Style::default().fg(MUTED),
            )),
            inner,
        );
        return;
    }

    let header = Row::new(vec![
        Cell::from("Rig")
            .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        Cell::from("Polecats")
            .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        Cell::from("Crew")
            .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        Cell::from("Wit")
            .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        Cell::from("Ref")
            .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        Cell::from("Hooks")
            .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        Cell::from("MQ")
            .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
    ]);

    let rows: Vec<Row> = app
        .rigs
        .iter()
        .map(|rig| {
            let active_hooks = rig.hooks.iter().filter(|h| h.has_work).count();
            let mq_str = rig
                .mq
                .as_ref()
                .map(|mq| format!("{}p/{}f", mq.pending, mq.in_flight))
                .unwrap_or_else(|| "-".to_string());
            let mq_color = rig
                .mq
                .as_ref()
                .map(|mq| {
                    if mq.blocked > 0 {
                        RED
                    } else if mq.in_flight > 0 {
                        CYAN
                    } else {
                        MUTED
                    }
                })
                .unwrap_or(MUTED);

            Row::new(vec![
                Cell::from(rig.name.clone()).style(
                    Style::default()
                        .fg(TEXT_BRIGHT)
                        .add_modifier(Modifier::BOLD),
                ),
                Cell::from(rig.polecat_count.to_string()).style(Style::default().fg(
                    if rig.polecat_count > 0 {
                        GREEN
                    } else {
                        MUTED
                    },
                )),
                Cell::from(rig.crew_count.to_string()).style(Style::default().fg(MUTED)),
                Cell::from(if rig.has_witness { "+" } else { "-" }).style(
                    Style::default().fg(if rig.has_witness { GREEN } else { MUTED }),
                ),
                Cell::from(if rig.has_refinery { "+" } else { "-" }).style(
                    Style::default().fg(if rig.has_refinery { GREEN } else { MUTED }),
                ),
                Cell::from(format!("{}/{}", active_hooks, rig.hooks.len())).style(
                    Style::default().fg(if active_hooks > 0 { YELLOW } else { MUTED }),
                ),
                Cell::from(mq_str).style(Style::default().fg(mq_color)),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(16),
        Constraint::Length(9),
        Constraint::Length(6),
        Constraint::Length(5),
        Constraint::Length(5),
        Constraint::Length(8),
        Constraint::Length(10),
    ];

    let table = Table::new(rows, widths).header(header);
    frame.render_widget(table, inner);
}

// ── Things tab ───────────────────────────────────────────────────────

fn draw_things(frame: &mut Frame, app: &mut App, area: Rect) {
    let [inbox_area, today_area] =
        Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(area);

    // Inbox
    let inbox_block = Block::default()
        .title(Span::styled(
            format!(" Things Inbox ({}) ", app.inbox_tasks.len()),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
        .style(Style::default().bg(SURFACE))
        .padding(Padding::new(1, 1, 0, 0));
    let inbox_inner = inbox_block.inner(inbox_area);
    frame.render_widget(inbox_block, inbox_area);

    if app.inbox_tasks.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "Inbox empty. Add tasks in Things 3, tag with 'agent-queued', press 't' to triage.",
                Style::default().fg(MUTED),
            ))
            .wrap(Wrap { trim: false }),
            inbox_inner,
        );
    } else {
        let items: Vec<ListItem> = app
            .inbox_tasks
            .iter()
            .map(|task| {
                let (marker, color) = if task.has_agent_tag {
                    (">", GREEN)
                } else {
                    (" ", TEXT)
                };

                let tag_str = if task.tags.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", task.tags.join(", "))
                };

                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("{marker} "),
                        Style::default().fg(color).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(&task.title, Style::default().fg(color)),
                    Span::styled(tag_str, Style::default().fg(MUTED)),
                ]))
            })
            .collect();

        frame.render_widget(List::new(items), inbox_inner);
    }

    // Today's agent tasks
    let today_block = Block::default()
        .title(Span::styled(
            format!(" Today's Agent Tasks ({}) ", app.agent_today.len()),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
        .style(Style::default().bg(SURFACE))
        .padding(Padding::new(1, 1, 0, 0));
    let today_inner = today_block.inner(today_area);
    frame.render_widget(today_block, today_area);

    if app.agent_today.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "No agent tasks scheduled for today.",
                Style::default().fg(MUTED),
            )),
            today_inner,
        );
    } else {
        let items: Vec<ListItem> = app
            .agent_today
            .iter()
            .map(|task| {
                let tag_str = task
                    .tags
                    .iter()
                    .find(|t| t.starts_with("agent-"))
                    .cloned()
                    .unwrap_or_default();

                let tag_color = match tag_str.as_str() {
                    "agent-queued" => YELLOW,
                    "agent-running" => CYAN,
                    "agent-done" => GREEN,
                    "agent-error" => RED,
                    _ => MUTED,
                };

                let project_str = task
                    .project
                    .as_ref()
                    .map(|p| format!(" @{}", p))
                    .unwrap_or_default();

                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("{:<14} ", tag_str),
                        Style::default().fg(tag_color).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(&task.title, Style::default().fg(TEXT_BRIGHT)),
                    Span::styled(project_str, Style::default().fg(PURPLE)),
                ]))
            })
            .collect();

        frame.render_widget(List::new(items), today_inner);
    }
}

// ── Rigs tab ─────────────────────────────────────────────────────────

fn draw_rigs(frame: &mut Frame, app: &mut App, area: Rect) {
    let [agents_area, rigs_detail_area] =
        Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(area);

    // GT Agents table
    let agent_count = app.agents.len();
    app.max_scroll = agent_count.saturating_sub(1);

    let header = Row::new(vec![
        Cell::from("Name")
            .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        Cell::from("Address")
            .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        Cell::from("Role")
            .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        Cell::from("State")
            .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        Cell::from("Run")
            .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        Cell::from("Work")
            .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        Cell::from("Hook Bead")
            .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
    ]);

    let rows: Vec<Row> = app
        .agents
        .iter()
        .skip(app.scroll)
        .map(|agent| {
            let running_color = if agent.running { GREEN } else { MUTED };
            let work_color = if agent.has_work { CYAN } else { MUTED };
            let state = agent.state.as_deref().unwrap_or("-");
            let state_color = match state {
                "active" | "working" => GREEN,
                "idle" => MUTED,
                "stuck" => YELLOW,
                _ => TEXT,
            };
            let hook = agent.hook_bead.as_deref().unwrap_or("-");

            Row::new(vec![
                Cell::from(agent.name.clone())
                    .style(Style::default().fg(TEXT_BRIGHT)),
                Cell::from(agent.address.clone())
                    .style(Style::default().fg(ACCENT_DIM)),
                Cell::from(agent.role.as_deref().unwrap_or("-").to_string())
                    .style(Style::default().fg(PURPLE)),
                Cell::from(state.to_string()).style(
                    Style::default()
                        .fg(state_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Cell::from(if agent.running { "+" } else { "-" })
                    .style(Style::default().fg(running_color)),
                Cell::from(if agent.has_work { "+" } else { "-" })
                    .style(Style::default().fg(work_color)),
                Cell::from(hook.to_string()).style(Style::default().fg(
                    if hook != "-" { CYAN } else { MUTED },
                )),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(12),
        Constraint::Length(22),
        Constraint::Length(12),
        Constraint::Length(8),
        Constraint::Length(5),
        Constraint::Length(6),
        Constraint::Length(14),
    ];

    let table = Table::new(rows, widths)
        .header(header.style(Style::default().bg(SURFACE)))
        .block(
            Block::default()
                .title(Span::styled(
                    format!(" GT Agents ({agent_count}) "),
                    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(BORDER))
                .style(Style::default().bg(SURFACE)),
        );
    frame.render_widget(table, agents_area);

    // Scrollbar for agents
    let sb_area = agents_area.inner(Margin {
        vertical: 1,
        horizontal: 0,
    });
    let mut sb_state = ScrollbarState::new(agent_count).position(app.scroll);
    frame.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .thumb_style(Style::default().fg(ACCENT_DIM))
            .track_style(Style::default().fg(Color::Rgb(35, 35, 50))),
        sb_area,
        &mut sb_state,
    );

    // Rig details
    draw_rig_overview(frame, app, rigs_detail_area);
}

// ── Beads tab ────────────────────────────────────────────────────────

fn draw_beads(frame: &mut Frame, app: &mut App, area: Rect) {
    let count = app.user_beads().len();
    app.max_scroll = count.saturating_sub(1);
    let beads = app.user_beads();

    let header = Row::new(vec![
        Cell::from("ID")
            .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        Cell::from("P")
            .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        Cell::from("Status")
            .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        Cell::from("Type")
            .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        Cell::from("Title")
            .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        Cell::from("Rig")
            .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        Cell::from("Assignee")
            .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
    ]);

    let rows: Vec<Row> = beads
        .iter()
        .skip(app.scroll)
        .map(|(rig, bead)| {
            let status_color = match bead.status.as_str() {
                "open" => MUTED,
                "in_progress" => CYAN,
                "closed" => GREEN,
                "blocked" => RED,
                _ => MUTED,
            };
            let priority_color = match bead.priority {
                0 => RED,
                1 => ORANGE,
                2 => YELLOW,
                3 => MUTED,
                _ => Color::Rgb(50, 50, 60),
            };
            Row::new(vec![
                Cell::from(bead.id.clone()).style(Style::default().fg(ACCENT_DIM)),
                Cell::from(format!("P{}", bead.priority))
                    .style(Style::default().fg(priority_color)),
                Cell::from(bead.status.clone()).style(
                    Style::default()
                        .fg(status_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Cell::from(bead.issue_type.clone()).style(Style::default().fg(MUTED)),
                Cell::from(bead.title.clone()).style(Style::default().fg(TEXT)),
                Cell::from(rig.clone()).style(Style::default().fg(PURPLE)),
                Cell::from(
                    bead.assignee
                        .clone()
                        .unwrap_or_else(|| "-".into()),
                )
                .style(Style::default().fg(CYAN)),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(12),
        Constraint::Length(4),
        Constraint::Length(12),
        Constraint::Length(10),
        Constraint::Min(30),
        Constraint::Length(14),
        Constraint::Length(14),
    ];

    let table = Table::new(rows, widths)
        .header(header.style(Style::default().bg(SURFACE)))
        .block(
            Block::default()
                .title(Span::styled(
                    format!(" Beads ({count}) "),
                    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(BORDER))
                .style(Style::default().bg(SURFACE)),
        );
    frame.render_widget(table, area);

    let sb_area = area.inner(Margin {
        vertical: 1,
        horizontal: 0,
    });
    let mut sb_state = ScrollbarState::new(count).position(app.scroll);
    frame.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .thumb_style(Style::default().fg(ACCENT_DIM))
            .track_style(Style::default().fg(Color::Rgb(35, 35, 50))),
        sb_area,
        &mut sb_state,
    );
}

// ── Log tab ──────────────────────────────────────────────────────────

fn draw_log(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(Span::styled(
            " Log ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
        .style(Style::default().bg(SURFACE))
        .padding(Padding::new(1, 1, 0, 0));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let visible_height = inner.height as usize;
    let total = app.log_messages.len();
    let start = if total > visible_height {
        total - visible_height
    } else {
        0
    };

    let text: Vec<Line> = app.log_messages[start..]
        .iter()
        .map(|msg| {
            let color = if msg.contains("Error") || msg.contains("error") || msg.contains("Failed")
            {
                RED
            } else if msg.contains("Triaged") || msg.contains("Completed") || msg.contains("Sync:")
            {
                GREEN
            } else if msg.contains("dispatched") || msg.contains("Slung") {
                CYAN
            } else {
                TEXT
            };
            Line::from(Span::styled(msg.as_str(), Style::default().fg(color)))
        })
        .collect();

    let paragraph = Paragraph::new(text).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}
