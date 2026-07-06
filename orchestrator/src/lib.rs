pub mod action;
pub mod agents;
pub mod app;
pub mod chat;
pub mod config;
pub mod control;
pub mod event;
pub mod mock;
pub mod model_registry;
pub mod observability;
pub mod policy;
pub mod pty_screen;
pub mod swarm;
pub mod tui;
pub mod ui;

use color_eyre::{eyre::WrapErr, Result};

use crate::app::App;
use crate::config::{AppConfig, RunnerBackend};
use crate::event::EventHandler;
use crate::tui::init_terminal;

#[derive(Debug, Clone)]
pub struct Config {
    pub swarm_demo: bool,
    pub project: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub verbose: bool,
    /// Instance name shown in `eos ps` and the dashboard.
    pub name: Option<String>,
    pub runner_backend: RunnerBackend,
}

impl Config {
    pub fn build(args: CliArgs) -> Result<Self> {
        // CLI flag wins, then on-disk config, then env, then default Pty.
        // Because the CLI argument has a default value, we treat an explicit
        // `--runner container` as authoritative; when it resolves to Pty we
        // still give config/env a chance to override the default.
        let mut runner_backend = args.runner;
        if runner_backend == RunnerBackend::Pty {
            if let Ok(app_config) = confy::load::<AppConfig>("eocon-swarm", None) {
                if app_config.runner_backend != RunnerBackend::Pty {
                    runner_backend = app_config.runner_backend;
                }
            }
            if runner_backend == RunnerBackend::Pty {
                if let Ok(val) = std::env::var("EOCON_RUNNER_BACKEND") {
                    if let Some(b) = RunnerBackend::parse(&val) {
                        runner_backend = b;
                    }
                }
            }
        }

        Ok(Self {
            swarm_demo: args.swarm_demo,
            project: args.project,
            provider: args.provider,
            model: args.model,
            verbose: args.verbose,
            name: args.name,
            runner_backend,
        })
    }
}

const HELP_EXAMPLES: &str = "\
Quick start:
  eos                                     open the interactive TUI (also controllable via CLI)
  eos serve --name my-swarm               run a headless instance (no TUI)
  eos dashboard                           watch all running instances live

Control a running instance (anything the TUI can do):
  eos ps                                  list running instances
  eos agent spawn worker --role coder     spawn an agent
  eos swarm run \"build the auth module\"   enqueue a swarm task
  eos status                              show phase / agents / gists
  eos exec \"/gist list\"                   send any raw slash command
  eos exec \"summarize the repo\"           natural input enqueues a swarm task
  eos exec --json \"/swarm status\"         machine-readable replies (for models/scripts)
  eos snapshot                            full JSON state: agents, gists, metrics, activity
  eos quit                                shut the instance down

With several instances running, target one with -i/--instance <name|pid>;
with a single instance the target is implicit. Put flags (--json, -i)
before the command words. Exit code is non-zero on error replies.

Instances register in ~/.eocon/instances/ and listen on a Unix socket in
~/.eocon/run/. Commands sent via CLI appear in the TUI and dashboard
activity log as `cli> <command>`.";

#[derive(Debug, Clone, clap::Parser)]
#[command(name = "eos")]
#[command(about = "Rust TUI multi-agent swarm orchestrator with private memory")]
#[command(version)]
#[command(after_long_help = HELP_EXAMPLES, after_help = "Run `eos --help` for usage examples.")]
pub struct CliArgs {
    /// Seed the TUI with mock agents and chatter (no model calls)
    #[arg(long)]
    pub swarm_demo: bool,

    /// Project directory (defaults to the enclosing git repo of the cwd)
    #[arg(long)]
    pub project: Option<String>,

    /// Default provider for spawned agents (e.g. anthropic)
    #[arg(long)]
    pub provider: Option<String>,

    /// Default model for spawned agents (e.g. claude-sonnet-4-6)
    #[arg(long)]
    pub model: Option<String>,

    /// Verbose output
    #[arg(long)]
    pub verbose: bool,

    /// Instance name shown in `eos ps` and the dashboard
    #[arg(long)]
    pub name: Option<String>,

    /// Execution backend for agent commands: pty (default) or container.
    #[arg(long, default_value = "pty")]
    pub runner: RunnerBackend,

    #[command(subcommand)]
    pub command: Option<EosCommand>,
}

#[derive(Debug, Clone, clap::Subcommand)]
pub enum EosCommand {
    /// Run a headless swarm instance controllable via the CLI/dashboard
    #[command(after_help = "\
The instance keeps running until it receives /quit (`eos quit`) or is killed.
It prints its name and control socket on startup.

Examples:
  eos serve --name my-swarm
  eos serve --swarm-demo          # seeded with mock agents, good for demos
  eos serve --verbose             # print the activity log to stdout")]
    Serve {
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        swarm_demo: bool,
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        provider: Option<String>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        verbose: bool,
    },
    /// List running swarm instances
    Ps {
        #[arg(long)]
        json: bool,
    },
    /// Send a raw input line (slash command or natural task) to an instance
    #[command(after_help = "\
The line is handled exactly as if typed into the TUI prompt: slash commands
dispatch to the command handlers, anything else enqueues a swarm task.
Put flags (--json, -i) before the command words.

Examples:
  eos exec \"/agent spawn worker --role coder\"
  eos exec \"/help\"                       # list all slash commands
  eos exec --json \"/swarm status\"
  eos exec -i my-swarm \"map the codebase\"")]
    Exec {
        /// Target instance name or pid (optional when only one is running)
        #[arg(short, long)]
        instance: Option<String>,
        #[arg(long)]
        json: bool,
        /// The command line, e.g. "/agent list" or a natural-language task
        #[arg(required = true, num_args = 1.., allow_hyphen_values = true)]
        command: Vec<String>,
    },
    /// Print a full JSON snapshot of an instance's state
    Snapshot {
        #[arg(short, long)]
        instance: Option<String>,
    },
    /// Live TUI dashboard over all running instances
    Dashboard,
    /// /agent commands: spawn, list, pause, resume, edit, restart, kill, unfold
    #[command(after_help = "\
Examples:
  eos agent spawn worker --role coder
  eos agent list
  eos agent pause worker
  eos agent edit worker \"focus on the parser module\"
  eos agent unfold worker            # show the agent's private memory
Roles: generalist, planner, coder, reviewer, researcher, synthesizer,
       swift-builder, swift-tester, risk-guard, quant")]
    Agent(SugarArgs),
    /// /gist commands: list, unfold, reject
    #[command(after_help = "\
Examples:
  eos gist list
  eos gist unfold <id>
  eos gist reject <id>")]
    Gist(SugarArgs),
    /// /swarm commands: run, stop, reset, status
    #[command(after_help = "\
Examples:
  eos swarm run \"build the auth module\"
  eos swarm status
  eos swarm stop
  eos swarm reset")]
    Swarm(SugarArgs),
    /// /model commands: list, variants, set, ping
    #[command(after_help = "\
Examples:
  eos model list
  eos model variants <handle>
  eos model set <handle> <variant>
  eos model ping <handle>")]
    Model(SugarArgs),
    /// Show the instance's current policy
    Policy(SugarArgs),
    /// Show swarm metrics
    Metrics(SugarArgs),
    /// Shorthand for `/swarm status`
    Status(SugarArgs),
    /// Show trace for an agent
    Trace(SugarArgs),
    /// Shut an instance down (sends /quit)
    Quit(SugarArgs),
}

/// Shared arguments for the slash-command sugar subcommands: everything after
/// the flags is joined verbatim into the equivalent TUI slash command.
#[derive(Debug, Clone, clap::Args)]
pub struct SugarArgs {
    /// Target instance name or pid (optional when only one is running)
    #[arg(short, long)]
    pub instance: Option<String>,
    #[arg(long)]
    pub json: bool,
    #[arg(num_args = 0.., allow_hyphen_values = true, trailing_var_arg = true)]
    pub args: Vec<String>,
}

fn default_instance_name(project_dir: &std::path::Path) -> String {
    let base = project_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("swarm");
    format!("{}-{}", base, std::process::id())
}

fn build_app(config: &Config) -> App {
    let mut app = App::new(config.swarm_demo, config.runner_backend);
    if let Some(provider) = &config.provider {
        app.current_provider = Some(provider.clone());
    }
    if let Some(model) = &config.model {
        if let Err(e) = app.apply_model_spec(model) {
            eprintln!("Warning: {}", e);
            app.current_model = Some(model.clone());
        }
    }
    app
}

fn start_control_plane(
    app: &App,
    name: &str,
    mode: &str,
) -> Result<(control::server::ControlServer, control::registry::InstanceGuard), String> {
    let socket = control::registry::socket_path_for(std::process::id());
    let server = control::server::ControlServer::start(&socket, name.to_string(), mode)
        .map_err(|e| format!("failed to start control server: {}", e))?;
    let guard = control::registry::register(
        name.to_string(),
        socket,
        app.project_dir.clone(),
        mode,
    )
    .map_err(|e| format!("failed to register instance: {}", e))?;
    Ok((server, guard))
}

pub fn run(config: Config) -> Result<()> {
    let mut guard = init_terminal().wrap_err("failed to initialize terminal")?;

    // Animated Eocon → Swarm startup splash (blocking, before the main event loop).
    crate::ui::startup::run_startup(|draw_callback| {
        let _ = guard.terminal().draw(|frame| {
            draw_callback(frame);
        });
    });

    let mut app = build_app(&config);
    let instance_name = config
        .name
        .clone()
        .unwrap_or_else(|| default_instance_name(&app.project_dir));

    // The control plane lets the CLI and dashboard attach to this TUI
    // instance; if it can't start, the TUI still works standalone.
    let control_plane = match start_control_plane(&app, &instance_name, "tui") {
        Ok(pair) => Some(pair),
        Err(e) => {
            app.push_interaction(crate::ui::theme::Voice::Warning, e);
            None
        }
    };

    let (_, dummy_rx) = std::sync::mpsc::channel::<crate::agents::runner::RunnerMsg>();
    let event_handler = EventHandler::new(dummy_rx, 50);

    while !app.should_quit {
        guard
            .terminal()
            .draw(|frame| ui::render_swarm(frame, &app))?;

        let action = event_handler.next();
        app.update(action);
        if let Some((server, _)) = &control_plane {
            server.drain(&mut app);
        }
    }

    Ok(())
}

/// Headless instance: same engine and command surface as the TUI, no
/// terminal UI. Controlled entirely through the CLI / control socket.
pub fn run_headless(config: Config) -> Result<()> {
    use color_eyre::eyre::eyre;

    let mut app = build_app(&config);
    let instance_name = config
        .name
        .clone()
        .unwrap_or_else(|| default_instance_name(&app.project_dir));

    let (server, guard) =
        start_control_plane(&app, &instance_name, "headless").map_err(|e| eyre!(e))?;

    println!(
        "eocon-swarm headless instance '{}' (pid {})",
        instance_name,
        std::process::id()
    );
    println!("socket: {}", guard.record.socket.display());
    println!(
        "control: eos exec --instance {} \"/swarm status\" | observe: eos dashboard",
        instance_name
    );

    let mut printed = app.interaction.len();
    while !app.should_quit {
        app.update(crate::action::Action::Tick);
        server.drain(&mut app);
        if config.verbose {
            for msg in &app.interaction[printed.min(app.interaction.len())..] {
                println!("[{}] {}", control::protocol::voice_name(msg.voice), msg.text);
            }
            printed = app.interaction.len();
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    Ok(())
}

pub fn run_dashboard() -> Result<()> {
    ui::dashboard::run_dashboard()
}
