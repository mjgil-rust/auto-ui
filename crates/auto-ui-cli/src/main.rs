mod common;
mod debug_cmd;
mod header_debug_cmd;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "auto-ui", version, about = "Rust Chatbot desktop automation helpers")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Debug(debug_cmd::Args),
    HeaderDebug(header_debug_cmd::Args),
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err:#}");
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Debug(args) => debug_cmd::run(args),
        Command::HeaderDebug(args) => header_debug_cmd::run(args),
    }
}
