use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::app::{App, InteractionMessage};
use crate::ui::theme::Voice;

pub fn render(app: &App) -> Paragraph<'_> {
    let lines: Vec<Line> = app
        .interaction
        .iter()
        .flat_map(|msg| message_lines(msg))
        .collect();

    Paragraph::new(lines)
        .block(
            Block::default()
                .title("Chat / Verified Gists")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .wrap(Wrap { trim: true })
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
    use crate::ui::theme::Voice;

    use super::*;

    #[test]
    fn renders_interaction_messages() {
        let mut app = App::new(true);
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
                frame.render_widget(render(&app), frame.area());
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
