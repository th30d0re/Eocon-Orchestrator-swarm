use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, ScrollbarState, Wrap},
};

use crate::app::{App, InteractionMessage};
use crate::ui::theme::{self, Voice};

pub fn render(app: &App, area: Rect, is_focused: bool) -> (Paragraph<'_>, ScrollbarState) {
    let lines: Vec<Line> = app
        .interaction
        .iter()
        .flat_map(|msg| message_lines(msg))
        .collect();

    let content_lines = lines.len();
    let visible_lines = area.height.saturating_sub(2) as usize;
    let max_scroll = content_lines.saturating_sub(visible_lines);
    let scroll = if app.chat_auto_scroll {
        0
    } else {
        (app.chat_scroll as usize).min(max_scroll)
    };

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
                .title(" Chat / Verified Gists ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color)),
        )
        .wrap(Wrap { trim: true });

    let scrollbar_state = ScrollbarState::new(content_lines)
        .position(content_lines.saturating_sub(visible_lines + scroll))
        .viewport_content_length(visible_lines);

    (paragraph, scrollbar_state)
}

fn message_lines(msg: &InteractionMessage) -> Vec<Line<'_>> {
    let prefix = match msg.voice {
        Voice::User => "[USER] ",
        Voice::Agent => "[AGENT] ",
        Voice::Shell => "[SHELL] ",
        Voice::Error => "[ERROR] ",
        Voice::Warning => "[WARN] ",
        Voice::Gist => "[GIST] ",
        Voice::System => "[SYS] ",
    };
    let prefix_style = msg
        .voice
        .style()
        .add_modifier(ratatui::style::Modifier::BOLD);
    let text_style = Style::default().fg(Color::White);

    msg.text
        .split('\n')
        .enumerate()
        .map(|(idx, line)| {
            if idx == 0 {
                Line::from(vec![
                    Span::styled(prefix, prefix_style),
                    Span::styled(line, text_style),
                ])
            } else {
                Line::from(vec![Span::raw("       "), Span::styled(line, text_style)])
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use crate::app::{App, InteractionMessage};
    use crate::config::RunnerBackend;
    use crate::ui::theme::Voice;

    use super::*;

    #[test]
    fn renders_interaction_messages() {
        let mut app = App::new(true, RunnerBackend::Pty);
        app.interaction.push(InteractionMessage {
            voice: Voice::User,
            text: "Hello swarm".to_string(),
        });
        app.interaction.push(InteractionMessage {
            voice: Voice::Gist,
            text: "Claim: use token sessions".to_string(),
        });
        app.interaction.push(InteractionMessage {
            voice: Voice::Agent,
            text: "Working on it".to_string(),
        });
        app.interaction.push(InteractionMessage {
            voice: Voice::System,
            text: "Synthetic agents loaded".to_string(),
        });

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

        assert!(text.contains("[USER]"));
        assert!(text.contains("Hello swarm"));
        assert!(text.contains("[GIST]"));
        assert!(text.contains("use token sessions"));
        assert!(text.contains("[SYS]"));
        assert!(text.contains("Synthetic agents loaded"));
        assert!(text.contains("[AGENT]"));
    }
}
