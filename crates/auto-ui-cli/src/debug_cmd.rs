use anyhow::Result;
use auto_ui_adapter_rust_chatbot::{run_debug, DebugConfig, Provider};
use clap::{Args as ClapArgs, CommandFactory};

#[derive(Clone, Debug, ClapArgs)]
pub struct Args {
    #[arg(long)]
    pub app_root: Option<String>,
    #[arg(long, value_enum, default_value_t = Provider::Codex)]
    pub provider: Provider,
    #[arg(long)]
    pub instance: Option<u32>,
    #[arg(long, default_value = "520,900,1000")]
    pub widths: String,
    #[arg(long, default_value_t = 900)]
    pub height: u32,
    #[arg(long, default_value_t = 8)]
    pub max_sessions: usize,
    #[arg(long)]
    pub session_id: Option<String>,
    #[arg(long)]
    pub include_hidden: bool,
    #[arg(
        long = "launch",
        default_value_t = true,
        action = clap::ArgAction::Set,
        help = "Launch a fresh app window with RUST_CHATBOT_AUTO_UI_DEBUG=1 and use that window."
    )]
    #[arg(
        long = "no-launch",
        action = clap::ArgAction::SetFalse,
        overrides_with = "launch_if_missing",
        help = "Reuse an existing matching window instead of launching a fresh one."
    )]
    pub launch_if_missing: bool,
    #[arg(long, default_value_t = 15.0)]
    pub window_timeout: f64,
    #[arg(long, default_value_t = 5.0)]
    pub trace_timeout: f64,
    #[arg(long, default_value_t = 0.7)]
    pub settle: f64,
    #[arg(long)]
    pub output_dir: Option<String>,
    #[arg(
        long,
        help = "Do not lower launched automation windows behind other windows."
    )]
    pub keep_front: bool,
}

pub fn run(args: Args) -> Result<()> {
    run_debug(DebugConfig {
        app_root: args.app_root,
        provider: args.provider,
        instance: args.instance,
        widths: args.widths,
        height: args.height,
        max_sessions: args.max_sessions,
        session_id: args.session_id,
        include_hidden: args.include_hidden,
        launch_if_missing: args.launch_if_missing,
        window_timeout: args.window_timeout,
        trace_timeout: args.trace_timeout,
        settle: args.settle,
        output_dir: args.output_dir,
        keep_front: args.keep_front,
    })?;
    Ok(())
}
