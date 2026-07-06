//! Control plane: every running instance exposes its full TUI command
//! surface over a Unix socket, registered in `~/.eocon/instances/`, so
//! CLI callers (including AI models) can drive the swarm and the dashboard
//! can observe it.

pub mod cli;
pub mod client;
pub mod protocol;
pub mod registry;
pub mod server;
