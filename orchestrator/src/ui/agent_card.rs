use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::swarm::agent::{AgentInstance, AgentLifecycleState};

pub fn render<'a>(agent: &AgentInstance, is_selected: bool) -> Paragraph<'a> {
    let state_str = state_label(agent.state);
    let state_color = state_color(agent.state);
    let elapsed = agent.elapsed_secs();
    let task = agent
        .current_task
        .as_deref()
        .unwrap_or("idle");

    let border_color = if is_selected { Color::Cyan } else { Color::DarkGray };

    let lines = vec![
        Line::from(vec![
            Span::styled(
                format!("{} ", agent.name),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("[{}]", agent.role.display_name()),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(vec![
            Span::styled("state: ", Style::default().fg(Color::DarkGray)),
            Span::styled(state_str, Style::default().fg(state_color).add_modifier(Modifier::BOLD)),
            Span::raw(" | "),
            Span::styled(format!("{:02}:{:02}", elapsed / 60, elapsed % 60), Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("task: ", Style::default().fg(Color::DarkGray)),
            Span::styled(task.to_string(), Style::default().fg(Color::White)),
        ]),
    ];

    Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color)),
        )
}

fn state_label(state: AgentLifecycleState) -> &'static str {
    match state {
        AgentLifecycleState::Idle => "Idle",
        AgentLifecycleState::Thinking => "Thinking",
        AgentLifecycleState::Acting => "Acting",
        AgentLifecycleState::Paused => "Paused",
        AgentLifecycleState::Failed => "Failed",
        AgentLifecycleState::Completed => "Done",
        AgentLifecycleState::Preparing => "Preparing",
    }
}

fn state_color(state: AgentLifecycleState) -> Color {
    match state {
        AgentLifecycleState::Idle => Color::DarkGray,
        AgentLifecycleState::Thinking => Color::Yellow,
        AgentLifecycleState::Acting => Color::Cyan,
        AgentLifecycleState::Paused => Color::Blue,
        AgentLifecycleState::Failed => Color::Red,
        AgentLifecycleState::Completed => Color::Green,
        AgentLifecycleState::Preparing => Color::Magenta,
    }
}
