//! Thin read-only wrapper around shelling out (only `git` is used, §8).

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const STDOUT_TIMEOUT: Duration = Duration::from_secs(5);
const STATUS_TIMEOUT: Duration = Duration::from_secs(30);

/// Run a command, returning trimmed stdout on success (exit 0).
/// Returns `None` on spawn failure or non-zero exit — callers degrade gracefully.
pub fn run_stdout(program: &str, args: &[&str], cwd: Option<&Path>) -> Option<String> {
    run_stdout_with_timeout(program, args, cwd, STDOUT_TIMEOUT)
}

fn run_stdout_with_timeout(
    program: &str,
    args: &[&str],
    cwd: Option<&Path>,
    timeout: Duration,
) -> Option<String> {
    let resolved = resolve_program(program, cwd)?;
    let mut cmd = Command::new(resolved);
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    harden_command_env(&mut cmd);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let child = cmd.spawn().ok()?;
    let output = wait_output_with_timeout(child, timeout)?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Run a command for its exit status only.
pub fn run_status(program: &str, args: &[&str], cwd: Option<&Path>) -> Option<bool> {
    run_status_with_timeout(program, args, cwd, STATUS_TIMEOUT)
}

fn run_status_with_timeout(
    program: &str,
    args: &[&str],
    cwd: Option<&Path>,
    timeout: Duration,
) -> Option<bool> {
    let resolved = resolve_program(program, cwd)?;
    let mut cmd = Command::new(resolved);
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    harden_command_env(&mut cmd);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = cmd.spawn().ok()?;
    wait_status_with_timeout(&mut child, timeout)
}

/// Whether `git` is available on PATH.
pub fn git_available() -> bool {
    run_stdout("git", &["--version"], None).is_some()
}

fn harden_command_env(cmd: &mut Command) {
    // Keep git queries read-only and non-interactive. Callers degrade to
    // "unknown" on failure; prompting would turn a scan into a hang.
    cmd.env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .env("GIT_ASKPASS", "echo")
        .env("SSH_ASKPASS", "echo");
}

fn resolve_program(program: &str, cwd: Option<&Path>) -> Option<PathBuf> {
    let program_path = Path::new(program);
    if program_path.is_absolute() {
        return Some(program_path.to_path_buf());
    }
    if program.contains('/') || program.contains('\\') {
        return None;
    }
    let path = std::env::var_os("PATH")?;
    resolve_program_from_path(program, cwd, &path)
}

fn resolve_program_from_path(program: &str, cwd: Option<&Path>, path: &OsStr) -> Option<PathBuf> {
    for dir in std::env::split_paths(path) {
        if !dir.is_absolute() || path_entry_inside_cwd(&dir, cwd) {
            continue;
        }
        for candidate in candidate_paths(&dir, program) {
            if is_executable_file(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

fn path_entry_inside_cwd(dir: &Path, cwd: Option<&Path>) -> bool {
    let Some(cwd) = cwd else {
        return false;
    };
    let Ok(dir) = dir.canonicalize() else {
        return false;
    };
    let Ok(cwd) = cwd.canonicalize() else {
        return false;
    };
    dir.starts_with(cwd)
}

#[cfg(windows)]
fn candidate_paths(dir: &Path, program: &str) -> Vec<PathBuf> {
    if Path::new(program).extension().is_some() {
        return vec![dir.join(program)];
    }
    let pathext = std::env::var("PATHEXT").unwrap_or_else(|_| ".EXE;.BAT;.CMD".to_string());
    pathext
        .split(';')
        .filter(|ext| !ext.is_empty())
        .map(|ext| dir.join(format!("{program}{ext}")))
        .collect()
}

#[cfg(not(windows))]
fn candidate_paths(dir: &Path, program: &str) -> Vec<PathBuf> {
    vec![dir.join(program)]
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    std::fs::metadata(path)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

fn wait_status_with_timeout(child: &mut Child, timeout: Duration) -> Option<bool> {
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status.success()),
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

fn wait_output_with_timeout(mut child: Child, timeout: Duration) -> Option<std::process::Output> {
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return child.wait_with_output().ok(),
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    fn make_executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;

        let mut perms = std::fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn resolver_skips_relative_path_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let fake = tmp.path().join("git");
        std::fs::write(&fake, "#!/bin/sh\necho fake\n").unwrap();
        make_executable(&fake);

        assert!(resolve_program_from_path("git", Some(tmp.path()), OsStr::new(".")).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn resolver_skips_absolute_path_entries_inside_cwd() {
        let tmp = tempfile::tempdir().unwrap();
        let bin = tmp.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let fake = bin.join("git");
        std::fs::write(&fake, "#!/bin/sh\necho fake\n").unwrap();
        make_executable(&fake);

        assert!(
            resolve_program_from_path("git", Some(tmp.path()), bin.as_os_str()).is_none(),
            "repo-local PATH entry should be ignored"
        );
    }

    #[cfg(unix)]
    #[test]
    fn resolver_accepts_absolute_path_entries_outside_cwd() {
        let cwd = tempfile::tempdir().unwrap();
        let safe = tempfile::tempdir().unwrap();
        let fake = safe.path().join("git");
        std::fs::write(&fake, "#!/bin/sh\necho fake\n").unwrap();
        make_executable(&fake);

        assert_eq!(
            resolve_program_from_path("git", Some(cwd.path()), safe.path().as_os_str()),
            Some(fake)
        );
    }

    #[cfg(unix)]
    #[test]
    fn stdout_command_times_out() {
        let start = Instant::now();
        let out = run_stdout_with_timeout("sleep", &["1"], None, Duration::from_millis(50));

        assert!(out.is_none());
        assert!(start.elapsed() < Duration::from_secs(1));
    }

    #[cfg(unix)]
    #[test]
    fn status_command_times_out() {
        let start = Instant::now();
        let ok = run_status_with_timeout("sleep", &["1"], None, Duration::from_millis(50));

        assert!(ok.is_none());
        assert!(start.elapsed() < Duration::from_secs(1));
    }

    #[cfg(unix)]
    #[test]
    fn stdout_command_collects_trimmed_output() {
        let out = run_stdout_with_timeout(
            "sh",
            &["-c", "printf 'hello\\n'"],
            None,
            Duration::from_secs(1),
        );

        assert_eq!(out.as_deref(), Some("hello"));
    }
}
