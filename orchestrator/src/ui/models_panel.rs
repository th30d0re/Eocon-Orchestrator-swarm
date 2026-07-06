use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, ScrollbarState, Wrap},
};

use crate::app::App;
use crate::model_registry::{active_variant, MODEL_HANDLES};
use crate::ui::theme;

pub fn render(app: &App, area: Rect, is_focused: bool) -> (Paragraph<'_>, ScrollbarState) {
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(vec![
        Span::styled(
            "Configured @handles",
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(ratatui::style::Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(vec![Span::raw("")]));

    for handle in MODEL_HANDLES {
        let active_idx = active_variant(handle.handle, &app.active_variants)
            .and_then(|v| handle.variants.iter().position(|x| x.name == v.name))
            .unwrap_or(handle.default_variant);

        let is_default = app
            .current_provider
            .as_ref()
            .map(|p| p.eq_ignore_ascii_case(handle.variants[active_idx].provider))
            .unwrap_or(false)
            && app
                .current_model
                .as_ref()
                .map(|m| m == handle.variants[active_idx].model)
                .unwrap_or(false);

        let handle_style = if is_default {
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(ratatui::style::Modifier::BOLD)
        } else {
            Style::default().fg(theme::ACCENT)
        };
        let desc_style = Style::default().fg(Color::White);
        let dim_style = Style::default().fg(Color::DarkGray);

        lines.push(Line::from(vec![
            Span::styled(format!("@{}", handle.handle), handle_style),
            Span::raw(" — "),
            Span::styled(handle.display_name.to_string(), desc_style),
        ]));

        for (i, v) in handle.variants.iter().enumerate() {
            let active_marker = if i == active_idx { "› " } else { "  " };
            let name_style = if i == active_idx {
                Style::default().fg(Color::Yellow)
            } else {
                dim_style
            };
            lines.push(Line::from(vec![
                Span::raw("      "),
                Span::styled(active_marker.to_string(), name_style),
                Span::styled(v.display_name.to_string(), name_style),
                Span::raw("  provider: "),
                Span::styled(v.provider.to_string(), dim_style),
                Span::raw("  model: "),
                Span::styled(v.model.to_string(), dim_style),
                Span::raw("  role: "),
                Span::styled(v.role.display_name().to_string(), dim_style),
            ]));
        }

        lines.push(Line::from(vec![Span::raw("")]));
    }

    lines.push(Line::from(vec![
        Span::styled("Tip: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "Type @handle <task> to dispatch, or /model set <handle> <variant> to switch.",
            Style::default().fg(Color::Gray),
        ),
    ]));

    let content_lines = lines.len();
    let visible_lines = area.height.saturating_sub(2) as usize;
    let max_scroll = content_lines.saturating_sub(visible_lines);
    let scroll = (app.models_scroll as usize).min(max_scroll);

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
                .title(" Models & Providers ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color)),
        )
        .wrap(Wrap { trim: false });

    let scrollbar_state = ScrollbarState::new(content_lines)
        .position(content_lines.saturating_sub(visible_lines + scroll))
        .viewport_content_length(visible_lines);

    (paragraph, scrollbar_state)
}
