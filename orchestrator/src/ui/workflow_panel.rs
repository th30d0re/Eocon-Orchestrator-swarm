use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, ScrollbarState, Wrap},
};

use crate::app::App;
use crate::swarm::agent::{AgentLifecycleState, AgentOutput};
use crate::ui::theme;

pub fn render(app: &App, area: Rect, is_focused: bool) -> (Paragraph<'_>, ScrollbarState) {
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
                // Render the full scrollback so the pane's own bottom-anchored
                // windowing (workflow_scroll) can scroll back into build history.
                let text = pty.screen.full_text();
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
            AgentOutput::SwiftResult(result) => {
                let status = if result.success && result.exit_code == 0 {
                    "success"
                } else {
                    "failure"
                };
                lines.push(Line::from(vec![
                    Span::styled("status: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("{} (exit {})", status, result.exit_code),
                        Style::default().fg(if status == "success" {
                            Color::Green
                        } else {
                            Color::Red
                        }),
                    ),
                ]));
                if !result.output.is_empty() {
                    lines.push(Line::from(vec![Span::styled(
                        result.output.clone(),
                        Style::default().fg(Color::White),
                    )]));
                }
                if !result.error.is_empty() {
                    lines.push(Line::from(vec![Span::styled(
                        result.error.clone(),
                        Style::default().fg(Color::Red),
                    )]));
                }
            }
            AgentOutput::None => {}
        }
    }

    let content_lines = lines.len();
    let visible_lines = area.height.saturating_sub(2) as usize;
    let max_scroll = content_lines.saturating_sub(visible_lines);
    let scroll = (app.workflow_scroll as usize).min(max_scroll);

    // Scroll offset is measured from the bottom (0 = show the most recent content).
    let end = content_lines.saturating_sub(scroll);
    let start = end.saturating_sub(visible_lines);
    let visible = &lines[start..end];

    let border_color = if is_focused {
        theme::ACCENT
    } else {
        Color::DarkGray
    };

    let paragraph = Paragraph::new(visible.to_vec())
        .block(
            Block::default()
                .title(" Workflow ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color)),
        )
        .wrap(Wrap { trim: true });

    let scrollbar_state = ScrollbarState::new(content_lines)
        .position(content_lines.saturating_sub(visible_lines + scroll))
        .viewport_content_length(visible_lines);

    (paragraph, scrollbar_state)
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

#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use crate::app::App;

    use super::*;

    #[test]
    fn renders_agents_states_and_selection() {
        let app = App::new(true);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let (paragraph, _) = render(&app, frame.area(), false);
                frame.render_widget(paragraph, frame.area());
            })
            .unwrap();

        let buf = terminal.backend().buffer();
        let text: String = buf.content.iter().map(|c| c.symbol()).collect();

        assert!(text.contains("planner"));
        assert!(text.contains("coder"));
        assert!(text.contains("Thinking"));
        assert!(text.contains("Acting"));
        assert!(text.contains("[planner]"));
        assert!(text.contains(">"));
        assert!(text.contains("task:"));
    }
}
