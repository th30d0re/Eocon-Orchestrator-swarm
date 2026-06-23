use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::app::App;
use crate::model_registry::{active_variant, MODEL_HANDLES};
use crate::ui::theme;

/// Render the @-mention model picker as a floating overlay.
pub fn render(frame: &mut ratatui::Frame, app: &App) {
    if !app.mention_picker_open || app.mention_picker_filtered.is_empty() {
        return;
    }

    let area = frame.area();

    let item_count = app.mention_picker_filtered.len().min(10);
    let picker_height = (item_count as u16 + 2).min(area.height.saturating_sub(4)).max(3);
    let picker_width = 65u16.min(area.width.saturating_sub(4)).max(20);

    let x = area.x + (area.width.saturating_sub(picker_width)) / 2;
    let y = area.y + area.height.saturating_sub(picker_height + 3);
    let picker_area = Rect::new(x, y, picker_width, picker_height);

    frame.render_widget(Clear, picker_area);

    let filter_text = current_mention_prefix(app);

    let items: Vec<Line> = app
        .mention_picker_filtered
        .iter()
        .enumerate()
        .map(|(list_idx, &handle_idx)| {
            let handle = &MODEL_HANDLES[handle_idx];
            let is_selected = list_idx == app.mention_picker_selected;
            let active = active_variant(handle.handle, &app.active_variants)
                .unwrap_or(&handle.variants[handle.default_variant]);

            let handle_style = if is_selected {
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
            let active_style = if is_selected {
                Style::default().fg(Color::Yellow).bg(Color::DarkGray)
            } else {
                Style::default().fg(Color::Yellow)
            };

            let name_span = Span::styled(format!("@{}", handle.handle), handle_style);
            let pad = " ".repeat((12usize).saturating_sub(handle.handle.len() + 1));
            let desc_span = Span::styled(
                format!("{}{} ", pad, handle.display_name),
                desc_style,
            );
            let active_span = Span::styled(
                format!("({})", active.display_name),
                active_style,
            );

            if is_selected {
                Line::from(vec![
                    Span::styled(" › ", Style::default().fg(theme::ACCENT).bg(Color::DarkGray)),
                    name_span,
                    desc_span,
                    active_span,
                ])
            } else {
                Line::from(vec![
                    Span::styled("   ", Style::default()),
                    name_span,
                    desc_span,
                    active_span,
                ])
            }
        })
        .collect();

    let title = if filter_text.is_empty() {
        " Models ".to_string()
    } else {
        format!(" Models matching '@{}' ", filter_text)
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

fn current_mention_prefix(app: &App) -> String {
    let before_cursor = &app.input_buffer[..app.input_cursor];
    before_cursor
        .split_whitespace()
        .last()
        .unwrap_or("")
        .strip_prefix('@')
        .unwrap_or("")
        .to_string()
}
