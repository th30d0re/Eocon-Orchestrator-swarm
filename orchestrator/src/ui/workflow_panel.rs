use ratatui::{
    layout::{Margin, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    Frame,
};

use crate::app::App;
use crate::swarm::agent::{AgentLifecycleState, AgentOutput};
use crate::term::TerminalPaneWidget;
use crate::ui::theme;

/// Render the workflow panel. Returns the rectangle used for a PTY terminal
/// widget when the selected agent has `AgentOutput::Pty`, so the caller can
/// record the pane dimensions for resize propagation.
pub fn render(app: &App, frame: &mut Frame, area: Rect, is_focused: bool) -> Option<Rect> {
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
                // Render PTY agents with the faithful terminal widget. The
                // caller records the widget area for emulator/PTY resize
                // propagation.
                return Some(render_pty_agent(frame, area, is_focused, agent, pty));
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

    let border_color = if is_focused {
        theme::ACCENT
    } else {
        Color::DarkGray
    };
    let block = Block::default()
        .title(" Workflow ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let content_lines = lines.len();
    let visible_lines = inner.height.saturating_sub(0) as usize;
    let max_scroll = content_lines.saturating_sub(visible_lines);
    let scroll = (app.workflow_scroll as usize).min(max_scroll);

    // Scroll offset is measured from the bottom (0 = show the most recent content).
    let end = content_lines.saturating_sub(scroll);
    let start = end.saturating_sub(visible_lines);
    let visible = &lines[start..end];

    let paragraph = Paragraph::new(visible.to_vec())
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, inner);

    let mut scrollbar_state = ScrollbarState::new(content_lines)
        .position(content_lines.saturating_sub(visible_lines + scroll))
        .viewport_content_length(visible_lines);
    frame.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .style(Style::default().fg(Color::DarkGray)),
        area.inner(Margin {
            vertical: 1,
            horizontal: 0,
        }),
        &mut scrollbar_state,
    );

    None
}

fn render_pty_agent(
    frame: &mut Frame,
    area: Rect,
    is_focused: bool,
    agent: &crate::swarm::agent::AgentInstance,
    pty: &crate::swarm::agent::PtyOutput,
) -> Rect {
    let border_color = if is_focused {
        theme::ACCENT
    } else {
        Color::DarkGray
    };
    let block = Block::default()
        .title(" Workflow ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // One-line status header so the operator still sees which agent is shown.
    let header = Line::from(vec![
        Span::styled(format!("{} ", agent.name), Style::default().fg(Color::White)),
        Span::styled(
            format!("[{}]", agent.role.display_name()),
            Style::default().fg(Color::DarkGray),
        ),
        Span::raw(" "),
        Span::styled(
            format!("{:?}", agent.state),
            Style::default().fg(state_color(agent.state)),
        ),
    ]);
    let header_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    };
    frame.render_widget(Paragraph::new(header), header_area);

    let widget_area = Rect {
        x: inner.x,
        y: inner.y + 1,
        width: inner.width,
        height: inner.height.saturating_sub(1),
    };
    frame.render_widget(
        TerminalPaneWidget::new(&pty.screen).attached(is_focused),
        widget_area,
    );

    widget_area
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
    use crate::config::RunnerBackend;

    use super::*;

    #[test]
    fn renders_agents_states_and_selection() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new(true, RunnerBackend::Pty);
        terminal
            .draw(|frame| {
                render(&app, frame, frame.area(), false);
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
