use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::app::App;
use crate::model_registry::{active_variant, MODEL_HANDLES};
use crate::ui::theme;

pub fn render(app: &App) -> Paragraph<'_> {
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

    Paragraph::new(lines)
        .block(
            Block::default()
                .title(" Models & Providers ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .wrap(Wrap { trim: false })
}
