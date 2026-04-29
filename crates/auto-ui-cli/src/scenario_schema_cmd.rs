use anyhow::Result;
use clap::Args;

#[derive(Debug, Args)]
pub struct ScenarioSchemaArgs {
    /// Target to show schemas for (rust_chatbot or gpui_component_testing).
    /// If omitted, shows schemas for all targets.
    #[arg(long)]
    pub target: Option<String>,
}

pub fn run(args: &ScenarioSchemaArgs) -> Result<()> {
    match args.target.as_deref() {
        Some("rust_chatbot") => {
            print_rust_chatbot_schemas();
        }
        Some("gpui_component_testing") => {
            print_gpui_schemas();
        }
        Some(other) => {
            anyhow::bail!("unknown target: {other}");
        }
        None => {
            print_all_schemas();
        }
    }
    Ok(())
}

fn print_all_schemas() {
    println!("Available scenario schemas:");
    print_rust_chatbot_schemas();
    print_gpui_schemas();
}

fn print_rust_chatbot_schemas() {
    println!("\nrust_chatbot scenarios:");
    println!("  debug");
    println!("    Configuration for rust-chatbot debug session with window measurements");
    println!("  header_debug");
    println!("    Configuration for rust-chatbot header debug with screenshot and crop metrics");
    println!("  prompt_debug");
    println!("    Configuration for rust-chatbot prompt debug scenario");
}

fn print_gpui_schemas() {
    println!("\ngpui_component_testing scenarios:");
    println!("  scroll_matrix");
    println!("    Configuration for GPUI scroll matrix benchmark");
    println!("  scrollbar_trace");
    println!("    Configuration for GPUI scrollbar trace scenario");
    println!("  conversation_paint");
    println!("    Configuration for GPUI conversation paint benchmark");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_command_runs_without_panic() {
        let args = ScenarioSchemaArgs { target: None };
        run(&args).unwrap();
    }

    #[test]
    fn rust_chatbot_target_shows_schemas() {
        let args = ScenarioSchemaArgs {
            target: Some("rust_chatbot".to_string()),
        };
        run(&args).unwrap();
    }

    #[test]
    fn gpui_target_shows_schemas() {
        let args = ScenarioSchemaArgs {
            target: Some("gpui_component_testing".to_string()),
        };
        run(&args).unwrap();
    }

    #[test]
    fn unknown_target_returns_error() {
        let args = ScenarioSchemaArgs {
            target: Some("unknown".to_string()),
        };
        let result = run(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("unknown target"));
    }
}
