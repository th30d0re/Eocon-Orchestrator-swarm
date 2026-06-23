use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Text},
    widgets::{Clear, Paragraph},
};

use crate::app::{App, StartupPhase};
use crate::ui::theme;

const ECOCON_ART: &str = r#"
 ___ ___ ___   ___ ___  _  _ 
| __/ __/ _ \ / __/ _ \| \| |
| _| (_| (_) | (_| (_) | .` |
|___\___\___/ \___\___/|_|\_|
"#;

const ORCHESTRATOR_ART: &str = r#"
  ___         _           _            _           
 / _ \ _ _ __| |_  ___ __| |_ _ _ __ _| |_ ___ _ _ 
| (_) | '_/ _| ' \/ -_|_-<  _| '_/ _` |  _/ _ \ '_|
 \___/|_| \__|_||_\___/__/\__|_| \__,_|\__\___/_|  
"#;

const SWARM_ART: &str = r#"
 ____ __ ____ _ _ _ _ __  
(_-< V  V / _` | '_| '  \ 
/__/\_/\_/\__,_|_| |_|_|_|
"#;

/// Render the startup splash screen. If the startup phase is `Done`, this is a no-op.
pub fn render(frame: &mut ratatui::Frame, app: &App) {
    match app.startup_phase {
        StartupPhase::Done => return,
        StartupPhase::Eocon => render_art(frame, ECOCON_ART, None),
        StartupPhase::OrchestratorSwarm => {
            render_art(frame, ORCHESTRATOR_ART, Some(SWARM_ART));
        }
    }
}

fn render_art(frame: &mut ratatui::Frame, main_art: &str, sub_art: Option<&str>) {
    let area = frame.area();

    // Clear the screen so the previous frame doesn't bleed through, but keep the terminal's
    // default background (no forced black background).
    frame.render_widget(Clear, area);

    let mut lines: Vec<Line> = Vec::new();
    for line in main_art.lines().skip(1) {
        lines.push(Line::from(line).style(
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        ));
    }

    if let Some(sub) = sub_art {
        lines.push(Line::from(""));
        for line in sub.lines().skip(1) {
            lines.push(Line::from(line).style(Style::default().fg(Color::Gray)));
        }
    }

    let text_height = lines.len() as u16;
    let text = Text::from(lines);
    let paragraph = Paragraph::new(text).alignment(Alignment::Center);

    // Vertically center the art.
    let top = area.height.saturating_sub(text_height) / 2;
    let bottom = area.height.saturating_sub(text_height).saturating_sub(top);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(top),
            Constraint::Length(text_height),
            Constraint::Length(bottom),
        ])
        .split(area);

    frame.render_widget(paragraph, chunks[1]);
}
