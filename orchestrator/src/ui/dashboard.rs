//! Live dashboard over all running swarm instances (`eos dashboard`).
//!
//! Left pane lists instances from the registry; right pane shows the
//! selected instance's phase, agents, gists, metrics, and activity log —
//! including commands arriving over the CLI control socket, so you can
//! watch a model drive the swarm in real time.

use std::time::{Duration, Instant};

use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Row, Table, Wrap};
use ratatui::Frame;

use crate::control::client;
use crate::control::protocol::{ControlRequest, Snapshot};
use crate::control::registry::{self, InstanceRecord};
use crate::ui::theme;

const REFRESH_INTERVAL: Duration = Duration::from_millis(1000);
const POLL_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Default)]
struct DashboardState {
    instances: Vec<InstanceRecord>,
    snapshot: Option<Snapshot>,
    selected: usize,
    error: Option<String>,
}

impl DashboardState {
    fn refresh(&mut self) {
        self.instances = registry::list();
        if self.instances.is_empty() {
            self.selected = 0;
            self.snapshot = None;
            self.error = None;
            return;
        }
        self.selected = self.selected.min(self.instances.len() - 1);
        let record = &self.instances[self.selected];
        match client::send(&record.socket, &ControlRequest::Snapshot) {
            Ok(resp) => {
                self.snapshot = resp.snapshot;
                self.error = resp.error;
            }
            Err(e) => {
                self.snapshot = None;
                self.error = Some(e);
            }
        }
    }
}

pub fn run_dashboard() -> Result<()> {
    let mut guard = crate::tui::init_terminal()?;
    let mut state = DashboardState::default();
    state.refresh();
    let mut last_refresh = Instant::now();

    loop {
        guard.terminal().draw(|frame| render(frame, &state))?;

        if event::poll(POLL_INTERVAL).unwrap_or(false) {
            if let Ok(Event::Key(key)) = event::read() {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Up | KeyCode::Char('k') => {
                        if state.selected > 0 {
                            state.selected -= 1;
                            state.refresh();
                            last_refresh = Instant::now();
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if state.selected + 1 < state.instances.len() {
                            state.selected += 1;
                            state.refresh();
                            last_refresh = Instant::now();
                        }
                    }
                    KeyCode::Char('r') => {
                        state.refresh();
                        last_refresh = Instant::now();
                    }
                    _ => {}
                }
            }
        }

        if last_refresh.elapsed() >= REFRESH_INTERVAL {
            state.refresh();
            last_refresh = Instant::now();
        }
    }

    Ok(())
}

fn render(frame: &mut Frame, state: &DashboardState) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(3)])
        .split(frame.area());

    let header = Line::from(vec![
        Span::styled(
            " Eocon Swarm Dashboard ",
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(
                "— {} instance{} ",
                state.instances.len(),
                if state.instances.len() == 1 { "" } else { "s" }
            ),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            "  q quit · up/down select · r refresh",
            Style::default().fg(Color::DarkGray),
        ),
    ]);
    frame.render_widget(Paragraph::new(header), outer[0]);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(34), Constraint::Min(20)])
        .split(outer[1]);

    render_instance_list(frame, state, body[0]);
    render_detail(frame, state, body[1]);
}

fn render_instance_list(frame: &mut Frame, state: &DashboardState, rect: Rect) {
    let items: Vec<ListItem> = if state.instances.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "none running — try `eos serve`",
            Style::default().fg(Color::DarkGray),
        )))]
    } else {
        state
            .instances
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let style = if i == state.selected {
                    theme::highlight_style()
                } else {
                    Style::default()
                };
                ListItem::new(Line::from(vec![
                    Span::styled("● ", Style::default().fg(Color::Green)),
                    Span::styled(
                        format!("{} ({}) [{}]", r.name, r.pid, r.mode),
                        style,
                    ),
                ]))
            })
            .collect()
    };
    let list = List::new(items).block(
        Block::default()
            .title(" Instances ")
            .borders(Borders::ALL)
            .border_style(theme::border_style()),
    );
    frame.render_widget(list, rect);
}

fn render_detail(frame: &mut Frame, state: &DashboardState, rect: Rect) {
    let Some(snap) = &state.snapshot else {
        let msg = state
            .error
            .clone()
            .unwrap_or_else(|| "No instance selected.".to_string());
        frame.render_widget(
            Paragraph::new(msg)
                .wrap(Wrap { trim: false })
                .block(
                    Block::default()
                        .title(" Detail ")
                        .borders(Borders::ALL)
                        .border_style(theme::border_style()),
                ),
            rect,
        );
        return;
    };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Percentage(35),
            Constraint::Min(5),
        ])
        .split(rect);

    render_overview(frame, snap, rows[0]);
    render_agents(frame, snap, rows[1]);

    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(rows[2]);
    render_gists(frame, snap, bottom[0]);
    render_activity(frame, snap, bottom[1]);
}

fn render_overview(frame: &mut Frame, snap: &Snapshot, rect: Rect) {
    let lines = vec![
        Line::from(vec![
            Span::styled("Phase: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                snap.phase.clone(),
                Style::default()
                    .fg(theme::ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("   uptime {}   pid {}", fmt_secs(snap.uptime_secs), snap.pid),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(Span::raw(format!("Project: {}", snap.project_dir))),
        Line::from(Span::raw(format!(
            "Agents: {}   Gists: {}   Queued tasks: {}   Tokens: {}   CSR: {:.2}",
            snap.agents.len(),
            snap.gists.len(),
            snap.queued_tasks,
            snap.metrics.total_tokens,
            snap.metrics.csr,
        ))),
    ];
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .title(format!(" {} [{}] ", snap.name, snap.mode))
                .borders(Borders::ALL)
                .border_style(theme::border_style()),
        ),
        rect,
    );
}

fn render_agents(frame: &mut Frame, snap: &Snapshot, rect: Rect) {
    let header = Row::new(vec!["Name", "Role", "State", "Model", "Task"]).style(
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );
    let rows: Vec<Row> = snap
        .agents
        .iter()
        .map(|a| {
            let state_color = match a.state.as_str() {
                "Thinking" | "Acting" | "Preparing" => Color::Yellow,
                "Failed" => Color::Red,
                "Paused" => Color::Magenta,
                "Completed" => Color::Green,
                _ => Color::DarkGray,
            };
            // For failed agents the failure reason is more useful than the task.
            let detail = match &a.last_error {
                Some(err) if a.state == "Failed" => Span::styled(
                    format!("error: {}", err.lines().next().unwrap_or(err)),
                    Style::default().fg(Color::Red),
                ),
                _ => Span::raw(a.current_task.clone().unwrap_or_else(|| "idle".into())),
            };
            Row::new(vec![
                Span::raw(a.name.clone()),
                Span::raw(a.role.clone()),
                Span::styled(a.state.clone(), Style::default().fg(state_color)),
                Span::raw(a.model.clone()),
                detail,
            ])
        })
        .collect();
    let table = Table::new(
        rows,
        [
            Constraint::Length(14),
            Constraint::Length(12),
            Constraint::Length(10),
            Constraint::Length(22),
            Constraint::Min(10),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title(format!(" Agents ({}) ", snap.agents.len()))
            .borders(Borders::ALL)
            .border_style(theme::border_style()),
    );
    frame.render_widget(table, rect);
}

fn render_gists(frame: &mut Frame, snap: &Snapshot, rect: Rect) {
    let items: Vec<ListItem> = snap
        .gists
        .iter()
        .rev()
        .map(|g| {
            let color = match g.status.as_str() {
                "verified" => Color::Green,
                "rejected" => Color::Red,
                _ => Color::Yellow,
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("[{}] ", g.status), Style::default().fg(color)),
                Span::styled(
                    format!("{}: ", g.agent_name),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw(g.claim.clone()),
            ]))
        })
        .collect();
    let list = List::new(items).block(
        Block::default()
            .title(format!(" Gists ({}) ", snap.gists.len()))
            .borders(Borders::ALL)
            .border_style(theme::border_style()),
    );
    frame.render_widget(list, rect);
}

fn render_activity(frame: &mut Frame, snap: &Snapshot, rect: Rect) {
    let visible = rect.height.saturating_sub(2) as usize;
    let start = snap.recent.len().saturating_sub(visible);
    let items: Vec<ListItem> = snap.recent[start..]
        .iter()
        .map(|m| {
            ListItem::new(Line::from(Span::styled(
                m.text.clone(),
                Style::default().fg(voice_color(&m.voice)),
            )))
        })
        .collect();
    let list = List::new(items).block(
        Block::default()
            .title(" Activity ")
            .borders(Borders::ALL)
            .border_style(theme::border_style()),
    );
    frame.render_widget(list, rect);
}

fn voice_color(voice: &str) -> Color {
    match voice {
        "user" => Color::Green,
        "agent" => Color::Blue,
        "shell" => Color::LightGreen,
        "error" => Color::Red,
        "warning" => Color::Yellow,
        "gist" => Color::Cyan,
        "system" => Color::Magenta,
        _ => Color::White,
    }
}

fn fmt_secs(secs: u64) -> String {
    if secs >= 3600 {
        format!("{}h{:02}m", secs / 3600, (secs % 3600) / 60)
    } else if secs >= 60 {
        format!("{}m{:02}s", secs / 60, secs % 60)
    } else {
        format!("{}s", secs)
    }
}

#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use super::*;
    use crate::control::protocol::{Snapshot, SnapshotAgent, SnapshotGist, SnapshotMessage, SnapshotMetrics};

    fn sample_state() -> DashboardState {
        DashboardState {
            instances: vec![InstanceRecord {
                pid: 4242,
                name: "demo-4242".to_string(),
                socket: "/tmp/eos-4242.sock".into(),
                project_dir: "/tmp/demo".into(),
                mode: "headless".to_string(),
                started_at_epoch: 0,
            }],
            snapshot: Some(Snapshot {
                pid: 4242,
                name: "demo-4242".to_string(),
                mode: "headless".to_string(),
                phase: "Running".to_string(),
                project_dir: "/tmp/demo".to_string(),
                uptime_secs: 125,
                provider: Some("anthropic".to_string()),
                model: Some("claude-sonnet-4-6".to_string()),
                queued_tasks: 1,
                agents: vec![SnapshotAgent {
                    id: "a1".to_string(),
                    name: "worker".to_string(),
                    role: "coder".to_string(),
                    provider: "anthropic".to_string(),
                    model: "claude-sonnet-4-6".to_string(),
                    state: "Thinking".to_string(),
                    current_task: Some("write tests".to_string()),
                    risk_tier: "Medium".to_string(),
                    stall_count: 0,
                    elapsed_secs: 30,
                    memory_messages: 3,
                    last_error: None,
                }],
                gists: vec![SnapshotGist {
                    id: "g1".to_string(),
                    agent_name: "worker".to_string(),
                    status: "verified".to_string(),
                    claim: "API uses JWT".to_string(),
                    summary: "auth summary".to_string(),
                    evidence_count: 1,
                }],
                metrics: SnapshotMetrics {
                    success_rate: 1.0,
                    compliance_rate: 1.0,
                    csr: 1.0,
                    machiavellian_gap: 0.0,
                    total_tokens: 1234,
                    successes: 1,
                    failures: 0,
                },
                recent: vec![SnapshotMessage {
                    voice: "system".to_string(),
                    text: "cli> /agent list".to_string(),
                }],
            }),
            selected: 0,
            error: None,
        }
    }

    #[test]
    fn dashboard_render_smoke() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let state = sample_state();
        terminal.draw(|frame| render(frame, &state)).unwrap();
        let buf = terminal.backend().buffer();
        let content: String = (0..40)
            .flat_map(|row| (0..120).map(move |col| (col, row)))
            .map(|(col, row)| buf[(col, row)].symbol().to_string())
            .collect();
        assert!(content.contains("Eocon Swarm Dashboard"));
        assert!(content.contains("demo-4242"));
        assert!(content.contains("worker"));
        assert!(content.contains("Running"));
        assert!(content.contains("cli> /agent list"));
    }

    #[test]
    fn dashboard_render_empty() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let state = DashboardState::default();
        terminal.draw(|frame| render(frame, &state)).unwrap();
        let buf = terminal.backend().buffer();
        let content: String = (0..24)
            .flat_map(|row| (0..80).map(move |col| (col, row)))
            .map(|(col, row)| buf[(col, row)].symbol().to_string())
            .collect();
        assert!(content.contains("none running"));
    }
}
