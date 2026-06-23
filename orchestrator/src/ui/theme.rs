use ratatui::style::{Color, Style};

pub const ACCENT: Color = Color::Cyan;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Voice {
    User,
    Agent,
    Shell,
    Error,
    Warning,
    Gist,
    System,
}

impl Voice {
    pub fn style(&self) -> Style {
        match self {
            Voice::User => Style::default().fg(Color::Green),
            Voice::Agent => Style::default().fg(Color::Blue),
            Voice::Shell => Style::default().fg(Color::LightGreen),
            Voice::Error => Style::default().fg(Color::Red),
            Voice::Warning => Style::default().fg(Color::Yellow),
            Voice::Gist => Style::default().fg(Color::Cyan),
            Voice::System => Style::default().fg(Color::Magenta),
        }
    }
}

pub fn border_style() -> Style {
    Style::default().fg(Color::DarkGray)
}

pub fn highlight_style() -> Style {
    Style::default().fg(Color::White).bg(Color::Blue)
}
