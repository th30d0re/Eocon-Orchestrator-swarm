use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::action::Action;
use crate::agents::swift;
use crate::model_registry::{filter_by_prefix, active_variant, Mention, MODEL_HANDLES};
use crate::policy::config::{load_global_policy, load_user_policy};
use crate::policy::engine::PolicyEngine;
use crate::swarm::agent::{AgentLifecycleState, AgentOutput, AgentRole, ChatOutput};
use crate::swarm::engine::SwarmEngine;
use crate::ui::layout::SwarmLayout;
use crate::ui::theme::Voice;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwarmPhase {
    Idle,
    Planning,
    Running,
    Synthesizing,
    Complete,
    Failed,
}

#[derive(Debug, Clone)]
pub struct InteractionMessage {
    pub voice: Voice,
    pub text: String,
}

/// Where a submitted input line came from: the TUI prompt or the CLI
/// control socket. Remote lines are echoed with a marker so anyone watching
/// the TUI or dashboard can see what an external controller is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputSource {
    Local,
    Remote,
}

#[derive(Debug, Clone, Copy)]
pub struct SlashSubcommand {
    pub name: &'static str,
    pub description: &'static str,
    pub args: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct SlashCommand {
    pub name: &'static str,
    pub description: &'static str,
    pub takes_arg: bool,
    pub arg_hint: &'static str,
    pub subcommands: &'static [SlashSubcommand],
}

pub const SLASH_COMMANDS: &[SlashCommand] = &[
    SlashCommand {
        name: "/agent",
        description: "Manage agents",
        takes_arg: true,
        arg_hint: "<subcommand> [args]",
        subcommands: &[
            SlashSubcommand { name: "spawn", description: "Spawn a new agent", args: "<name> [--role <role>] [--provider <p>] [--model <m>]" },
            SlashSubcommand { name: "list", description: "List running agents", args: "" },
            SlashSubcommand { name: "pause", description: "Pause an agent", args: "<name>" },
            SlashSubcommand { name: "resume", description: "Resume a paused agent", args: "<name>" },
            SlashSubcommand { name: "edit", description: "Edit an agent's instruction", args: "<name> <instruction>" },
            SlashSubcommand { name: "restart", description: "Restart an agent", args: "<name>" },
            SlashSubcommand { name: "kill", description: "Kill an agent", args: "<name>" },
            SlashSubcommand { name: "unfold", description: "Show an agent's private memory", args: "<name>" },
        ],
    },
    SlashCommand {
        name: "/gist",
        description: "Manage gists",
        takes_arg: true,
        arg_hint: "<subcommand> [args]",
        subcommands: &[
            SlashSubcommand { name: "list", description: "List gists", args: "" },
            SlashSubcommand { name: "unfold", description: "Unfold a gist", args: "<id>" },
            SlashSubcommand { name: "reject", description: "Reject a gist", args: "<id>" },
        ],
    },
    SlashCommand {
        name: "/swarm",
        description: "Manage swarm",
        takes_arg: true,
        arg_hint: "<subcommand> [args]",
        subcommands: &[
            SlashSubcommand { name: "run", description: "Run a swarm task", args: "<task description>" },
            SlashSubcommand { name: "stop", description: "Stop the swarm", args: "" },
            SlashSubcommand { name: "reset", description: "Reset the swarm", args: "" },
            SlashSubcommand { name: "status", description: "Show swarm status", args: "" },
        ],
    },
    SlashCommand {
        name: "/model",
        description: "Manage model variants",
        takes_arg: true,
        arg_hint: "<subcommand> [args]",
        subcommands: &[
            SlashSubcommand { name: "list", description: "List model handles and active variants", args: "" },
            SlashSubcommand { name: "variants", description: "List variants for a handle", args: "<handle>" },
            SlashSubcommand { name: "set", description: "Set active variant for a handle", args: "<handle> <variant>" },
            SlashSubcommand { name: "ping", description: "Ping configured model", args: "<handle>" },
        ],
    },
    SlashCommand {
        name: "/policy",
        description: "Show current policy",
        takes_arg: false,
        arg_hint: "",
        subcommands: &[],
    },
    SlashCommand {
        name: "/trace",
        description: "Show trace for an agent",
        takes_arg: true,
        arg_hint: "<agent_id>",
        subcommands: &[],
    },
    SlashCommand {
        name: "/metrics",
        description: "Show swarm metrics",
        takes_arg: false,
        arg_hint: "",
        subcommands: &[],
    },
    SlashCommand {
        name: "/layout",
        description: "Toggle swarm layout",
        takes_arg: false,
        arg_hint: "",
        subcommands: &[],
    },
    SlashCommand {
        name: "/help",
        description: "Show available commands",
        takes_arg: false,
        arg_hint: "",
        subcommands: &[],
    },
    SlashCommand {
        name: "/quit",
        description: "Exit orchestrator",
        takes_arg: false,
        arg_hint: "",
        subcommands: &[],
    },
];

/// An item that can be selected in the slash-command picker.
#[derive(Debug, Clone, Copy)]
pub enum PickerItem {
    Command(usize),
    Subcommand(usize, usize),
}

enum SlashInputState {
    TopLevel { partial: String },
    Subcommand { cmd: usize, partial: String },
}

pub const FUN_STATUSES: &[&str] = &[
    "Herding agents",
    "Verifying gists",
    "Compressing private memory",
    "Waiting for a plan",
    "Polishing the UI",
    "Thinking really hard",
    "Brewing coffee",
    "Aligning the stars",
];

pub const SPINNER_FRAMES: &[&str] = &["◐", "◓", "◑", "◒"];
pub const SPINNER_COLORS: &[ratatui::style::Color] = &[
    ratatui::style::Color::Cyan,
    ratatui::style::Color::Green,
    ratatui::style::Color::Yellow,
    ratatui::style::Color::Magenta,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StartupPhase {
    #[default]
    Eocon,
    OrchestratorSwarm,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusedPane {
    #[default]
    Chat,
    Workflow,
    Models,
}

pub struct App {
    pub swarm_phase: SwarmPhase,
    pub swarm_engine: SwarmEngine,
    pub swarm_layout: SwarmLayout,
    pub startup_phase: StartupPhase,
    pub focused_pane: FocusedPane,
    pub selected_agent_index: usize,
    pub should_quit: bool,
    pub tick_count: u64,
    pub show_help: bool,
    pub mock_mode: bool,
    pub input_buffer: String,
    pub input_cursor: usize,
    pub picker_open: bool,
    pub picker_selected: usize,
    pub picker_filtered: Vec<PickerItem>,
    pub mention_picker_open: bool,
    pub mention_picker_selected: usize,
    pub mention_picker_filtered: Vec<usize>,
    pub handle_counter: HashMap<String, u32>,
    pub active_variants: HashMap<String, usize>,
    pub interaction: Vec<InteractionMessage>,
    /// When set, messages pushed via `push_interaction` are also collected
    /// here so control-socket callers receive the command's replies.
    pub capture: Option<Vec<InteractionMessage>>,
    pub streaming_interaction_indices: HashMap<String, usize>,
    pub chat_scroll: u16,
    pub workflow_scroll: u16,
    pub models_scroll: u16,
    pub chat_auto_scroll: bool,
    pub cwd: PathBuf,
    pub project_dir: PathBuf,
    pub current_provider: Option<String>,
    pub current_model: Option<String>,
    pub started: Instant,
}

impl App {
    pub fn new(mock_mode: bool) -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let project_dir = detect_project_dir(&cwd);
        let global = load_global_policy(&project_dir);
        let user = load_user_policy();
        let engine =
            SwarmEngine::new(project_dir.clone()).with_policy(PolicyEngine::new(global, user));

        let mut app = Self {
            swarm_phase: SwarmPhase::Idle,
            swarm_engine: engine,
            swarm_layout: SwarmLayout::ChatWorkflow,
            // The animated splash now runs as a blocking pre-loop sequence in `lib::run`
            // (see ui/startup.rs), so the in-loop splash starts already Done.
            startup_phase: StartupPhase::Done,
            focused_pane: FocusedPane::Chat,
            selected_agent_index: 0,
            should_quit: false,
            tick_count: 0,
            show_help: false,
            mock_mode,
            input_buffer: String::new(),
            input_cursor: 0,
            picker_open: false,
            picker_selected: 0,
            picker_filtered: Vec::new(),
            mention_picker_open: false,
            mention_picker_selected: 0,
            mention_picker_filtered: Vec::new(),
            handle_counter: HashMap::new(),
            active_variants: HashMap::new(),
            interaction: Vec::new(),
            capture: None,
            streaming_interaction_indices: HashMap::new(),
            chat_scroll: 0,
            workflow_scroll: 0,
            models_scroll: 0,
            chat_auto_scroll: true,
            cwd: cwd.clone(),
            project_dir,
            current_provider: Some("anthropic".to_string()),
            current_model: Some("claude-sonnet-4-6".to_string()),
            started: Instant::now(),
        };

        if mock_mode {
            crate::mock::seed_interaction(&mut app);
        } else {
            app.interaction.push(InteractionMessage {
                voice: Voice::Agent,
                text: "Welcome to Eocon Swarm. Type /help for commands.".to_string(),
            });
        }

        app
    }

    pub fn elapsed_secs(&self) -> u64 {
        self.started.elapsed().as_secs()
    }

    pub fn update(&mut self, action: Action) {
        match action {
            Action::Tick => {
                self.tick_count += 1;
                self.advance_startup_phase();
                if self.mock_mode {
                    crate::mock::update_mock(self);
                }
                self.swarm_engine.assign_tasks();
                self.drain_chat_responses();
                self.drain_pty_outputs();
            }
            Action::Tab => {
                if self.startup_phase != StartupPhase::Done {
                    return;
                }
                if self.mention_picker_open {
                    self.accept_mention_picker();
                } else if self.picker_open {
                    self.accept_picker();
                } else {
                    self.swarm_layout = self.swarm_layout.next();
                    self.clamp_focus_to_layout();
                }
            }
            Action::ShiftTab => {
                if self.startup_phase != StartupPhase::Done {
                    return;
                }
                self.swarm_engine.select_next();
            }
            Action::Abort | Action::Quit => {
                self.should_quit = true;
            }
            Action::ToggleHelp => {
                if self.startup_phase != StartupPhase::Done {
                    return;
                }
                self.show_help = !self.show_help;
            }
            Action::Resize(cols, rows) => {
                self.resize_screens(cols, rows);
            }
            Action::SelectNextAgent => {
                if self.startup_phase != StartupPhase::Done {
                    return;
                }
                self.swarm_engine.select_next();
            }
            Action::SelectPrevAgent => {
                if self.startup_phase != StartupPhase::Done {
                    return;
                }
                self.swarm_engine.select_prev();
            }
            Action::ToggleSwarmLayout => {
                if self.startup_phase != StartupPhase::Done {
                    return;
                }
                self.swarm_layout = self.swarm_layout.next();
                self.clamp_focus_to_layout();
            }
            Action::InputChar(c) => {
                if self.startup_phase != StartupPhase::Done {
                    return;
                }
                self.input_buffer.insert(self.input_cursor, c);
                self.input_cursor += 1;
                self.maybe_open_picker();
                self.maybe_open_mention_picker();
            }
            Action::InputBackspace => {
                if self.startup_phase != StartupPhase::Done {
                    return;
                }
                if self.input_cursor > 0 {
                    self.input_cursor -= 1;
                    self.input_buffer.remove(self.input_cursor);
                    self.maybe_open_picker();
                    self.maybe_open_mention_picker();
                }
            }
            Action::InputCursorLeft => {
                if self.startup_phase != StartupPhase::Done {
                    return;
                }
                if self.input_cursor > 0 {
                    self.input_cursor -= 1;
                }
            }
            Action::InputCursorRight => {
                if self.startup_phase != StartupPhase::Done {
                    return;
                }
                if self.input_cursor < self.input_buffer.len() {
                    self.input_cursor += 1;
                }
            }
            Action::InputCancel => {
                if self.startup_phase != StartupPhase::Done {
                    return;
                }
                self.input_buffer.clear();
                self.input_cursor = 0;
                self.picker_open = false;
                self.mention_picker_open = false;
            }
            Action::InputSubmit => {
                if self.startup_phase != StartupPhase::Done {
                    return;
                }
                if self.picker_open {
                    self.accept_picker();
                } else if self.mention_picker_open {
                    self.accept_mention_picker();
                } else {
                    let cmd = self.input_buffer.trim().to_string();
                    self.submit_line(&cmd, InputSource::Local);
                    self.input_buffer.clear();
                    self.input_cursor = 0;
                }
                self.chat_auto_scroll = true;
            }
            Action::PickerUp => {
                if self.startup_phase != StartupPhase::Done {
                    return;
                }
                if self.picker_open && self.picker_selected > 0 {
                    self.picker_selected -= 1;
                } else if self.mention_picker_open && self.mention_picker_selected > 0 {
                    self.mention_picker_selected -= 1;
                } else {
                    self.scroll_focused_pane(1);
                }
            }
            Action::PickerDown => {
                if self.startup_phase != StartupPhase::Done {
                    return;
                }
                if self.picker_open && self.picker_selected + 1 < self.picker_filtered.len() {
                    self.picker_selected += 1;
                } else if self.mention_picker_open
                    && self.mention_picker_selected + 1 < self.mention_picker_filtered.len()
                {
                    self.mention_picker_selected += 1;
                } else {
                    self.scroll_focused_pane(-1);
                }
            }
            Action::FocusLeft | Action::FocusRight => {
                if self.startup_phase != StartupPhase::Done {
                    return;
                }
                self.cycle_focus(action == Action::FocusRight);
            }
            Action::ScrollUp => {
                if self.startup_phase != StartupPhase::Done {
                    return;
                }
                self.scroll_focused_pane(5);
            }
            Action::ScrollDown => {
                if self.startup_phase != StartupPhase::Done {
                    return;
                }
                self.scroll_focused_pane(-5);
            }
            _ => {}
        }
    }

    fn resize_screens(&mut self, cols: u16, rows: u16) {
        let rows = rows as usize;
        let cols = cols as usize;
        if rows > 4 && cols > 10 {
            for agent in &mut self.swarm_engine.agents {
                match &mut agent.output {
                    AgentOutput::Pty(pty) => {
                        pty.screen.resize(rows.saturating_sub(4), cols / 2);
                    }
                    AgentOutput::SwiftResult(_) | AgentOutput::Chat(_) | AgentOutput::None => {}
                }
            }
        }
    }

    /// Update (or create) the chat-panel interaction message that mirrors a streaming agent
    /// response. The message text is prefixed with the agent's display name.
    fn update_streaming_interaction(&mut self, id: &str) {
        let (name, pending, state) = if let Some(agent) = self.swarm_engine.get_agent(id) {
            let pending = if let AgentOutput::Chat(chat) = &agent.output {
                chat.pending_response.clone()
            } else {
                String::new()
            };
            (agent.name.clone(), pending, agent.state)
        } else {
            (id.to_string(), String::new(), AgentLifecycleState::Idle)
        };

        let body = if pending.is_empty() {
            match state {
                AgentLifecycleState::Thinking => "thinking...".to_string(),
                AgentLifecycleState::Acting => "acting...".to_string(),
                AgentLifecycleState::Preparing => "preparing...".to_string(),
                _ => pending,
            }
        } else {
            pending
        };

        let text = format!("{}: {}", name, body);
        if let Some(&idx) = self.streaming_interaction_indices.get(id) {
            if let Some(msg) = self.interaction.get_mut(idx) {
                msg.text = text;
                // Don't force auto-scroll here: if the user has scrolled up to
                // read history, streaming chunks must not yank them to the bottom.
                return;
            }
        }
        self.interaction.push(InteractionMessage {
            voice: Voice::Agent,
            text,
        });
        self.streaming_interaction_indices
            .insert(id.to_string(), self.interaction.len() - 1);
    }

    fn drain_chat_responses(&mut self) {
        let ids: Vec<String> = self
            .swarm_engine
            .agents
            .iter()
            .map(|a| a.id.clone())
            .collect();
        for id in ids {
            let maybe_rx = if let Some(agent) = self.swarm_engine.get_agent_mut(&id) {
                if let AgentOutput::Chat(chat) = &mut agent.output {
                    chat.rx.take()
                } else {
                    None
                }
            } else {
                None
            };

            if let Some(rx) = maybe_rx {
                use crate::chat::ChatEvent;
                use std::sync::mpsc::TryRecvError;

                // Drain everything available this frame: accumulate streamed chunks, and
                // note whether the stream finished or errored.
                let mut finished = false;
                let mut errored: Option<String> = None;
                loop {
                    match rx.try_recv() {
                        Ok(ChatEvent::Chunk(text)) => {
                            // Accumulate into the agent's pending response and mirror the stream
                            // into the chat panel.
                            if let Some(agent) = self.swarm_engine.get_agent_mut(&id) {
                                if let AgentOutput::Chat(chat) = &mut agent.output {
                                    chat.pending_response.push_str(&text);
                                }
                            }
                            self.update_streaming_interaction(&id);
                        }
                        Ok(ChatEvent::Done) => {
                            finished = true;
                            break;
                        }
                        Ok(ChatEvent::Error(e)) => {
                            errored = Some(e);
                            break;
                        }
                        // No more events yet — still streaming.
                        Err(TryRecvError::Empty) => break,
                        // Worker ended without an explicit Done — finalize with what we have.
                        Err(TryRecvError::Disconnected) => {
                            finished = true;
                            break;
                        }
                    }
                }

                if let Some(e) = errored {
                    let name = self
                        .swarm_engine
                        .get_agent(&id)
                        .map(|a| a.name.clone())
                        .unwrap_or_else(|| id.clone());
                    if let Some(agent) = self.swarm_engine.get_agent_mut(&id) {
                        agent.last_error = Some(e.clone());
                        agent.set_state(AgentLifecycleState::Failed);
                    }
                    self.swarm_engine.metrics.record_failure();
                    self.streaming_interaction_indices.remove(&id);
                    self.push_interaction(
                        Voice::Error,
                        format!("Agent {} failed: {}", name, e),
                    );
                } else if finished {
                    self.streaming_interaction_indices.remove(&id);
                    // Move the accumulated text into memory, complete, then publish a gist.
                    let mut response_len = 0;
                    if let Some(agent) = self.swarm_engine.get_agent_mut(&id) {
                        let full = if let AgentOutput::Chat(chat) = &mut agent.output {
                            std::mem::take(&mut chat.pending_response)
                        } else {
                            String::new()
                        };
                        response_len = full.len();
                        agent.append_to_memory("assistant", full.trim_end().to_string());
                        agent.set_state(AgentLifecycleState::Completed);
                    }
                    // Provider CLIs don't report usage, so approximate at
                    // ~4 chars/token to keep the metrics panel meaningful.
                    self.swarm_engine
                        .metrics
                        .record_tokens((response_len / 4) as u64);
                    self.swarm_engine.metrics.record_success();
                    if let Some(gist) = self.swarm_engine.publish_gist(&id) {
                        self.interaction.push(InteractionMessage {
                            voice: Voice::Gist,
                            text: format!(
                                "{}: {} ({} evidence items)",
                                gist.agent_name,
                                gist.claim,
                                gist.evidence.len()
                            ),
                        });
                    }
                } else {
                    // Still streaming; put the receiver back for the next frame.
                    if let Some(agent) = self.swarm_engine.get_agent_mut(&id) {
                        if let AgentOutput::Chat(chat) = &mut agent.output {
                            chat.rx = Some(rx);
                        }
                    }
                }
            }
        }
    }

    fn drain_pty_outputs(&mut self) {
        let ids: Vec<String> = self
            .swarm_engine
            .agents
            .iter()
            .map(|a| a.id.clone())
            .collect();
        for id in ids {
            let mut maybe_handle = None;
            let mut parsed_output: Option<swift::SwiftBuildOutput> = None;
            if let Some(agent) = self.swarm_engine.get_agent_mut(&id) {
                match &mut agent.output {
                    AgentOutput::Pty(pty) => {
                        let is_swift =
                            matches!(agent.role, AgentRole::SwiftBuilder | AgentRole::SwiftTester);
                        if let Some(handle) = pty.run_handle.as_mut() {
                            // drain
                            loop {
                                match handle.rx.try_recv() {
                                    Ok(crate::agents::runner::RunnerMsg::Bytes(buf)) => {
                                        pty.screen.advance(&buf);
                                    }
                                    Ok(crate::agents::runner::RunnerMsg::Error(msg)) => {
                                        self.interaction.push(InteractionMessage {
                                            voice: Voice::Error,
                                            text: msg,
                                        });
                                    }
                                    Err(_) => break,
                                }
                            }
                        }
                        if is_swift {
                            let screen_text: String = pty
                                .screen
                                .as_text()
                                .lines
                                .iter()
                                .map(|l| l.to_string())
                                .collect::<Vec<_>>()
                                .join("\n");
                            parsed_output = swift::parse_build_agent_output(&screen_text);
                        }
                        if let Some(handle) = pty.run_handle.as_mut() {
                            if let Ok(Some(status)) = handle.child.try_wait() {
                                maybe_handle = pty.run_handle.take();
                                let code = status.exit_code() as i32;
                                if code == 0 {
                                    agent.set_state(AgentLifecycleState::Completed);
                                } else {
                                    agent.set_state(AgentLifecycleState::Failed);
                                }
                                if let Some(gist) = self.swarm_engine.publish_gist(&id) {
                                    self.interaction.push(InteractionMessage {
                                        voice: Voice::Gist,
                                        text: format!(
                                            "{}: {} (exit {})",
                                            gist.agent_name, gist.claim, code
                                        ),
                                    });
                                }
                            }
                        }
                    }
                    AgentOutput::SwiftResult(_) | AgentOutput::Chat(_) | AgentOutput::None => {}
                }
            }
            if let Some(output) = parsed_output {
                let (name, role_display, exit_code) =
                    if let Some(agent) = self.swarm_engine.get_agent_mut(&id) {
                        let name = agent.name.clone();
                        let role_display = agent.role.display_name().to_string();
                        let exit_code = output.exit_code;
                        swift::apply_swift_output(agent, output);
                        (name, role_display, exit_code)
                    } else {
                        continue;
                    };
                self.interaction.push(InteractionMessage {
                    voice: Voice::Agent,
                    text: format!(
                        "{}: Swift {} result parsed (exit {})",
                        name, role_display, exit_code
                    ),
                });
            }
            drop(maybe_handle);
        }
    }

    fn advance_startup_phase(&mut self) {
        // Each splash screen shows for ~0.5s (50ms tick rate * 10 ticks).
        self.startup_phase = match self.startup_phase {
            StartupPhase::Eocon if self.tick_count >= 10 => StartupPhase::OrchestratorSwarm,
            StartupPhase::OrchestratorSwarm if self.tick_count >= 20 => StartupPhase::Done,
            other => other,
        };
    }

    /// Panes that are visible (and therefore focusable/scrollable) in the
    /// current layout.
    fn visible_panes(&self) -> Vec<FocusedPane> {
        match self.swarm_layout {
            SwarmLayout::ChatWorkflow => vec![FocusedPane::Chat, FocusedPane::Workflow],
            SwarmLayout::ChatOnly => vec![FocusedPane::Chat],
            SwarmLayout::WorkflowOnly => vec![FocusedPane::Workflow],
            SwarmLayout::Models => vec![FocusedPane::Models],
        }
    }

    /// Ensure the focused pane is actually visible in the current layout. Called
    /// after a layout change so scroll/focus input targets a rendered pane.
    fn clamp_focus_to_layout(&mut self) {
        let panes = self.visible_panes();
        if !panes.contains(&self.focused_pane) {
            if let Some(first) = panes.first() {
                self.focused_pane = *first;
            }
        }
    }

    /// Cycle focus through the panes visible in the current layout.
    fn cycle_focus(&mut self, forward: bool) {
        let panes = self.visible_panes();
        if panes.is_empty() {
            return;
        }
        if panes.len() == 1 || !panes.contains(&self.focused_pane) {
            self.focused_pane = panes[0];
            return;
        }
        let current_pos = panes
            .iter()
            .position(|p| *p == self.focused_pane)
            .unwrap_or(0);
        let next_pos = if forward {
            (current_pos + 1) % panes.len()
        } else {
            (current_pos + panes.len() - 1) % panes.len()
        };
        self.focused_pane = panes[next_pos];
    }

    /// Scroll the currently focused pane by `delta` lines.
    /// Positive values scroll up (away from the bottom); negative values scroll down.
    fn scroll_focused_pane(&mut self, delta: i16) {
        let delta = delta as i32;
        match self.focused_pane {
            FocusedPane::Chat => {
                let new = (self.chat_scroll as i32 + delta).max(0) as u16;
                self.chat_scroll = new;
                self.chat_auto_scroll = false;
            }
            FocusedPane::Workflow => {
                let new = (self.workflow_scroll as i32 + delta).max(0) as u16;
                self.workflow_scroll = new;
            }
            FocusedPane::Models => {
                let new = (self.models_scroll as i32 + delta).max(0) as u16;
                self.models_scroll = new;
            }
        }
    }

    pub(crate) fn maybe_open_picker(&mut self) {
        if self.startup_phase != StartupPhase::Done {
            return;
        }
        if self.input_buffer.starts_with('/') && Self::slash_input_state(&self.input_buffer).is_some() {
            self.picker_open = true;
            self.refresh_picker();
        } else {
            self.picker_open = false;
        }
    }

    pub(crate) fn refresh_picker(&mut self) {
        self.picker_filtered.clear();
        if let Some(state) = Self::slash_input_state(&self.input_buffer) {
            match state {
                SlashInputState::TopLevel { partial } => {
                    self.picker_filtered = SLASH_COMMANDS
                        .iter()
                        .enumerate()
                        .filter(|(_, cmd)| {
                            cmd.name.to_lowercase().contains(&partial)
                                || cmd.description.to_lowercase().contains(&partial)
                        })
                        .map(|(i, _)| PickerItem::Command(i))
                        .collect();
                }
                SlashInputState::Subcommand { cmd, partial } => {
                    self.picker_filtered = SLASH_COMMANDS[cmd]
                        .subcommands
                        .iter()
                        .enumerate()
                        .filter(|(_, sub)| {
                            sub.name.to_lowercase().contains(&partial)
                                || sub.description.to_lowercase().contains(&partial)
                        })
                        .map(|(j, _)| PickerItem::Subcommand(cmd, j))
                        .collect();
                }
            }
        }
        self.picker_selected = self
            .picker_selected
            .min(self.picker_filtered.len().saturating_sub(1));
    }

    pub(crate) fn accept_picker(&mut self) {
        if let Some(&item) = self.picker_filtered.get(self.picker_selected) {
            match item {
                PickerItem::Command(idx) => {
                    let cmd = &SLASH_COMMANDS[idx];
                    if !cmd.subcommands.is_empty() {
                        self.input_buffer = format!("{} ", cmd.name);
                        self.input_cursor = self.input_buffer.len();
                        self.picker_selected = 0;
                        self.refresh_picker();
                        // Keep picker open so the user can pick a subcommand.
                    } else if cmd.takes_arg {
                        self.input_buffer = format!("{} ", cmd.name);
                        self.input_cursor = self.input_buffer.len();
                        self.picker_open = false;
                    } else {
                        let full_cmd = cmd.name.to_string();
                        self.input_buffer.clear();
                        self.input_cursor = 0;
                        self.picker_open = false;
                        self.submit_line(&full_cmd, InputSource::Local);
                    }
                }
                PickerItem::Subcommand(cmd_idx, sub_idx) => {
                    let cmd = &SLASH_COMMANDS[cmd_idx];
                    let sub = &cmd.subcommands[sub_idx];
                    self.input_buffer = format!("{} {} ", cmd.name, sub.name);
                    self.input_cursor = self.input_buffer.len();
                    self.picker_open = false;
                }
            }
        }
    }

    /// Returns the parser state used to decide what the slash picker should display.
    fn slash_input_state(input: &str) -> Option<SlashInputState> {
        let rest = input.strip_prefix('/')?;
        let tokens: Vec<&str> = rest.split_whitespace().collect();
        let trailing_space = rest.ends_with(' ') || rest.ends_with('\t');

        if tokens.is_empty() {
            return Some(SlashInputState::TopLevel {
                partial: "".to_string(),
            });
        }

        // A partial root (e.g. "/ag") is always a top-level filter; no need to match exactly.
        if tokens.len() == 1 && !trailing_space {
            return Some(SlashInputState::TopLevel {
                partial: tokens[0].to_string(),
            });
        }

        // From here on we need an exact root command match.
        let cmd_idx = SLASH_COMMANDS
            .iter()
            .position(|c| c.name.trim_start_matches('/') == tokens[0])?;

        if tokens.len() == 1 {
            // "/agent " — show subcommands if any.
            if SLASH_COMMANDS[cmd_idx].subcommands.is_empty() {
                return None;
            }
            return Some(SlashInputState::Subcommand {
                cmd: cmd_idx,
                partial: "".to_string(),
            });
        }

        // We have at least two tokens. If the command has no subcommands, there is nothing to
        // complete.
        if SLASH_COMMANDS[cmd_idx].subcommands.is_empty() {
            return None;
        }

        if tokens.len() == 2 && !trailing_space {
            return Some(SlashInputState::Subcommand {
                cmd: cmd_idx,
                partial: tokens[1].to_string(),
            });
        }

        // Subcommand already chosen (or more tokens typed); no picker completions.
        None
    }

    pub(crate) fn maybe_open_mention_picker(&mut self) {
        // Only open if the current word starts with '@' and we are not inside a slash command.
        if self.picker_open {
            self.mention_picker_open = false;
            return;
        }
        let before_cursor = &self.input_buffer[..self.input_cursor];
        let current_word = before_cursor.split_whitespace().last().unwrap_or("");
        if current_word.starts_with('@') && current_word.len() > 1 {
            self.mention_picker_open = true;
            self.refresh_mention_picker();
        } else {
            self.mention_picker_open = false;
        }
    }

    pub(crate) fn refresh_mention_picker(&mut self) {
        let before_cursor = &self.input_buffer[..self.input_cursor];
        let current_word = before_cursor.split_whitespace().last().unwrap_or("");
        let prefix = current_word.strip_prefix('@').unwrap_or("");
        self.mention_picker_filtered = filter_by_prefix(prefix);
        self.mention_picker_selected = self
            .mention_picker_selected
            .min(self.mention_picker_filtered.len().saturating_sub(1));
    }

    pub(crate) fn accept_mention_picker(&mut self) {
        if let Some(&idx) = self
            .mention_picker_filtered
            .get(self.mention_picker_selected)
        {
            let handle = MODEL_HANDLES[idx].handle;
            let before_cursor = &self.input_buffer[..self.input_cursor];
            let after_cursor = &self.input_buffer[self.input_cursor..];

            // Find the start of the current '@prefix' word.
            let word_start = before_cursor
                .rfind(|c: char| c.is_whitespace())
                .map(|i| i + 1)
                .unwrap_or(0);
            let new_before = format!("{}@{} ", &before_cursor[..word_start], handle);
            self.input_buffer = format!("{}{}", new_before, after_cursor);
            self.input_cursor = new_before.len();
            self.mention_picker_open = false;
        }
    }

    /// Apply a CLI/TUI model spec like `claude:opus-4.8`.
    pub fn apply_model_spec(&mut self, spec: &str) -> Result<(), String> {
        if let Some((handle, variant)) = crate::model_registry::parse_model_spec(spec) {
            self.set_active_variant(handle, variant)
        } else {
            // Treat as raw model ID for the default provider.
            self.current_model = Some(spec.to_string());
            Ok(())
        }
    }

    pub fn set_active_variant(&mut self, handle: &str, variant_name: &str) -> Result<(), String> {
        let h = crate::model_registry::find_handle(handle)
            .ok_or_else(|| format!("Unknown handle '@{}'", handle))?;
        let idx = h
            .variants
            .iter()
            .position(|v| v.name.eq_ignore_ascii_case(variant_name))
            .ok_or_else(|| {
                let names: Vec<_> = h.variants.iter().map(|v| v.name).collect();
                format!("Unknown variant '{}'. Available: {}", variant_name, names.join(", "))
            })?;
        self.active_variants.insert(h.handle.to_lowercase(), idx);
        // Also update current_provider/model to reflect the default model identity.
        let v = &h.variants[idx];
        self.current_provider = Some(v.provider.to_string());
        self.current_model = Some(v.model.to_string());
        Ok(())
    }

    /// Return a human-readable description of the model that will be used for a handle.
    pub fn ping_handle(&self, handle: &str) -> String {
        let Some(h) = crate::model_registry::find_handle(handle) else {
            return format!("Unknown handle '@{}'", handle);
        };
        let v = active_variant(handle, &self.active_variants).unwrap_or(&h.variants[h.default_variant]);

        // For local providers, try to get runtime confirmation from the sidecar or HTTP API.
        let runtime = match v.provider {
            "coreai" | "mlx" => self.spawn_local_model_info(v.provider),
            "ollama" => self.spawn_ollama_info(),
            "lmstudio" => crate::chat::lmstudio_model_info(),
            _ => None,
        };

        // Verify the provider is actually usable (CLI present, credentials,
        // daemon reachable, model on disk) instead of just echoing the mapping.
        let preflight = match crate::chat::preflight_provider(v.provider, v.model) {
            Ok(warnings) if warnings.is_empty() => "preflight ok".to_string(),
            Ok(warnings) => format!("preflight ok, warnings: {}", warnings.join("; ")),
            Err(e) => format!("preflight FAILED: {}", e),
        };

        let mut out = format!(
            "@{} → {} ({}); {}",
            h.handle, v.display_name, v.model, preflight
        );
        if let Some(name) = runtime {
            out.push_str(&format!("; runtime reports: {}", name));
        }
        out
    }

    fn spawn_local_model_info(&self, backend: &str) -> Option<String> {
        let paths = crate::chat::localmodel_lm_paths();
        let exe = paths
            .iter()
            .find(|p| std::path::Path::new(p).is_file())?;
        let output = std::process::Command::new(exe)
            .arg("info")
            .arg(backend)
            .envs(std::env::vars())
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8(output.stdout).ok()?;
        // Expect a JSON object with a "model" field.
        serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v.get("model")?.as_str().map(|s| s.to_string()))
    }

    fn spawn_ollama_info(&self) -> Option<String> {
        let output = std::process::Command::new("ollama")
            .args(["list"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8(output.stdout).ok()?;
        text.lines().next().map(|s| s.to_string())
    }

    /// Push a message to the interaction log, mirroring it into the active
    /// capture buffer (if any) so control-socket callers see command replies.
    pub fn push_interaction(&mut self, voice: Voice, text: impl Into<String>) {
        let msg = InteractionMessage {
            voice,
            text: text.into(),
        };
        if let Some(buf) = &mut self.capture {
            buf.push(msg.clone());
        }
        self.interaction.push(msg);
    }

    pub fn begin_capture(&mut self) {
        self.capture = Some(Vec::new());
    }

    pub fn take_capture(&mut self) -> Vec<InteractionMessage> {
        self.capture.take().unwrap_or_default()
    }

    /// Submit one input line exactly as the TUI prompt would: slash commands
    /// dispatch to the command handlers, anything else is natural input.
    pub fn submit_line(&mut self, line: &str, source: InputSource) {
        let cmd = line.trim();
        if cmd.is_empty() {
            return;
        }
        match source {
            InputSource::Local => self.push_interaction(Voice::User, cmd.to_string()),
            InputSource::Remote => {
                // Echo remote commands for TUI/dashboard observers only —
                // bypass the capture buffer so the CLI caller doesn't get
                // its own command back in the response.
                self.interaction.push(InteractionMessage {
                    voice: Voice::System,
                    text: format!("cli> {}", cmd),
                });
            }
        }
        if cmd.starts_with('/') {
            crate::swarm::commands::dispatch(self, cmd);
        } else {
            self.handle_natural_input(cmd);
        }
    }

    fn handle_natural_input(&mut self, input: &str) {
        let mentions = crate::model_registry::parse_mentions(input);
        if mentions.is_empty() {
            self.swarm_engine.run_swarm_task(input);
            self.swarm_phase = SwarmPhase::Running;
        } else {
            self.spawn_from_mentions(mentions);
        }
    }

    fn spawn_from_mentions(&mut self, mentions: Vec<Mention>) {
        for mention in mentions {
            let variant = active_variant(&mention.handle, &self.active_variants);
            let (provider, model, role) = match variant {
                Some(v) => (v.provider.to_string(), v.model.to_string(), v.role),
                None => {
                    self.interaction.push(InteractionMessage {
                        voice: Voice::Warning,
                        text: format!(
                            "Unknown handle '@{}'; using default provider/model.",
                            mention.handle
                        ),
                    });
                    (
                        self.current_provider.clone().unwrap_or_else(|| "anthropic".to_string()),
                        self.current_model.clone().unwrap_or_else(|| "claude-sonnet-4-6".to_string()),
                        AgentRole::Generalist,
                    )
                }
            };

            if mention.text.trim().is_empty() {
                self.interaction.push(InteractionMessage {
                    voice: Voice::Warning,
                    text: format!("@{} has no task text; skipping.", mention.handle),
                });
                continue;
            }

            let counter = self.handle_counter.entry(mention.handle.clone()).or_insert(0);
            *counter += 1;
            let name = format!("{}-{}", mention.handle, counter);

            let id = self.swarm_engine.spawn_agent(name.clone(), role, provider, model);
            if let Some(agent) = self.swarm_engine.get_agent_mut(&id) {
                let req = agent.spawn_chat(&mention.text);
                let rx = crate::chat::spawn_chat_request(req);
                agent.output = AgentOutput::Chat(ChatOutput {
                    rx: Some(rx),
                    pending_response: String::new(),
                });
            }
            self.interaction.push(InteractionMessage {
                voice: Voice::System,
                text: format!("Spawned {} ({}) with task: {}", name, mention.handle, mention.text),
            });
        }
        self.swarm_phase = SwarmPhase::Running;
    }

}

pub fn detect_project_dir(cwd: &Path) -> PathBuf {
    let mut cur = cwd;
    loop {
        if cur.join(".git").is_dir()
            || cur.join("Package.swift").is_file()
            || cur.join("Cargo.toml").is_file()
        {
            return cur.to_path_buf();
        }
        match cur.parent() {
            Some(p) => cur = p,
            None => return cwd.to_path_buf(),
        }
    }
}


#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use crate::action::Action;
    use crate::app::{App, FocusedPane, StartupPhase};
    use crate::chat::ChatEvent;
    use crate::swarm::agent::{AgentOutput, AgentRole, ChatOutput};
    use crate::ui::theme::Voice;

    #[test]
    fn chat_response_streams_to_interaction() {
        let mut app = App::new(false);
        app.startup_phase = StartupPhase::Done;

        let id = app.swarm_engine.spawn_agent(
            "kimi-1".to_string(),
            AgentRole::Generalist,
            "moonshot".to_string(),
            "".to_string(),
        );
        let (tx, rx) = mpsc::channel();
        if let Some(agent) = app.swarm_engine.get_agent_mut(&id) {
            agent.output = AgentOutput::Chat(ChatOutput {
                rx: Some(rx),
                pending_response: String::new(),
            });
        }

        tx.send(ChatEvent::Chunk("Hello".to_string())).unwrap();
        app.drain_chat_responses();

        assert!(
            app.interaction.iter().any(|m| {
                m.voice == Voice::Agent && m.text.contains("kimi-1: Hello")
            }),
            "first chunk should appear in chat panel: {:?}",
            app.interaction
        );

        tx.send(ChatEvent::Chunk(" world".to_string())).unwrap();
        tx.send(ChatEvent::Done).unwrap();
        app.drain_chat_responses();

        assert!(
            app.interaction.iter().any(|m| {
                m.voice == Voice::Agent && m.text.contains("kimi-1: Hello world")
            }),
            "accumulated response should appear in chat panel: {:?}",
            app.interaction
        );
    }

    #[test]
    fn focus_cycles_visible_panes() {
        let mut app = App::new(false);
        app.startup_phase = StartupPhase::Done;

        // ChatWorkflow starts focused on Chat.
        assert_eq!(app.focused_pane, FocusedPane::Chat);
        app.update(Action::FocusRight);
        assert_eq!(app.focused_pane, FocusedPane::Workflow);
        app.update(Action::FocusRight);
        assert_eq!(app.focused_pane, FocusedPane::Chat);
        app.update(Action::FocusLeft);
        assert_eq!(app.focused_pane, FocusedPane::Workflow);

        // Single-pane layouts keep the only valid pane.
        app.swarm_layout = crate::app::SwarmLayout::ChatOnly;
        app.focused_pane = FocusedPane::Workflow;
        app.update(Action::FocusRight);
        assert_eq!(app.focused_pane, FocusedPane::Chat);
    }

    #[test]
    fn tab_layout_change_reconciles_focus() {
        let mut app = App::new(false);
        app.startup_phase = StartupPhase::Done;

        // Focus Workflow in the split layout, then cycle layouts with Tab until
        // we land on a layout that doesn't contain Workflow.
        app.focused_pane = FocusedPane::Workflow;
        for _ in 0..4 {
            app.update(Action::Tab);
            assert!(
                app.visible_panes().contains(&app.focused_pane),
                "focus {:?} must stay valid in layout {:?}",
                app.focused_pane,
                app.swarm_layout
            );
        }
    }

    #[test]
    fn scroll_affects_focused_pane() {
        let mut app = App::new(false);
        app.startup_phase = StartupPhase::Done;

        app.focused_pane = FocusedPane::Workflow;
        app.update(Action::ScrollUp);
        assert_eq!(app.workflow_scroll, 5);
        assert_eq!(app.chat_scroll, 0);

        app.focused_pane = FocusedPane::Chat;
        app.update(Action::ScrollDown);
        // chat_scroll stays 0 because of saturating_sub, but auto-scroll should disable.
        assert!(!app.chat_auto_scroll);
    }

    #[test]
    fn arrow_keys_scroll_when_picker_closed() {
        let mut app = App::new(false);
        app.startup_phase = StartupPhase::Done;
        app.focused_pane = FocusedPane::Chat;
        app.picker_open = false;
        app.mention_picker_open = false;

        app.update(Action::PickerUp);
        assert_eq!(app.chat_scroll, 1);
        assert!(!app.chat_auto_scroll);

        app.update(Action::PickerDown);
        assert_eq!(app.chat_scroll, 0);
    }

    #[test]
    fn streaming_preserves_manual_scroll() {
        let mut app = App::new(false);
        app.startup_phase = StartupPhase::Done;
        // User has scrolled up to read history.
        app.chat_auto_scroll = false;

        let id = app.swarm_engine.spawn_agent(
            "kimi-1".to_string(),
            AgentRole::Generalist,
            "moonshot".to_string(),
            "".to_string(),
        );
        let (tx, rx) = mpsc::channel();
        if let Some(agent) = app.swarm_engine.get_agent_mut(&id) {
            agent.output = AgentOutput::Chat(ChatOutput {
                rx: Some(rx),
                pending_response: String::new(),
            });
        }

        tx.send(ChatEvent::Chunk("x".to_string())).unwrap();
        app.drain_chat_responses();

        assert!(
            !app.chat_auto_scroll,
            "streaming must not yank a user who scrolled up back to the bottom"
        );
    }
}
