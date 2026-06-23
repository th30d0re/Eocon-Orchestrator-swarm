use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::App;

pub fn render(app: &App) -> Paragraph<'_> {
    let layout = format!("{:?}", app.swarm_layout);
    let agent_count = app.swarm_engine.agents.len();
    let gist_count = app.swarm_engine.gist_board.verified().len();

    let line = Line::from(vec![
        Span::styled("Eocon Swarm", Style::default().fg(Color::White)),
        Span::raw(" | "),
        Span::styled(format!("layout: {}", layout), Style::default().fg(Color::Cyan)),
        Span::raw(" | "),
        Span::styled(format!("agents: {}", agent_count), Style::default().fg(Color::Cyan)),
        Span::raw(" | "),
        Span::styled(format!("gists: {}", gist_count), Style::default().fg(Color::Cyan)),
    ]);

    Paragraph::new(line)
}
