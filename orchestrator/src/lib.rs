pub mod action;
pub mod agents;
pub mod app;
pub mod chat;
pub mod event;
pub mod mock;
pub mod observability;
pub mod policy;
pub mod pty_screen;
pub mod swarm;
pub mod tui;
pub mod ui;

use color_eyre::{eyre::WrapErr, Result};

use crate::app::App;
use crate::event::EventHandler;
use crate::tui::init_terminal;

#[derive(Debug, Clone)]
pub struct Config {
    pub swarm_demo: bool,
    pub project: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub verbose: bool,
}

impl Config {
    pub fn build(args: CliArgs) -> Result<Self> {
        Ok(Self {
            swarm_demo: args.swarm_demo,
            project: args.project,
            provider: args.provider,
            model: args.model,
            verbose: args.verbose,
        })
    }
}

#[derive(Debug, Clone, clap::Parser)]
#[command(name = "eocon-swarm")]
#[command(about = "Rust TUI multi-agent swarm orchestrator with private memory")]
#[command(version)]
pub struct CliArgs {
    #[arg(long)]
    pub swarm_demo: bool,

    #[arg(long)]
    pub project: Option<String>,

    #[arg(long)]
    pub provider: Option<String>,

    #[arg(long)]
    pub model: Option<String>,

    #[arg(long)]
    pub verbose: bool,
}

pub fn run(config: Config) -> Result<()> {
    let mut guard = init_terminal().wrap_err("failed to initialize terminal")?;

    let mut app = App::new(config.swarm_demo);
    if let Some(provider) = config.provider {
        app.current_provider = Some(provider);
    }
    if let Some(model) = config.model {
        app.current_model = Some(model);
    }

    let (_, dummy_rx) = std::sync::mpsc::channel::<crate::agents::runner::RunnerMsg>();
    let event_handler = EventHandler::new(dummy_rx, 50);

    while !app.should_quit {
        guard
            .terminal()
            .draw(|frame| ui::render_swarm(frame, &app))?;

        let action = event_handler.next();
        app.update(action);
    }

    Ok(())
}
