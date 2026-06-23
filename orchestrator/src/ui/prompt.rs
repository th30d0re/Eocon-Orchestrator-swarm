use ratatui::{
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::app::App;

/// Width of the `> ` prompt prefix rendered at the start of the input line.
const PROMPT_PREFIX_LEN: usize = 2;

/// Compute the total height (including border) needed to render the current input buffer
/// inside a bordered area of the given total width.
pub fn input_height(app: &App, width: u16) -> u16 {
    if width <= 2 {
        return 3; // minimum: 1 content line + 2 border rows
    }
    let content_width = (width - 2) as usize;
    let text_width = content_width.saturating_sub(PROMPT_PREFIX_LEN);
    let lines = wrap_text(&app.input_buffer, text_width);
    let content_lines = lines.len().max(1) as u16;
    content_lines + 2
}

pub fn input_bar(app: &App) -> Paragraph<'_> {
    let cursor_char = '▎';
    let prompt = Span::styled(
        "> ",
        Style::default().fg(Color::DarkGray),
    );
    let content = if app.input_buffer.is_empty() {
        // Empty buffer shows only the prompt chevron.
        Text::from(vec![Line::from(vec![prompt])])
    } else {
        // We don't know the render width here, so we build a single-line Text with the cursor
        // inserted. The Paragraph below uses Wrap so it will reflow. The cursor marker is
        // inserted at the byte position matching input_cursor, which works for ASCII input.
        let before = &app.input_buffer[..app.input_cursor];
        let after = &app.input_buffer[app.input_cursor..];
        let line = Line::from(vec![
            prompt,
            Span::raw(before),
            Span::styled(cursor_char.to_string(), Style::default().fg(Color::White)),
            Span::raw(after),
        ]);
        Text::from(vec![line])
    };

    Paragraph::new(content)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .wrap(Wrap { trim: false })
}

/// Simple word-wrapping helper.
fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.is_empty() {
            // A single word longer than the width gets broken forcibly.
            if word.len() > width {
                let mut chars = word.chars();
                while current.len() < width {
                    if let Some(c) = chars.next() {
                        current.push(c);
                    } else {
                        break;
                    }
                }
                lines.push(current.clone());
                current.clear();
                for c in chars {
                    current.push(c);
                    if current.len() >= width {
                        lines.push(current.clone());
                        current.clear();
                    }
                }
            } else {
                current.push_str(word);
            }
        } else if current.len() + 1 + word.len() <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(current.clone());
            current.clear();
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_text_splits_long_lines() {
        let lines = wrap_text("hello world this is a test", 10);
        assert_eq!(lines, vec!["hello", "world this", "is a test"]);
    }

    #[test]
    fn input_height_includes_border() {
        let app = App::new(true);
        assert_eq!(input_height(&app, 80), 3);
    }

    #[test]
    fn input_height_accounts_for_prompt_prefix() {
        let mut app = App::new(true);
        // Width 20 -> content width 18 -> text width 16 (after "> ").
        // 40 ASCII chars with spaces will wrap to 3 text lines + 2 border rows = 5.
        app.input_buffer = "a ".repeat(20).trim_end().to_string();
        assert_eq!(input_height(&app, 20), 5);
    }
}
