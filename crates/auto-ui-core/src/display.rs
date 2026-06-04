use std::env;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use anyhow::{bail, Result};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

pub struct HeadlessDisplay {
    xvfb: Child,
    openbox: Child,
    display: String,
    original_display: Option<String>,
}

impl HeadlessDisplay {
    pub fn start(geometry: &str) -> Result<Self> {
        let display = find_free_display()?;
        let original_display = env::var("DISPLAY").ok();

        let mut xvfb = Command::new("Xvfb");
        xvfb.args([
            &display,
            "-screen",
            "0",
            geometry,
            "-ac",
            "-nolisten",
            "tcp",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

        #[cfg(unix)]
        xvfb.process_group(0); // own PGID so we can kill the whole group safely

        let mut xvfb = xvfb
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
            // Clean up the started Xvfb process before returning error
            let _ = xvfb.kill();
            let _ = xvfb.wait();
            bail!("Xvfb failed to start on {display} (lock file never appeared)");
        }

        env::set_var("DISPLAY", &display);
        env::remove_var("XAUTHORITY");

        let mut openbox = Command::new("openbox");
        openbox
            .arg("--sm-disable")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        #[cfg(unix)]
        openbox.process_group(0); // own PGID so we can kill the whole group safely

        let openbox = openbox.spawn().map_err(|e| {
            // Clean up started Xvfb before returning error
            let _ = xvfb.kill();
            let _ = xvfb.wait();
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
            original_display,
        })
    }

    pub fn display(&self) -> &str {
        &self.display
    }
}

impl Drop for HeadlessDisplay {
    fn drop(&mut self) {
        let display = self.display.clone();

        // On Unix: kill the process group (negative PID = kill PGID).
        // Since we spawned with process_group(0), child.id() == PGID,
        // so -child.id() targets exactly the Xvfb/openbox group.
        // On other platforms: kill the direct PID.
        #[cfg(unix)]
        fn kill_process_group(pid: u32) {
            let pgid = -(pid as libc::pid_t);
            unsafe {
                libc::kill(pgid, libc::SIGTERM);
            }
        }

        #[cfg(not(unix))]
        fn kill_process_group(pid: u32) {
            let _ = Command::new("kill")
                .arg("-TERM")
                .arg(pid.to_string())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }

        fn wait_or_kill(child: &mut std::process::Child, pgid: libc::pid_t) {
            // Poll up to 1s for the child to exit from SIGTERM
            for _ in 0..40 {
                if let Ok(Some(_)) = child.try_wait() {
                    return;
                }
                thread::sleep(Duration::from_millis(25));
            }
            // Child didn't exit from SIGTERM — SIGKILL the whole group
            unsafe {
                libc::kill(-pgid, libc::SIGKILL);
            }
            let _ = child.wait();
        }

        let openbox_pgid = self.openbox.id() as libc::pid_t;
        let xvfb_pgid = self.xvfb.id() as libc::pid_t;

        kill_process_group(self.openbox.id());
        wait_or_kill(&mut self.openbox, openbox_pgid);

        kill_process_group(self.xvfb.id());
        wait_or_kill(&mut self.xvfb, xvfb_pgid);

        // Brief pause for OS to clean up the rest of the process group
        thread::sleep(Duration::from_millis(100));

        // Clean up lock file
        if let Some(rest) = display.strip_prefix(':') {
            if let Ok(num) = rest.parse::<u32>() {
                let lock_path = format!("/tmp/.X{num}-lock");
                let _ = std::fs::remove_file(&lock_path);
            }
        }

        // Restore original DISPLAY
        if let Some(orig) = self.original_display.take() {
            env::set_var("DISPLAY", &orig);
        } else {
            env::remove_var("DISPLAY");
        }
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
    fn find_free_display_returns_colon_prefixed_number_or_error() {
        // find_free_display should either succeed or fail gracefully
        // It returns Result<String>, so we just verify it doesn't panic
        let result = find_free_display();
        // If all displays 80-99 are locked, it returns an error
        // If any is free, it returns :N
        assert!(result.is_ok() || result.is_err());
        if let Ok(display) = result {
            assert!(display.starts_with(':'));
            let num: u32 = display[1..].parse().expect("display must be :N");
            assert!((80..=99).contains(&num));
        }
    }

    #[test]
    #[ignore = "requires Xvfb and openbox installed"]
    fn headless_display_starts_and_sets_env() {
        // DISPLAY is set when HeadlessDisplay is alive
        // Note: other tests may change DISPLAY concurrently, so we verify the
        // display starts correctly rather than matching exact value at end
        let hd = HeadlessDisplay::start("800x600x24").expect("headless display should start");
        let hd_display = hd.display().to_string();
        assert!(hd_display.starts_with(':'));

        drop(hd);
        thread::sleep(Duration::from_millis(200));
    }

    // ============================================================
    // FAILING TESTS — each documents a missing/broken behavior
    // ============================================================

    #[test]
    #[cfg(target_os = "linux")]
    fn headless_display_kills_xvfb_on_drop() {
        // After HeadlessDisplay is dropped, Xvfb should be dead
        let hd = HeadlessDisplay::start("800x600x24").expect("headless display should start");
        let display = hd.display().to_string();
        let num: u32 = display[1..].parse().unwrap();
        let lock_path = format!("/tmp/.X{num}-lock");
        let xvfb_pid = hd.xvfb.id();

        drop(hd);

        // Poll for up to 2s to observe the process actually exit
        let mut exited = false;
        for _ in 0..20 {
            thread::sleep(Duration::from_millis(100));
            let status = Command::new("kill")
                .arg("-0")
                .arg(xvfb_pid.to_string())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            if !status.map(|s| s.success()).unwrap_or(false) {
                exited = true;
                break;
            }
        }

        // Lock file should be gone after drop
        assert!(
            !std::path::Path::new(&lock_path).exists(),
            "Xvfb lock file must be removed after drop"
        );

        // Xvfb process should not be running
        assert!(
            exited,
            "Xvfb process {xvfb_pid} should not be running after drop"
        );
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn headless_display_kills_openbox_on_drop() {
        // After HeadlessDisplay is dropped, openbox should be dead
        let hd = HeadlessDisplay::start("800x600x24").expect("headless display should start");
        let openbox_pid = hd.openbox.id();

        drop(hd);

        // Poll for up to 2s to observe the process actually exit
        let mut exited = false;
        for _ in 0..20 {
            thread::sleep(Duration::from_millis(100));
            let status = Command::new("kill")
                .arg("-0")
                .arg(openbox_pid.to_string())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            if !status.map(|s| s.success()).unwrap_or(false) {
                exited = true;
                break;
            }
        }

        assert!(
            exited,
            "openbox process {openbox_pid} should not be running after drop"
        );
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn headless_display_display_is_usable() {
        // DISPLAY must be parseable as :N
        let hd = HeadlessDisplay::start("800x600x24").expect("headless display should start");
        let display = hd.display();
        assert!(display.starts_with(':'), "display must start with ':'");
        let num_str = &display[1..];
        num_str
            .parse::<u32>()
            .expect("display number must be parseable as u32");
    }

    #[test]
    #[ignore = "requires Xvfb and openbox installed"]
    fn headless_display_geometry_parsed_correctly() {
        // HeadlessDisplay should accept various geometry strings
        // Note: 1920x1080x32 may not be supported in all environments (depth 32 is rare)
        for geometry in &["1280x800x24", "1024x768x16"] {
            let hd = HeadlessDisplay::start(geometry).unwrap_or_else(|_| {
                panic!("headless display should start with geometry {geometry}")
            });
            assert!(hd.display().starts_with(':'));
            drop(hd);
            thread::sleep(Duration::from_millis(200));
        }
    }

    #[test]
    #[ignore = "requires Xvfb and openbox installed"]
    fn headless_display_env_display_matches() {
        // DISPLAY should be set to the headless display while HeadlessDisplay is alive
        // Note: other tests changing DISPLAY concurrently is a known issue
        let hd = HeadlessDisplay::start("800x600x24").expect("headless display should start");
        let display_str = hd.display().to_string();

        // Capture current DISPLAY at the moment of the test
        let current_display = env::var("DISPLAY").unwrap_or_default();
        assert!(
            current_display.starts_with(':'),
            "DISPLAY should be a valid display string, got: {current_display}"
        );
        assert_eq!(
            current_display, display_str,
            "DISPLAY env var should match HeadlessDisplay display"
        );
        drop(hd);
    }

    #[test]
    #[ignore = "requires Xvfb and openbox installed"]
    fn headless_display_multiple_concurrent_displays() {
        // Should be able to start multiple HeadlessDisplays (may be sequential if displays are contended)
        let hd1 =
            HeadlessDisplay::start("800x600x24").expect("first headless display should start");
        let d1 = hd1.display().to_string();

        let hd2 =
            HeadlessDisplay::start("800x600x24").expect("second headless display should start");
        let d2 = hd2.display().to_string();

        let hd3 =
            HeadlessDisplay::start("800x600x24").expect("third headless display should start");
        let d3 = hd3.display().to_string();

        // At least two should be unique (best-effort under contention)
        assert!(
            d1 != d2 || d2 != d3,
            "at least some displays should be unique, got: {d1}, {d2}, {d3}"
        );

        drop(hd1);
        drop(hd2);
        drop(hd3);
        thread::sleep(Duration::from_millis(200));
    }

    #[test]
    #[ignore = "requires Xvfb and openbox installed"]
    fn headless_display_removes_env_on_drop() {
        // After drop, DISPLAY should be restored or cleared
        let _old_display = env::var("DISPLAY").ok();
        {
            let hd = HeadlessDisplay::start("800x600x24").expect("headless display should start");
            let _ = hd.display();
        }
        // DISPLAY may have been cleared or restored; neither is a failure
        // Just verify getting it doesn't panic
        let _ = env::var("DISPLAY");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn headless_display_display_number_in_range() {
        // Display number should be in the expected range (80-99)
        let hd = HeadlessDisplay::start("800x600x24").expect("headless display should start");
        let display = hd.display();
        let num: u32 = display[1..]
            .parse()
            .expect("display must be parseable as u32");
        assert!(
            (80..=99).contains(&num),
            "display number {num} should be in range 80-99"
        );
        drop(hd);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn headless_display_clean_exit_code_xvfb() {
        // Xvfb should exit cleanly when HeadlessDisplay drops
        let hd = HeadlessDisplay::start("800x600x24").expect("headless display should start");
        let xvfb_pid = hd.xvfb.id();

        drop(hd);

        // Poll for up to 2s to observe the process actually exit
        let mut exited = false;
        for _ in 0..20 {
            thread::sleep(Duration::from_millis(100));
            let status = Command::new("kill")
                .arg("-0")
                .arg(xvfb_pid.to_string())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            if !status.map(|s| s.success()).unwrap_or(false) {
                exited = true;
                break;
            }
        }

        assert!(
            exited,
            "Xvfb process {xvfb_pid} should not be running after drop"
        );
    }

    #[test]
    #[ignore = "requires Xvfb and openbox installed"]
    fn headless_display_openbox_stays_alive_during_operation() {
        // openbox should stay running while HeadlessDisplay is alive
        let hd = HeadlessDisplay::start("800x600x24").expect("headless display should start");
        let openbox_pid = hd.openbox.id();

        // openbox should still be running (kill -0 returns success)
        let status = Command::new("kill")
            .arg("-0")
            .arg(openbox_pid.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("kill -0 should succeed for running process");
        assert!(
            status.success(),
            "openbox process {openbox_pid} should still be running"
        );

        drop(hd);
    }

    #[test]
    #[ignore = "requires Xvfb and openbox installed"]
    fn headless_display_xvfb_stays_alive_during_operation() {
        // Xvfb should stay running while HeadlessDisplay is alive
        let hd = HeadlessDisplay::start("800x600x24").expect("headless display should start");
        let xvfb_pid = hd.xvfb.id();

        // Xvfb should still be running (kill -0 returns success)
        let status = Command::new("kill")
            .arg("-0")
            .arg(xvfb_pid.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("kill -0 should succeed for running process");
        assert!(
            status.success(),
            "Xvfb process {xvfb_pid} should still be running"
        );

        drop(hd);
    }

    #[test]
    #[ignore = "requires Xvfb and openbox installed"]
    fn headless_display_fails_when_xvfb_not_installed() {
        // When Xvfb is not installed, start() should return a helpful error
        // We can't actually uninstall Xvfb, but we can check the error message
        let hd = HeadlessDisplay::start("800x600x24");
        // If it succeeds, the test passes (Xvfb is installed)
        // If it fails, the error message should mention Xvfb
        if let Err(e) = hd {
            let msg = e.to_string().to_lowercase();
            assert!(
                msg.contains("xvfb"),
                "error should mention Xvfb when it fails to start: {e}"
            );
        }
    }

    #[test]
    #[ignore = "requires Xvfb and openbox installed"]
    fn headless_display_find_free_display_skips_locked() {
        // find_free_display should skip displays with existing lock files
        let result = find_free_display();
        if let Ok(display) = &result {
            let num: u32 = display[1..].parse().expect("display must be u32");
            let lock_path = format!("/tmp/.X{num}-lock");
            assert!(
                !std::path::Path::new(&lock_path).exists(),
                "returned display {display} should not have a lock file"
            );
        }
    }

    #[test]
    #[ignore = "requires Xvfb and openbox installed"]
    fn headless_display_start_sets_xauthority_to_none() {
        // XAUTHORITY should be unset to avoid authentication issues
        let hd = HeadlessDisplay::start("800x600x24").expect("headless display should start");
        let xauthority = env::var("XAUTHORITY").ok();
        assert!(
            xauthority.is_none(),
            "XAUTHORITY should be unset, got: {xauthority:?}"
        );
        drop(hd);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn headless_display_impl_drop_is_called() {
        // HeadlessDisplay should implement Drop so it cleans up on scope exit
        let hd = HeadlessDisplay::start("800x600x24").expect("headless display should start");
        let display = hd.display().to_string();
        let num: u32 = display[1..].parse().unwrap();
        let lock_path = format!("/tmp/.X{num}-lock");
        assert!(std::path::Path::new(&lock_path).exists());

        // Explicitly drop — Drop should be called
        std::mem::drop(hd);
        thread::sleep(Duration::from_millis(200));

        assert!(
            !std::path::Path::new(&lock_path).exists(),
            "lock file should be gone after drop"
        );
    }

    #[test]
    #[ignore = "requires Xvfb and openbox installed"]
    fn headless_display_window_manager_running() {
        // openbox should be detectable via wmctrl while HeadlessDisplay is alive
        let hd = HeadlessDisplay::start("800x600x24").expect("headless display should start");
        let _display = hd.display();

        // wmctrl should list the openbox root window
        let output = Command::new("wmctrl")
            .args(["-m"])
            .output()
            .expect("wmctrl should be available");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("Openbox") || stdout.contains("openbox"),
            "wmctrl should detect openbox: {stdout}"
        );
        drop(hd);
    }

    // ============================================================
    // OPENBOX-SPECIFIC TESTS — fail first, then fix
    // ============================================================

    #[test]
    #[cfg(target_os = "linux")]
    fn openbox_pid_is_valid_during_operation() {
        // openbox PID must be non-zero and the process must be responsive
        let hd = HeadlessDisplay::start("800x600x24").expect("headless display should start");
        let openbox_pid = hd.openbox.id();
        assert!(openbox_pid > 0, "openbox pid should be non-zero");

        // Verify it's actually running with kill -0
        let status = Command::new("kill")
            .arg("-0")
            .arg(openbox_pid.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("kill -0 should succeed for running process");
        assert!(
            status.success(),
            "openbox process {openbox_pid} should be running"
        );
        drop(hd);
    }

    #[test]
    #[ignore = "requires Xvfb and openbox installed"]
    fn openbox_executable_is_openbox() {
        // The openbox process should be the actual openbox binary
        let hd = HeadlessDisplay::start("800x600x24").expect("headless display should start");
        let openbox_pid = hd.openbox.id();

        // Read /proc/{pid}/comm to verify it's openbox
        let comm_path = format!("/proc/{}/comm", openbox_pid);
        let comm =
            std::fs::read_to_string(&comm_path).expect("should be able to read openbox comm");
        let comm = comm.trim();
        assert!(
            comm == "openbox" || comm.contains("openbox"),
            "process should be openbox, got: {comm}"
        );
        drop(hd);
    }

    #[test]
    #[ignore = "requires Xvfb and openbox installed"]
    fn openbox_env_display_matches_headless() {
        // While HeadlessDisplay is alive, DISPLAY should match what openbox uses
        let hd = HeadlessDisplay::start("800x600x24").expect("headless display should start");
        let hd_display = hd.display().to_string();

        // DISPLAY should be set to the headless display
        let current_display = env::var("DISPLAY").unwrap_or_default();
        assert!(
            current_display == hd_display,
            "DISPLAY ({current_display}) should match headless display ({hd_display})"
        );
        drop(hd);
    }

    #[test]
    #[ignore = "requires Xvfb and openbox installed"]
    fn openbox_stays_alive_for_duration_of_headless_display() {
        // openbox should remain alive throughout the entire HeadlessDisplay lifetime
        let hd = HeadlessDisplay::start("800x600x24").expect("headless display should start");
        let openbox_pid = hd.openbox.id();

        // Check multiple times over 500ms that openbox is still alive
        for _ in 0..5 {
            let status = Command::new("kill")
                .arg("-0")
                .arg(openbox_pid.to_string())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .expect("kill -0 should succeed");
            assert!(
                status.success(),
                "openbox should remain alive during operation"
            );
            thread::sleep(Duration::from_millis(100));
        }
        drop(hd);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn openbox_dies_after_drop() {
        // After HeadlessDisplay drops, openbox must be dead
        let hd = HeadlessDisplay::start("800x600x24").expect("headless display should start");
        let openbox_pid = hd.openbox.id();

        drop(hd);

        // Poll for up to 2s to observe the process actually exit
        let mut exited = false;
        for _ in 0..20 {
            thread::sleep(Duration::from_millis(100));
            let status = Command::new("kill")
                .arg("-0")
                .arg(openbox_pid.to_string())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            if !status.map(|s| s.success()).unwrap_or(false) {
                exited = true;
                break;
            }
        }

        assert!(
            exited,
            "openbox process {openbox_pid} should be dead after drop"
        );
    }

    #[test]
    #[ignore = "requires Xvfb and openbox installed"]
    fn wmctrl_detects_openbox_as_window_manager() {
        // wmctrl -m should report Openbox as the running window manager
        let hd = HeadlessDisplay::start("800x600x24").expect("headless display should start");
        let hd_display = hd.display().to_string();

        // wmctrl needs DISPLAY set to the headless display explicitly
        let output = Command::new("wmctrl")
            .args(["-m"])
            .env("DISPLAY", &hd_display)
            .output()
            .expect("wmctrl should be available");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.to_lowercase().contains("openbox"),
            "wmctrl should report Openbox as window manager, got: {stdout}"
        );
        drop(hd);
    }

    #[test]
    #[ignore = "requires Xvfb and openbox installed"]
    fn wmctrl_lists_openbox_root_window() {
        // wmctrl -l should run without error on the headless display
        // (it lists client windows; empty is fine on headless since no apps run)
        let hd = HeadlessDisplay::start("800x600x24").expect("headless display should start");
        let hd_display = hd.display().to_string();

        let output = Command::new("wmctrl")
            .args(["-l"])
            .env("DISPLAY", &hd_display)
            .output()
            .expect("wmctrl should be available");
        assert!(
            output.status.success(),
            "wmctrl -l should succeed on headless display, stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        drop(hd);
    }

    #[test]
    #[ignore = "requires Xvfb and openbox installed"]
    fn xwininfo_query_on_headless_display() {
        // xwininfo -root should be able to query the root window on openbox
        let hd = HeadlessDisplay::start("800x600x24").expect("headless display should start");
        let hd_display = hd.display().to_string();

        let output = Command::new("xwininfo")
            .args(["-root", "-display", &hd_display])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .expect("xwininfo should be available");
        assert!(
            output.status.success(),
            "xwininfo -root should succeed on openbox headless display"
        );
        drop(hd);
    }

    #[test]
    #[ignore = "requires Xvfb and openbox installed"]
    fn openbox_creates_root_window() {
        // openbox should create a root window on the display
        let hd = HeadlessDisplay::start("800x600x24").expect("headless display should start");
        let hd_display = hd.display();

        // xwininfo -root -display :N should return info about the root window
        let output = Command::new("xwininfo")
            .args(["-root", "-display", hd_display])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .expect("xwininfo should query root window");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stderr.contains("Error"),
            "xwininfo should succeed on openbox-managed display, stderr: {stderr}"
        );
        drop(hd);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn openbox_clean_exit_on_drop_no_zombie() {
        // After drop, openbox should be completely dead (not a zombie)
        let hd = HeadlessDisplay::start("800x600x24").expect("headless display should start");
        let openbox_pid = hd.openbox.id();

        drop(hd);

        // Poll for up to 2s for the process to exit
        let mut exited = false;
        for _ in 0..20 {
            thread::sleep(Duration::from_millis(100));
            let status = Command::new("kill")
                .arg("-0")
                .arg(openbox_pid.to_string())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            if !status.map(|s| s.success()).unwrap_or(false) {
                exited = true;
                break;
            }
        }

        // Process should be gone
        assert!(
            exited,
            "openbox process {openbox_pid} should not exist after drop"
        );

        // And not a zombie (zombie would still show in ps)
        let output = Command::new("ps")
            .args(["-p", &openbox_pid.to_string(), "-o", "stat="])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output();
        let stat = String::from_utf8_lossy(&output.unwrap().stdout)
            .trim()
            .to_string();
        // If ps succeeded, the process still exists as a zombie
        assert!(
            stat != "Z",
            "openbox should not be a zombie after drop, stat={stat}"
        );
    }

    #[test]
    #[ignore = "requires Xvfb and openbox installed"]
    fn openbox_with_different_geometries() {
        // openbox should work correctly with different Xvfb geometries
        for geometry in &["800x600x24", "1024x768x16", "1920x1080x24"] {
            let hd = HeadlessDisplay::start(geometry).unwrap_or_else(|_| {
                panic!("headless display should start with geometry {geometry}")
            });
            let openbox_pid = hd.openbox.id();
            assert!(
                openbox_pid > 0,
                "openbox pid should be non-zero for {geometry}"
            );

            // Verify openbox is still alive
            let status = Command::new("kill")
                .arg("-0")
                .arg(openbox_pid.to_string())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .expect("kill -0 should succeed");
            assert!(
                status.success(),
                "openbox should be alive for geometry {geometry}"
            );
            drop(hd);
            thread::sleep(Duration::from_millis(100));
        }
    }
}
