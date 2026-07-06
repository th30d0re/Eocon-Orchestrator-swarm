use clap::Parser;

use eocon_swarm::control::cli;
use eocon_swarm::tui::install_panic_hook;
use eocon_swarm::{run, run_dashboard, run_headless, CliArgs, Config, EosCommand, SugarArgs};

fn main() -> std::process::ExitCode {
    if let Err(e) = color_eyre::install() {
        eprintln!("Failed to install error reporter: {}", e);
        return std::process::ExitCode::FAILURE;
    }

    let args = CliArgs::parse();

    match args.command.clone() {
        None => {
            install_panic_hook();
            let config = match Config::build(args) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("{:#}", e);
                    return std::process::ExitCode::FAILURE;
                }
            };
            report(run(config))
        }
        Some(EosCommand::Serve {
            name,
            swarm_demo,
            project,
            provider,
            model,
            verbose,
        }) => report(run_headless(Config {
            swarm_demo,
            project,
            provider,
            model,
            verbose,
            name,
            runner_backend: args.runner,
        })),
        Some(EosCommand::Dashboard) => {
            install_panic_hook();
            report(run_dashboard())
        }
        Some(EosCommand::Ps { json }) => report_cli(cli::ps(json)),
        Some(EosCommand::Exec {
            instance,
            json,
            command,
        }) => report_cli(cli::exec(instance.as_deref(), &command.join(" "), json)),
        Some(EosCommand::Snapshot { instance }) => report_cli(cli::snapshot(instance.as_deref())),
        Some(EosCommand::Agent(a)) => sugar("agent", "/agent", a),
        Some(EosCommand::Gist(a)) => sugar("gist", "/gist", a),
        Some(EosCommand::Swarm(a)) => sugar("swarm", "/swarm", a),
        Some(EosCommand::Model(a)) => sugar("model", "/model", a),
        Some(EosCommand::Policy(a)) => sugar("policy", "/policy", a),
        Some(EosCommand::Metrics(a)) => sugar("metrics", "/metrics", a),
        Some(EosCommand::Trace(a)) => sugar("trace", "/trace", a),
        Some(EosCommand::Status(a)) => sugar("status", "/swarm status", a),
        Some(EosCommand::Quit(a)) => sugar("quit", "/quit", a),
    }
}

/// Translate a sugar subcommand into the equivalent TUI slash command and
/// send it to the target instance.
fn sugar(subcommand: &str, prefix: &str, args: SugarArgs) -> std::process::ExitCode {
    // Trailing args swallow hyphen values, so a late `--help` would otherwise
    // be sent to the instance verbatim (e.g. spawning an agent named
    // "--help"). Treat it as a help request instead.
    if args.args.iter().any(|a| a == "--help" || a == "-h") {
        use clap::CommandFactory;
        let mut cmd = CliArgs::command();
        if let Some(sub) = cmd.find_subcommand_mut(subcommand) {
            let _ = sub.print_long_help();
        }
        return std::process::ExitCode::SUCCESS;
    }
    let mut command = prefix.to_string();
    if !args.args.is_empty() {
        command.push(' ');
        command.push_str(&args.args.join(" "));
    }
    report_cli(cli::exec(args.instance.as_deref(), &command, args.json))
}

fn report(result: color_eyre::Result<()>) -> std::process::ExitCode {
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{:#}", e);
            std::process::ExitCode::FAILURE
        }
    }
}

fn report_cli(result: Result<(), String>) -> std::process::ExitCode {
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            if !e.is_empty() {
                eprintln!("{}", e);
            }
            std::process::ExitCode::FAILURE
        }
    }
}
