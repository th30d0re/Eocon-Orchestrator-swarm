use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::app::App;
use crate::swarm::agent::{AgentLifecycleState, AgentOutput};

pub fn render(app: &App) -> Paragraph<'_> {
    let mut lines: Vec<Line> = Vec::new();

    for (idx, agent) in app.swarm_engine.agents.iter().enumerate() {
        let state_color = state_color(agent.state);
        let elapsed = agent.elapsed_secs();
        let task = agent.current_task.as_deref().unwrap_or("idle");
        let selected = idx == app.selected_agent_index;

        lines.push(Line::from(vec![
            Span::styled(
                if selected { "> " } else { "  " },
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(
                format!("{} ", agent.name),
                Style::default().fg(Color::White),
            ),
            Span::styled(
                format!("[{}]", agent.role.display_name()),
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw(" "),
            Span::styled(
                format!("{:?}", agent.state),
                Style::default().fg(state_color),
            ),
            Span::raw(" "),
            Span::styled(
                format!("{:02}:{:02}", elapsed / 60, elapsed % 60),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("    task: ", Style::default().fg(Color::DarkGray)),
            Span::styled(task.to_string(), Style::default().fg(Color::White)),
        ]));
    }

    if let Some(agent) = app.swarm_engine.agents.get(app.selected_agent_index) {
        lines.push(Line::from(vec![Span::styled(
            "--- detail ---",
            Style::default().fg(Color::DarkGray),
        )]));
        lines.push(Line::from(vec![
            Span::styled("memory: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{} messages", agent.memory.messages.len()),
                Style::default().fg(Color::White),
            ),
        ]));
        match &agent.output {
            AgentOutput::Pty(pty) => {
                let text = pty.screen.as_text();
                for line in text.lines {
                    lines.push(line);
                }
            }
            AgentOutput::Chat(chat) => {
                if !chat.pending_response.is_empty() {
                    lines.push(Line::from(vec![Span::styled(
                        chat.pending_response.clone(),
                        Style::default().fg(Color::White),
                    )]));
                } else if agent.state == AgentLifecycleState::Thinking {
                    let dots = ".".repeat(((app.tick_count / 10) % 3 + 1) as usize);
                    lines.push(Line::from(vec![Span::styled(
                        format!("thinking{}", dots),
                        Style::default().fg(Color::Yellow),
                    )]));
                }
            }
            AgentOutput::None => {}
        }
    }

    Paragraph::new(lines)
        .block(
            Block::default()
                .title("Workflow")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .wrap(Wrap { trim: true })
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
