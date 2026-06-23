use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::app::{App, PickerItem, SLASH_COMMANDS};
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
        .map(|(list_idx, &item)| {
            let is_selected = list_idx == app.picker_selected;
            let (name, description, arg_hint) = describe_item(item);

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

            let pad = " ".repeat((14usize).saturating_sub(name.len()));
            let name_span = Span::styled(name.clone(), name_style);
            let desc_span = Span::styled(format!("{}{}", pad, description), desc_style);

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

            // Show arg hint for selected items.
            if is_selected && !arg_hint.is_empty() {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(arg_hint.to_string(), hint_style));
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

    render_preview(frame, app, picker_area);
}

fn describe_item(item: PickerItem) -> (String, &'static str, &'static str) {
    match item {
        PickerItem::Command(idx) => {
            let cmd = &SLASH_COMMANDS[idx];
            (
                cmd.name.to_string(),
                cmd.description,
                if cmd.subcommands.is_empty() {
                    cmd.arg_hint
                } else {
                    "<subcommand>"
                },
            )
        }
        PickerItem::Subcommand(cmd_idx, sub_idx) => {
            let cmd = &SLASH_COMMANDS[cmd_idx];
            let sub = &cmd.subcommands[sub_idx];
            (
                format!("{} {}", cmd.name, sub.name),
                sub.description,
                sub.args,
            )
        }
    }
}

/// Render a preview/help box above the picker showing usage for the selected item.
fn render_preview(frame: &mut ratatui::Frame, app: &App, picker_area: Rect) {
    let Some(&item) = app.picker_filtered.get(app.picker_selected) else {
        return;
    };

    let text = preview_text(item);
    let line_count = text.lines().count() as u16;
    let height = (line_count + 2)
        .min(picker_area.y.saturating_sub(2))
        .max(3);
    let width = picker_area.width;
    let x = picker_area.x;
    let y = picker_area.y.saturating_sub(height + 1);
    if y == 0 && height < 3 {
        return; // not enough room
    }
    let preview_area = Rect::new(x, y, width, height);

    frame.render_widget(Clear, preview_area);

    let paragraph = Paragraph::new(Text::from(text))
        .block(
            Block::default()
                .title(" Preview ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Gray)),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, preview_area);
}

fn preview_text(item: PickerItem) -> String {
    match item {
        PickerItem::Command(idx) => {
            let cmd = &SLASH_COMMANDS[idx];
            let mut lines = vec![
                format!("{} -- {}", cmd.name, cmd.description),
                String::new(),
            ];
            if cmd.subcommands.is_empty() {
                if cmd.takes_arg {
                    lines.push(format!("Usage: {} {}", cmd.name, cmd.arg_hint));
                } else {
                    lines.push(format!("Usage: {}", cmd.name));
                }
            } else {
                lines.push("Subcommands:".to_string());
                for sub in cmd.subcommands {
                    let args = if sub.args.is_empty() {
                        String::new()
                    } else {
                        format!(" {}", sub.args)
                    };
                    lines.push(format!("  {}{} -- {}", sub.name, args, sub.description));
                }
            }
            lines.join("\n")
        }
        PickerItem::Subcommand(cmd_idx, sub_idx) => {
            let cmd = &SLASH_COMMANDS[cmd_idx];
            let sub = &cmd.subcommands[sub_idx];
            let args = if sub.args.is_empty() {
                String::new()
            } else {
                format!(" {}", sub.args)
            };
            format!(
                "{} {} -- {}\n\nUsage: {} {}{}",
                cmd.name, sub.name, sub.description, cmd.name, sub.name, args
            )
        }
    }
}
