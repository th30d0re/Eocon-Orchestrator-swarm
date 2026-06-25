// Animated startup splash, ported from the Foundation Orchestrator (`startup.rs`).
//
// The Eocon glitch-reveal animation and its ASCII variants are kept exactly as in Foundation;
// the second wordmark is adapted from FOUNDATION to SWARM for this app, and the tips reflect
// the swarm's commands. Driven by a blocking `run_startup(draw)` from `lib::run` before the
// main event loop, identically to Foundation.

use std::thread;
use std::time::Duration;

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::Paragraph,
    Frame,
};

// ─────────────────────────────────────────────────────────────
//  Eocon logo variants (exact, from the Foundation Orchestrator)
// ─────────────────────────────────────────────────────────────

/// Box-drawing variant — the E is drawn as 3 separate horizontal bars;
/// rows 1 and 3 (the separators between bars) are rendered dim.
const EOCON_BOX: &[&str] = &[
    r" █████╗ ██████╗  ██████╗ ██████╗ ███╗   ██╗",
    r" ╚════╝██╔═══██╗██╔════╝██╔═══██╗████╗  ██║",
    r" ███╗  ██║   ██║██║     ██║   ██║██╔██╗ ██║",
    r" ╚══╝  ██║   ██║██║     ██║   ██║██║╚██╗██║",
    r" █████╗╚██████╔╝╚██████╗╚██████╔╝██║ ╚████║",
    r" ╚════╝ ╚═════╝  ╚═════╝ ╚═════╝ ╚═╝  ╚═══╝",
];

/// Bold block variant using ▓ characters.
const EOCON_BOLD: &[&str] = &[
    r" ▓▓▓▓▓▓▓  ▓▓▓▓▓▓▓  ▓▓▓▓▓▓▓  ▓▓▓▓▓▓▓  ▓▓▓   ▓▓▓",
    r" ▓▓       ▓▓   ▓▓  ▓▓       ▓▓   ▓▓  ▓▓▓▓  ▓▓▓",
    r" ▓▓▓▓▓    ▓▓   ▓▓  ▓▓       ▓▓   ▓▓  ▓▓ ▓▓ ▓▓▓",
    r" ▓▓       ▓▓   ▓▓  ▓▓       ▓▓   ▓▓  ▓▓  ▓▓▓▓▓",
    r" ▓▓▓▓▓▓▓  ▓▓▓▓▓▓▓   ▓▓▓▓▓▓▓  ▓▓▓▓▓▓▓  ▓▓   ▓▓▓",
    r"",
];

/// Minimal thin-line outline variant.
const EOCON_MINIMAL: &[&str] = &[
    r" ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐ ┌──┐  ┌──┐",
    r" │     │ │     │ │       │     │ │  │  │  │",
    r" ├─────┘ │     │ │       │     │ │  └──┘  │",
    r" │       │     │ │       │     │ │        │",
    r" └─────┘ └─────┘  └─────┘ └─────┘ └───┐    │",
    r"                                      └──┘",
];

/// Diagonal slash variant — uses /\ to create a slanted texture.
const EOCON_SLASH: &[&str] = &[
    r"  /\\\\  //////  /\\\  //////  ||||/  |||",
    r"  //    //  //  //      //  //  ||||// |||",
    r"  ////  //  //  //      //  //  |||/ / |||",
    r"  //    //  //  //      //  //  ||| //|||",
    r"  //    //////   //////  //////  |||   /||",
    r"",
];

const EOCON_VARIANTS: &[&[&str]] = &[EOCON_BOX, EOCON_BOLD, EOCON_MINIMAL, EOCON_SLASH];

/// Characters used for glitch frames.
const GLITCH_CHARS: &[char] = &[
    '@', '#', '$', '%', '&', '*', '!', '?', '/', '\\', '<', '>', '~', '=', '+', '-',
];

// ─────────────────────────────────────────────────────────────
//  Swarm wordmark (adapted second stage)
// ─────────────────────────────────────────────────────────────

/// ANSI-shadow block wordmark spelling SWARM.
const SWARM_SHADOW: &[&str] = &[
    r" ███████╗██╗    ██╗ █████╗ ██████╗ ███╗   ███╗",
    r" ██╔════╝██║    ██║██╔══██╗██╔══██╗████╗ ████║",
    r" ███████╗██║ █╗ ██║███████║██████╔╝██╔████╔██║",
    r" ╚════██║██║███╗██║██╔══██║██╔══██╗██║╚██╔╝██║",
    r" ███████║╚███╔███╔╝██║  ██║██║  ██║██║ ╚═╝ ██║",
    r" ╚══════╝ ╚══╝╚══╝ ╚═╝  ╚═╝╚═╝  ╚═╝╚═╝     ╚═╝",
];

const SWARM_VARIANTS: &[&[&str]] = &[SWARM_SHADOW];

const SUBTITLE: &str = "O R C H E S T R A T O R";

const TIPS: &[&str] = &[
    "Press Tab to cycle pane layouts",
    "Type / for the command picker",
    "Type @ to mention a model (e.g. @kimi)",
    "Agents share only verified gists",
    "Private memory stays per-agent",
];

// ─────────────────────────────────────────────────────────────
//  Startup sequence
// ─────────────────────────────────────────────────────────────

pub fn run_startup<F>(mut draw: F)
where
    F: FnMut(&mut dyn FnMut(&mut Frame)),
{
    let tip_idx = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as usize)
        % TIPS.len();
    let tip = TIPS[tip_idx];

    // Pick one random Eocon variant and one random Swarm variant per launch.
    let eocon_idx = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as usize)
        % EOCON_VARIANTS.len();
    let eocon_variant = EOCON_VARIANTS[eocon_idx];

    let swarm_idx = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .wrapping_add(7919) as usize)
        % SWARM_VARIANTS.len();
    let swarm_variant = SWARM_VARIANTS[swarm_idx];

    // Phase 0: Eocon typewriter glitch reveal
    let eocon_revealed = typewriter_reveal(eocon_variant, &mut draw);

    let logo_height = swarm_variant.len();

    // Phase 1: Swarm scanline reveal while Eocon stays dim at top
    for revealed in 1..=logo_height {
        draw(&mut |frame: &mut Frame| {
            render_combined_frame(
                frame, &eocon_revealed, swarm_variant, revealed, 0, "", tip,
            );
        });
        thread::sleep(Duration::from_millis(40));
    }

    // Phase 2: highlight sweep through the Swarm logo
    for sweep_row in 0..logo_height {
        draw(&mut |frame: &mut Frame| {
            render_combined_frame(
                frame, &eocon_revealed, swarm_variant, logo_height, sweep_row, "", tip,
            );
        });
        thread::sleep(Duration::from_millis(30));
    }

    // Phase 3: subtitle fades in
    draw(&mut |frame: &mut Frame| {
        render_combined_frame(
            frame, &eocon_revealed, swarm_variant, logo_height, logo_height, "", tip,
        );
    });
    thread::sleep(Duration::from_millis(200));

    for _ in 0..3 {
        draw(&mut |frame: &mut Frame| {
            render_combined_frame(
                frame, &eocon_revealed, swarm_variant, logo_height, logo_height, SUBTITLE, tip,
            );
        });
        thread::sleep(Duration::from_millis(60));
    }

    // Phase 4: spinner + progress
    let spinner_frames = ["◐", "◓", "◑", "◒"];
    for i in 0..16 {
        let spinner = spinner_frames[i % spinner_frames.len()];
        let progress = (i + 1) * 100 / 16;
        let status = format!("{} Initializing... {}%", spinner, progress);
        draw(&mut |frame: &mut Frame| {
            render_combined_frame(
                frame, &eocon_revealed, swarm_variant, logo_height, logo_height, SUBTITLE, tip,
            );
            render_status_line(frame, &status);
        });
        thread::sleep(Duration::from_millis(80));
    }

    // Phase 5: brief hold on full frame
    draw(&mut |frame: &mut Frame| {
        render_combined_frame(
            frame, &eocon_revealed, swarm_variant, logo_height, logo_height, SUBTITLE, tip,
        );
        render_status_line(frame, "✓ Ready");
    });
    thread::sleep(Duration::from_millis(400));
}

// ─────────────────────────────────────────────────────────────
//  Typewriter glitch reveal
// ─────────────────────────────────────────────────────────────

/// Reveal the Eocon logo character-by-character with occasional glitch frames.
/// Returns the final revealed strings for persistent display.
fn typewriter_reveal<F>(variant: &[&str], draw: &mut F) -> Vec<String>
where
    F: FnMut(&mut dyn FnMut(&mut Frame)),
{
    let rows = variant.len();
    let cols = variant.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    let total_chars = rows * cols;

    let mut revealed: Vec<Vec<char>> = vec![vec![' '; cols]; rows];
    let target: Vec<Vec<char>> = variant
        .iter()
        .map(|line| {
            let mut chars: Vec<char> = line.chars().collect();
            chars.resize(cols, ' ');
            chars
        })
        .collect();

    let mut reveal_order: Vec<(usize, usize)> = Vec::with_capacity(total_chars);
    for r in 0..rows {
        for c in 0..cols {
            reveal_order.push((r, c));
        }
    }

    let mut rng = SimpleRng::new();

    // Shuffle reveal order for a chaotic typewriter feel
    for i in (1..reveal_order.len()).rev() {
        let j = rng.next() % (i + 1);
        reveal_order.swap(i, j);
    }

    let mut revealed_count = 0;
    let mut glitch_frames = 0usize;

    while revealed_count < reveal_order.len() || glitch_frames > 0 {
        if glitch_frames > 0 {
            glitch_frames -= 1;
            let mut glitch = revealed.clone();
            for _ in 0..8 {
                let r = rng.next() % rows;
                let c = rng.next() % cols;
                if glitch[r][c] == ' ' && target[r][c] != ' ' {
                    glitch[r][c] = GLITCH_CHARS[rng.next() % GLITCH_CHARS.len()];
                }
            }
            draw(&mut |frame: &mut Frame| {
                render_eocon_matrix(frame, &glitch, Color::Cyan, true);
            });
            thread::sleep(Duration::from_millis(30));
            continue;
        }

        let batch = 3 + (rng.next() % 4);
        for _ in 0..batch {
            if revealed_count >= reveal_order.len() {
                break;
            }
            let (r, c) = reveal_order[revealed_count];
            revealed[r][c] = target[r][c];
            revealed_count += 1;
        }

        if rng.next() % 10 == 0 && revealed_count < reveal_order.len() {
            glitch_frames = 1 + (rng.next() % 2);
        }

        let color = if revealed_count >= reveal_order.len() {
            Color::Cyan
        } else {
            Color::Yellow
        };
        draw(&mut |frame: &mut Frame| {
            render_eocon_matrix(frame, &revealed, color, true);
        });
        thread::sleep(Duration::from_millis(20));
    }

    // Brief bright hold
    for _ in 0..6 {
        draw(&mut |frame: &mut Frame| {
            render_eocon_matrix(frame, &revealed, Color::Cyan, true);
        });
        thread::sleep(Duration::from_millis(40));
    }

    // Fade to dim
    for _ in 0..4 {
        draw(&mut |frame: &mut Frame| {
            render_eocon_matrix(frame, &revealed, Color::DarkGray, false);
        });
        thread::sleep(Duration::from_millis(60));
    }

    revealed
        .into_iter()
        .map(|row| row.into_iter().collect())
        .collect()
}

// ─────────────────────────────────────────────────────────────
//  Combined frame renderer (Eocon + Swarm)
// ─────────────────────────────────────────────────────────────

fn render_combined_frame(
    frame: &mut Frame,
    eocon_revealed: &[String],
    swarm_variant: &[&str],
    revealed: usize,
    sweep_row: usize,
    subtitle: &str,
    tip: &str,
) {
    let area = frame.area();
    let eocon_height = eocon_revealed.len() as u16;
    let logo_height = swarm_variant.len() as u16;
    let subtitle_height = if subtitle.is_empty() { 0 } else { 1 };
    let gap = 1u16;
    let total_content = eocon_height + gap + logo_height + gap + subtitle_height;

    let vert_margin = area.height.saturating_sub(total_content) / 2;
    let content_area = Rect::new(
        area.x,
        area.y + vert_margin,
        area.width,
        total_content.min(area.height),
    );

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(eocon_height),
            Constraint::Length(gap),
            Constraint::Length(logo_height),
            Constraint::Length(gap),
            Constraint::Length(subtitle_height.max(1)),
        ])
        .split(content_area);

    // Render Eocon (dim)
    render_eocon_strings(frame, eocon_revealed, chunks[0]);

    // Render the Swarm logo (scanline reveal + highlight sweep)
    let logo_lines: Vec<Line> = swarm_variant
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let style = if i >= revealed {
                Style::default().fg(Color::Black)
            } else if i == sweep_row && sweep_row < swarm_variant.len() {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            Line::from(Span::styled(*line, style))
        })
        .collect();
    let logo = Paragraph::new(Text::from(logo_lines)).alignment(Alignment::Center);
    frame.render_widget(logo, chunks[2]);

    // Render subtitle
    if !subtitle.is_empty() {
        let sub_style = Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::BOLD);
        let sub = Paragraph::new(Span::styled(subtitle, sub_style)).alignment(Alignment::Center);
        frame.render_widget(sub, chunks[4]);
    }

    // Render tip at bottom of screen
    let tip_area = Rect::new(area.x, area.y + area.height.saturating_sub(2), area.width, 1);
    let tip_style = Style::default().fg(Color::Yellow);
    let tip_widget =
        Paragraph::new(Span::styled(format!("💡 {}", tip), tip_style)).alignment(Alignment::Center);
    frame.render_widget(tip_widget, tip_area);
}

// ─────────────────────────────────────────────────────────────
//  Eocon renderers
// ─────────────────────────────────────────────────────────────

fn render_eocon_matrix(frame: &mut Frame, matrix: &[Vec<char>], color: Color, bold: bool) {
    let area = frame.area();
    let logo_height = matrix.len();
    let vert_margin = area.height.saturating_sub(logo_height as u16) / 2;
    let logo_area = Rect::new(area.x, area.y + vert_margin, area.width, logo_height as u16);

    let mut style = Style::default().fg(color);
    if bold {
        style = style.add_modifier(Modifier::BOLD);
    }

    let lines: Vec<Line> = matrix
        .iter()
        .map(|row| Line::from(Span::styled(row.iter().collect::<String>(), style)))
        .collect();
    let logo = Paragraph::new(Text::from(lines)).alignment(Alignment::Center);
    frame.render_widget(logo, logo_area);
}

fn render_eocon_strings(frame: &mut Frame, lines: &[String], area: Rect) {
    let bar_style = Style::default().fg(Color::Gray);
    let sep_style = Style::default().fg(Color::DarkGray);
    let lines: Vec<Line> = lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            // For the box-drawing variant, dim rows 1 and 3 (the E separators)
            let style = if i == 1 || i == 3 { sep_style } else { bar_style };
            Line::from(Span::styled(line.as_str(), style))
        })
        .collect();
    let logo = Paragraph::new(Text::from(lines)).alignment(Alignment::Center);
    frame.render_widget(logo, area);
}

fn render_status_line(frame: &mut Frame, status: &str) {
    let area = frame.area();
    let status_area = Rect::new(area.x, area.y + area.height.saturating_sub(4), area.width, 1);
    let status_style = Style::default().fg(Color::Green);
    let widget = Paragraph::new(Span::styled(status, status_style)).alignment(Alignment::Center);
    frame.render_widget(widget, status_area);
}

// ─────────────────────────────────────────────────────────────
//  Simple deterministic RNG
// ─────────────────────────────────────────────────────────────

struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new() -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        Self {
            state: seed.wrapping_add(0x9e3779b97f4a7c15),
        }
    }

    fn next(&mut self) -> usize {
        self.state = self.state.wrapping_mul(6364136223846793005);
        self.state = self.state.wrapping_add(1442695040888963407);
        (self.state >> 32) as usize
    }
}
