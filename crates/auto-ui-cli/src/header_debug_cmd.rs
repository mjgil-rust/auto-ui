use anyhow::Result;
use auto_ui_adapter_rust_chatbot::{run_header_debug, HeaderDebugConfig, Provider};
use clap::Args as ClapArgs;

#[derive(Clone, Debug, ClapArgs)]
pub struct Args {
    #[arg(long)]
    pub app_root: Option<String>,
    #[arg(long, value_enum, default_value_t = Provider::Codex)]
    pub provider: Provider,
    #[arg(long)]
    pub instance: Option<u32>,
    #[arg(long)]
    pub session_id: Option<String>,
    #[arg(long)]
    pub session_name: Option<String>,
    #[arg(long)]
    pub include_hidden: bool,
    #[arg(long, default_value = "520,900,1000")]
    pub widths: String,
    #[arg(long, default_value_t = 900)]
    pub height: u32,
    #[arg(long, default_value_t = 140)]
    pub header_height: i32,
    #[arg(long, default_value_t = 15.0)]
    pub window_timeout: f64,
    #[arg(long, default_value_t = 8.0)]
    pub trace_timeout: f64,
    #[arg(long, default_value_t = 0.8)]
    pub settle: f64,
    #[arg(long)]
    pub output_dir: Option<String>,
    #[arg(long, help = "Do not lower the launched window behind other windows.")]
    pub keep_front: bool,
}

pub fn run(args: Args) -> Result<()> {
    let driver = crate::build_driver()?;
    run_header_debug(
        &*driver,
        HeaderDebugConfig {
            app_root: args.app_root,
            provider: args.provider,
            instance: args.instance,
            session_id: args.session_id,
            session_name: args.session_name,
            include_hidden: args.include_hidden,
            widths: args.widths,
            height: args.height,
            header_height: args.header_height,
            window_timeout: args.window_timeout,
            trace_timeout: args.trace_timeout,
            settle: args.settle,
            output_dir: args.output_dir,
            keep_front: args.keep_front,
        },
    )?;
    Ok(())
}
