use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use auto_ui_artifacts::{write_report, Report};
use auto_ui_core::{
    build_output_dir, crop_image, enhance_image, expand_path, home_dir, heuristic_text_visible,
    log_line, normalize_name, parse_widths, repo_root, request_background_launch, run_command,
    CompletedRun, TraceFields, WindowDriver, WindowGeometry,
};

use clap::ValueEnum;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

pub const TARGET_ID: &str = "rust_chatbot";
pub const SCENARIOS: &[&str] = &["debug", "header_debug", "prompt_debug"];
const DEFAULT_SESSION_NAMES: &[&str] = &[
    "pl-update",
    "pl-enhance",
    "pl-24",
    "pl-assess",
    "da-scrape-result-submission",
    "pl-graph-problem",
    "pl-enhancements-2",
];

include!("adapter/debug.rs");
include!("adapter/header_debug.rs");
include!("adapter/prompt_debug.rs");
include!("adapter/sessions.rs");
include!("adapter/runtime.rs");

#[cfg(test)]
mod newest_trace_log_tests;

#[cfg(test)]
mod tests;
