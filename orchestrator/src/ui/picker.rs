use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::app::{App, SLASH_COMMANDS};
use crate::ui::theme;

/// Render the slash-command picker as a floating overlay above the input bar.
pub fn render(frame: &mut ratatui::Frame, app: &App) {
    if !app.picker_open || app.picker_filtered.is_empty() {
        return;
    }

    let area = frame.area();

    // Compute picker size: up to 10 rows, width = min(60, area.width - 4)
    let item_count = app.picker_filtered.len().min(10);
    let picker_height = (item_count as u16 + 2).min(area.height.saturating_sub(4)).max(3);
    let picker_width = 60u16.min(area.width.saturating_sub(4)).max(20);

    // Center horizontally, place above the bottom bar (status + input = 2 rows)
    let x = area.x + (area.width.saturating_sub(picker_width)) / 2;
    let y = area.y + area.height.saturating_sub(picker_height + 3);
    let picker_area = Rect::new(x, y, picker_width, picker_height);

    // Clear background behind picker
    frame.render_widget(Clear, picker_area);

    let filter_text = app.input_buffer.strip_prefix('/').unwrap_or("");

    let items: Vec<Line> = app
        .picker_filtered
        .iter()
        .enumerate()
        .map(|(list_idx, &cmd_idx)| {
            let cmd = &SLASH_COMMANDS[cmd_idx];
            let is_selected = list_idx == app.picker_selected;

            let name_style = if is_selected {
                Style::default()
                    .fg(theme::ACCENT)
                    .add_modifier(Modifier::BOLD)
                    .bg(Color::DarkGray)
            } else {
                Style::default().fg(theme::ACCENT)
            };
            let desc_style = if is_selected {
                Style::default().fg(Color::Gray).bg(Color::DarkGray)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let hint_style = if is_selected {
                Style::default().fg(Color::Yellow).bg(Color::DarkGray)
            } else {
                Style::default().fg(Color::Yellow)
            };

            let name_span = Span::styled(cmd.name, name_style);
            let pad = " ".repeat((14usize).saturating_sub(cmd.name.len()));
            let desc_span = Span::styled(format!("{}{}", pad, cmd.description), desc_style);

            let mut spans = if is_selected {
                vec![
                    Span::styled(" › ", Style::default().fg(theme::ACCENT).bg(Color::DarkGray)),
                    name_span,
                    desc_span,
                ]
            } else {
                vec![
                    Span::styled("   ", Style::default()),
                    name_span,
                    desc_span,
                ]
            };

            // Show arg hint for selected commands that take an argument.
            if is_selected && cmd.takes_arg && !cmd.arg_hint.is_empty() {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(cmd.arg_hint.to_string(), hint_style));
            }

            Line::from(spans)
        })
        .collect();

    let title = if filter_text.is_empty() {
        " Commands ".to_string()
    } else {
        format!(" Commands matching '/{}' ", filter_text)
    };

    let paragraph = Paragraph::new(items)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme::ACCENT)),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, picker_area);
}
