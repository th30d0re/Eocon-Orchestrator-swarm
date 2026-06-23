use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::App;

pub fn input_bar(app: &App) -> Paragraph<'_> {
    let cursor_char = '▎';
    let before_cursor = &app.input_buffer[..app.input_cursor];
    let after_cursor = &app.input_buffer[app.input_cursor..];

    let line = Line::from(vec![
        Span::styled("> ", Style::default().fg(Color::Green)),
        Span::raw(before_cursor),
        Span::styled(cursor_char.to_string(), Style::default().fg(Color::White)),
        Span::raw(after_cursor),
    ]);

    Paragraph::new(line)
}
