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

#[derive(Debug, Clone)]
pub struct SlashCommand {
    pub name: &'static str,
    pub description: &'static str,
    pub takes_arg: bool,
    pub arg_hint: &'static str,
}

pub const SLASH_COMMANDS: &[SlashCommand] = &[
    SlashCommand {
        name: "/agent",
        description: "Manage agents: spawn, pause, resume, edit, restart, unfold, kill, list",
        takes_arg: true,
        arg_hint: "<subcommand> [args]",
    },
    SlashCommand {
        name: "/gist",
        description: "Manage gists: list, unfold, reject",
        takes_arg: true,
        arg_hint: "<subcommand> [args]",
    },
    SlashCommand {
        name: "/swarm",
        description: "Manage swarm: run, stop, reset, status",
        takes_arg: true,
        arg_hint: "<subcommand> [args]",
    },
    SlashCommand {
        name: "/policy",
        description: "Show current policy",
        takes_arg: false,
        arg_hint: "",
    },
    SlashCommand {
        name: "/trace",
        description: "Show trace for an agent",
        takes_arg: true,
        arg_hint: "<agent_id>",
    },
    SlashCommand {
        name: "/metrics",
        description: "Show swarm metrics",
        takes_arg: false,
        arg_hint: "",
    },
    SlashCommand {
        name: "/layout",
        description: "Toggle swarm layout",
        takes_arg: false,
        arg_hint: "",
    },
    SlashCommand {
        name: "/help",
        description: "Show available commands",
        takes_arg: false,
        arg_hint: "",
    },
    SlashCommand {
        name: "/quit",
        description: "Exit orchestrator",
        takes_arg: false,
        arg_hint: "",
    },
];

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

pub struct App {
    pub swarm_phase: SwarmPhase,
    pub swarm_engine: SwarmEngine,
    pub swarm_layout: SwarmLayout,
    pub selected_agent_index: usize,
    pub should_quit: bool,
    pub tick_count: u64,
    pub show_help: bool,
    pub mock_mode: bool,
    pub input_buffer: String,
    pub input_cursor: usize,
    pub picker_open: bool,
    pub picker_selected: usize,
    pub picker_filtered: Vec<usize>,
    pub mention_picker_open: bool,
    pub mention_picker_selected: usize,
    pub mention_picker_filtered: Vec<usize>,
    pub handle_counter: HashMap<String, u32>,
    pub active_variants: HashMap<String, usize>,
    pub interaction: Vec<InteractionMessage>,
    pub interaction_scroll: u16,
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
            interaction_scroll: 0,
            cwd: cwd.clone(),
            project_dir,
            current_provider: Some("anthropic".to_string()),
            current_model: Some("claude-sonnet-4".to_string()),
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
                if self.mock_mode {
                    crate::mock::update_mock(self);
                }
                self.swarm_engine.assign_tasks();
                self.drain_chat_responses();
                self.drain_pty_outputs();
            }
            Action::Tab => {
                if self.mention_picker_open {
                    self.accept_mention_picker();
                } else if self.picker_open {
                    self.accept_picker();
                } else {
                    self.swarm_layout = self.swarm_layout.next();
                }
            }
            Action::ShiftTab => {
                self.swarm_engine.select_next();
            }
            Action::Abort | Action::Quit => {
                self.should_quit = true;
            }
            Action::ToggleHelp => {
                self.show_help = !self.show_help;
            }
            Action::Resize(cols, rows) => {
                self.resize_screens(cols, rows);
            }
            Action::SelectNextAgent => {
                self.swarm_engine.select_next();
            }
            Action::SelectPrevAgent => {
                self.swarm_engine.select_prev();
            }
            Action::ToggleSwarmLayout => {
                self.swarm_layout = self.swarm_layout.next();
            }
            Action::InputChar(c) => {
                self.input_buffer.insert(self.input_cursor, c);
                self.input_cursor += 1;
                self.maybe_open_picker();
                self.maybe_open_mention_picker();
            }
            Action::InputBackspace => {
                if self.input_cursor > 0 {
                    self.input_cursor -= 1;
                    self.input_buffer.remove(self.input_cursor);
                    self.maybe_open_picker();
                    self.maybe_open_mention_picker();
                }
            }
            Action::InputCursorLeft => {
                if self.input_cursor > 0 {
                    self.input_cursor -= 1;
                }
            }
            Action::InputCursorRight => {
                if self.input_cursor < self.input_buffer.len() {
                    self.input_cursor += 1;
                }
            }
            Action::InputCancel => {
                self.input_buffer.clear();
                self.input_cursor = 0;
                self.picker_open = false;
                self.mention_picker_open = false;
            }
            Action::InputSubmit => {
                if self.picker_open {
                    self.accept_picker();
                } else if self.mention_picker_open {
                    self.accept_mention_picker();
                } else {
                    let cmd = self.input_buffer.trim().to_string();
                    if !cmd.is_empty() {
                        if cmd.starts_with('/') {
                            self.handle_command(&cmd);
                        } else {
                            self.handle_natural_input(&cmd);
                        }
                    }
                    self.input_buffer.clear();
                    self.input_cursor = 0;
                }
                self.interaction_scroll = 0;
            }
            Action::PickerUp => {
                if self.picker_open && self.picker_selected > 0 {
                    self.picker_selected -= 1;
                } else if self.mention_picker_open && self.mention_picker_selected > 0 {
                    self.mention_picker_selected -= 1;
                }
            }
            Action::PickerDown => {
                if self.picker_open && self.picker_selected + 1 < self.picker_filtered.len() {
                    self.picker_selected += 1;
                } else if self.mention_picker_open
                    && self.mention_picker_selected + 1 < self.mention_picker_filtered.len()
                {
                    self.mention_picker_selected += 1;
                }
            }
            Action::ScrollUp => {
                self.interaction_scroll = self.interaction_scroll.saturating_sub(5);
            }
            Action::ScrollDown => {
                self.interaction_scroll += 5;
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
                match rx.try_recv() {
                    Ok(Ok(response)) => {
                        if let Some(agent) = self.swarm_engine.get_agent_mut(&id) {
                            agent.append_to_memory("assistant", response.clone());
                            agent.set_state(AgentLifecycleState::Completed);
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
                        }
                    }
                    Ok(Err(e)) => {
                        if let Some(agent) = self.swarm_engine.get_agent_mut(&id) {
                            agent.set_state(AgentLifecycleState::Failed);
                        }
                        self.interaction.push(InteractionMessage {
                            voice: Voice::Error,
                            text: format!("Agent {} error: {}", id, e),
                        });
                    }
                    Err(_) => {
                        // Still pending; put receiver back.
                        if let Some(agent) = self.swarm_engine.get_agent_mut(&id) {
                            if let AgentOutput::Chat(chat) = &mut agent.output {
                                chat.rx = Some(rx);
                            }
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

    pub(crate) fn maybe_open_picker(&mut self) {
        if self.input_buffer.starts_with('/') && !self.input_buffer.contains(' ') {
            self.picker_open = true;
            self.refresh_picker();
        } else {
            self.picker_open = false;
        }
    }

    pub(crate) fn refresh_picker(&mut self) {
        let filter = self
            .input_buffer
            .strip_prefix('/')
            .unwrap_or("")
            .to_lowercase();
        self.picker_filtered = SLASH_COMMANDS
            .iter()
            .enumerate()
            .filter(|(_, cmd)| {
                cmd.name.to_lowercase().contains(&filter)
                    || cmd.description.to_lowercase().contains(&filter)
            })
            .map(|(i, _)| i)
            .collect();
        self.picker_selected = self
            .picker_selected
            .min(self.picker_filtered.len().saturating_sub(1));
    }

    pub(crate) fn accept_picker(&mut self) {
        if let Some(&idx) = self.picker_filtered.get(self.picker_selected) {
            let cmd = &SLASH_COMMANDS[idx];
            if cmd.takes_arg {
                self.input_buffer = format!("{} ", cmd.name);
                self.input_cursor = self.input_buffer.len();
                self.picker_open = false;
            } else {
                let full_cmd = cmd.name.to_string();
                self.input_buffer.clear();
                self.input_cursor = 0;
                self.picker_open = false;
                self.handle_command(&full_cmd);
            }
        }
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

        if let Some(name) = runtime {
            format!(
                "@{} → {} ({}); runtime reports: {}",
                h.handle, v.display_name, v.model, name
            )
        } else {
            format!("@{} → {} ({})", h.handle, v.display_name, v.model)
        }
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

    fn handle_natural_input(&mut self, input: &str) {
        self.interaction.push(InteractionMessage {
            voice: Voice::User,
            text: input.to_string(),
        });
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
                        self.current_model.clone().unwrap_or_else(|| "claude-sonnet-4.6".to_string()),
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

    fn handle_command(&mut self, cmd: &str) {
        self.interaction.push(InteractionMessage {
            voice: Voice::User,
            text: cmd.to_string(),
        });
        crate::swarm::commands::dispatch(self, cmd);
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
