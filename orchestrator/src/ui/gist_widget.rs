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
