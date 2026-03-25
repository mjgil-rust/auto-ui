mod debug_cmd;
mod header_debug_cmd;
mod run_cmd;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "auto-ui", version, about = "Generic desktop UI automation helpers")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Debug(debug_cmd::Args),
    HeaderDebug(header_debug_cmd::Args),
    Run(run_cmd::Args),
    Targets,
    Scenarios {
        #[arg(long)]
        target: Option<String>,
    },
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
        Command::Run(args) => run_cmd::run(args),
        Command::Targets => {
            println!("rust_chatbot");
            println!("gpui_component_testing");
            Ok(())
        }
        Command::Scenarios { target } => {
            run_cmd::print_scenarios(target.as_deref());
            Ok(())
        }
    }
}
