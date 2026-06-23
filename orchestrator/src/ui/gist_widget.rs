use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::swarm::gist::{Gist, GistStatus};

pub fn render(gist: &Gist) -> Paragraph<'_> {
    let status_color = match gist.status {
        GistStatus::Verified => Color::Green,
        GistStatus::Rejected => Color::Red,
        GistStatus::Verifying => Color::Yellow,
        GistStatus::Draft => Color::DarkGray,
    };

    let lines = vec![
        Line::from(vec![
            Span::styled(
                format!("{} ", gist.agent_name),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(
                format!("[{:?}]", gist.status),
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![Span::styled(
            gist.claim.clone(),
            Style::default().fg(Color::White),
        )]),
        Line::from(vec![Span::styled(
            format!("evidence: {}", gist.evidence.len()),
            Style::default().fg(Color::DarkGray),
        )]),
    ];

    Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    )
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use crate::swarm::gist::{Gist, GistSource, GistStatus};

    use super::*;

    fn sample_gist() -> Gist {
        Gist {
            schema_version: 1,
            id: "g1".to_string(),
            agent_id: "a1".to_string(),
            agent_name: "coder".to_string(),
            claim: "Use token sessions".to_string(),
            summary: "REST-first session design".to_string(),
            evidence: vec![
                "API uses JWT".to_string(),
                "OAuth2 out of scope".to_string(),
            ],
            source_type: GistSource::ModelClaim,
            peer_rank: None,
            status: GistStatus::Verified,
            created_at: Instant::now(),
        }
    }

    #[test]
    fn renders_gist_claim_and_status() {
        let gist = sample_gist();
        let backend = TestBackend::new(40, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                frame.render_widget(render(&gist), frame.area());
            })
            .unwrap();

        let buf = terminal.backend().buffer();
        let text: String = buf.content.iter().map(|c| c.symbol()).collect();

        assert!(text.contains("coder"));
        assert!(text.contains("Verified"));
        assert!(text.contains(&gist.claim));
        assert!(text.contains("evidence: 2"));
    }
}
