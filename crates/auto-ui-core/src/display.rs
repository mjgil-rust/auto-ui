use std::env;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use anyhow::{bail, Result};

pub struct HeadlessDisplay {
    xvfb: Child,
    openbox: Child,
    display: String,
}

impl HeadlessDisplay {
    pub fn start(geometry: &str) -> Result<Self> {
        let display = find_free_display()?;

        let xvfb = Command::new("Xvfb")
            .args([
                &display,
                "-screen",
                "0",
                geometry,
                "-ac",
                "-nolisten",
                "tcp",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| anyhow::anyhow!("failed to start Xvfb (apt install xvfb): {e}"))?;

        let display_num: u32 = display[1..].parse().unwrap();
        let lock_path = format!("/tmp/.X{display_num}-lock");
        for _ in 0..30 {
            if std::path::Path::new(&lock_path).exists() {
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
        if !std::path::Path::new(&lock_path).exists() {
            bail!("Xvfb failed to start on {display} (lock file never appeared)");
        }

        env::set_var("DISPLAY", &display);
        env::remove_var("XAUTHORITY");

        let openbox = Command::new("openbox")
            .arg("--sm-disable")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| {
                anyhow::anyhow!(
                    "failed to start openbox (apt install openbox): {e}. \
                     wmctrl requires an EWMH-compliant window manager on Xvfb."
                )
            })?;
        thread::sleep(Duration::from_millis(300));

        eprintln!(
            "[auto-ui] headless display {display} (xvfb pid={}, openbox pid={})",
            xvfb.id(),
            openbox.id()
        );

        Ok(Self {
            xvfb,
            openbox,
            display,
        })
    }

    pub fn display(&self) -> &str {
        &self.display
    }
}

impl Drop for HeadlessDisplay {
    fn drop(&mut self) {
        let _ = self.openbox.kill();
        let _ = self.openbox.wait();
        let _ = self.xvfb.kill();
        let _ = self.xvfb.wait();
    }
}

fn find_free_display() -> Result<String> {
    for d in (80..=99).rev() {
        let lock = format!("/tmp/.X{d}-lock");
        if !std::path::Path::new(&lock).exists() {
            return Ok(format!(":{d}"));
        }
    }
    bail!("could not find a free X display in :80..:99")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires Xvfb and openbox installed"]
    fn find_free_display_returns_colon_prefixed_number() {
        let display = find_free_display().expect("should find a free display");
        assert!(display.starts_with(':'), "display must start with ':'");
        let num: u32 = display[1..].parse().expect("display must be :N");
        assert!(
            (80..=99).contains(&num),
            "display number must be in 80..=99"
        );
    }

    #[test]
    #[ignore = "requires Xvfb and openbox installed"]
    fn headless_display_starts_and_sets_env() {
        let hd = HeadlessDisplay::start("800x600x24").expect("headless display should start");
        let display = hd.display().to_string();
        assert!(display.starts_with(':'));
        assert_eq!(env::var("DISPLAY").ok().as_deref(), Some(display.as_str()));

        let num: u32 = display[1..].parse().unwrap();
        let lock = format!("/tmp/.X{num}-lock");
        assert!(
            std::path::Path::new(&lock).exists(),
            "Xvfb lock file must exist while HeadlessDisplay is alive"
        );

        drop(hd);
        thread::sleep(Duration::from_millis(200));
    }
}
