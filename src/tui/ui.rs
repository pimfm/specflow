use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs, Wrap},
    Frame,
};

use crate::agent::runner::{AgentState, AgentStatus};
use super::app::{App, Tab};

pub fn draw(frame: &mut Frame, app: &App, agent_states: &[AgentState]) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Tab bar
            Constraint::Min(10),   // Main content
            Constraint::Length(3), // Status bar
        ])
        .split(frame.area());

    draw_tabs(frame, app, chunks[0]);

    match app.selected_tab {
        Tab::Inbox => draw_inbox(frame, app, chunks[1]),
        Tab::Agents => draw_agents(frame, agent_states, chunks[1]),
        Tab::Log => draw_log(frame, app, chunks[1]),
    }

    draw_status_bar(frame, app, agent_states, chunks[2]);
}

fn draw_tabs(frame: &mut Frame, app: &App, area: Rect) {
    let titles = vec!["Inbox [1]", "Agents [2]", "Log [3]"];
    let selected = match app.selected_tab {
        Tab::Inbox => 0,
        Tab::Agents => 1,
        Tab::Log => 2,
    };

    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title(" SpecFlow "))
        .select(selected)
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_widget(tabs, area);
}

fn draw_inbox(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .inbox_tasks
        .iter()
        .map(|task| {
            let style = if task.has_agent_tag {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::Gray)
            };

            let tag_str = if task.tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", task.tags.join(", "))
            };

            let marker = if task.has_agent_tag { ">" } else { " " };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{} ", marker), style.add_modifier(Modifier::BOLD)),
                Span::styled(&task.title, style),
                Span::styled(tag_str, Style::default().fg(Color::DarkGray)),
            ]))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Inbox (r=refresh, t=triage) "),
    );

    frame.render_widget(list, area);
}

fn draw_agents(frame: &mut Frame, agent_states: &[AgentState], area: Rect) {
    let items: Vec<ListItem> = agent_states
        .iter()
        .map(|state| {
            let (status_color, status_icon) = match state.status {
                AgentStatus::Queued => (Color::Yellow, "..."),
                AgentStatus::Running => (Color::Blue, ">>>"),
                AgentStatus::Completed => (Color::Green, " + "),
                AgentStatus::Failed => (Color::Red, " X "),
            };

            let error_str = state
                .error
                .as_ref()
                .map(|e| format!(" - {}", e))
                .unwrap_or_default();

            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("[{}] ", status_icon),
                    Style::default().fg(status_color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{} ", state.status),
                    Style::default().fg(status_color),
                ),
                Span::styled(&state.task_title, Style::default().fg(Color::White)),
                Span::styled(error_str, Style::default().fg(Color::Red)),
            ]))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Agent Tasks "),
    );

    frame.render_widget(list, area);
}

fn draw_log(frame: &mut Frame, app: &App, area: Rect) {
    let visible_height = area.height.saturating_sub(2) as usize;
    let total = app.log_messages.len();
    let start = if total > visible_height {
        total - visible_height
    } else {
        0
    };

    let text: Vec<Line> = app.log_messages[start..]
        .iter()
        .map(|msg| {
            let color = if msg.contains("Error") || msg.contains("error") || msg.contains("Failed") {
                Color::Red
            } else if msg.contains("Triaged") || msg.contains("Completed") {
                Color::Green
            } else {
                Color::White
            };
            Line::from(Span::styled(msg.as_str(), Style::default().fg(color)))
        })
        .collect();

    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title(" Log "))
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

fn draw_status_bar(frame: &mut Frame, app: &App, agent_states: &[AgentState], area: Rect) {
    let running = agent_states
        .iter()
        .filter(|s| s.status == AgentStatus::Running)
        .count();
    let completed = agent_states
        .iter()
        .filter(|s| s.status == AgentStatus::Completed)
        .count();
    let failed = agent_states
        .iter()
        .filter(|s| s.status == AgentStatus::Failed)
        .count();
    let inbox_count = app.inbox_tasks.len();
    let agent_inbox = app
        .inbox_tasks
        .iter()
        .filter(|t| t.has_agent_tag)
        .count();

    let status = Line::from(vec![
        Span::styled(
            format!(" Inbox: {} ({} agent) ", inbox_count, agent_inbox),
            Style::default().fg(Color::White),
        ),
        Span::styled("| ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("Running: {} ", running),
            Style::default().fg(Color::Blue),
        ),
        Span::styled(
            format!("Done: {} ", completed),
            Style::default().fg(Color::Green),
        ),
        Span::styled(
            format!("Failed: {} ", failed),
            Style::default().fg(Color::Red),
        ),
        Span::styled(
            " | q=quit Tab=switch r=refresh t=triage",
            Style::default().fg(Color::DarkGray),
        ),
    ]);

    let paragraph = Paragraph::new(status).block(Block::default().borders(Borders::ALL));
    frame.render_widget(paragraph, area);
}
