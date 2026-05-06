fn main() {
    auto_ui_cli::logging::init();
    if let Err(err) = auto_ui_cli::run() {
        let safe_error = auto_ui_cli::error_fmt::format_safe_error(&err.to_string());
        eprintln!("error: {}\n", safe_error);
        eprintln!(
            "For troubleshooting, check the progress log and report.json in the output directory."
        );
        std::process::exit(1);
    }
}
