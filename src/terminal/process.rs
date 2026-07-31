//! Child-process creation and supervision for terminal sessions.

use super::{OutputStream, Session, SessionStatus, append_output};
use std::{
    io::Read,
    path::Path,
    process::{Child, Command, Stdio},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

pub(super) fn spawn_shell(
    command: &str,
    cwd: Option<&Path>,
    interactive: bool,
) -> Result<Child, std::io::Error> {
    #[cfg(windows)]
    let mut process = {
        use std::os::windows::process::CommandExt;
        let mut value = Command::new("cmd.exe");
        value.args(["/D", "/S", "/C"]);
        let command = format!("chcp 65001 >nul & {command}");
        value.raw_arg(format!("\"{command}\""));
        value
    };
    #[cfg(not(windows))]
    let mut process = {
        use std::os::unix::process::CommandExt;
        let mut value = Command::new("/bin/sh");
        value.args(["-c", command]);
        value.process_group(0);
        value
    };

    if let Some(cwd) = cwd {
        process.current_dir(cwd);
    }
    process
        .stdin(if interactive {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}

pub(super) fn spawn_reader(
    session: Arc<Session>,
    mut reader: impl Read + Send + 'static,
    stream: OutputStream,
) -> thread::JoinHandle<Result<(), std::io::Error>> {
    thread::spawn(move || {
        let mut buffer = [0_u8; 16 * 1024];
        loop {
            let read = reader.read(&mut buffer)?;
            if read == 0 {
                return Ok(());
            }
            if let Ok(mut state) = session.state.lock() {
                append_output(&session, &mut state, stream, &buffer[..read]);
                session.changed.notify_all();
            } else {
                return Err(std::io::Error::other("terminal session lock was poisoned"));
            }
        }
    })
}

pub(super) fn spawn_waiter(
    session: Arc<Session>,
    mut child: Child,
    timeout: Duration,
    stdout_reader: thread::JoinHandle<Result<(), std::io::Error>>,
    stderr_reader: thread::JoinHandle<Result<(), std::io::Error>>,
) {
    thread::spawn(move || {
        // Publish the final state only after readers have drained both pipes.
        let deadline = session.started + timeout;
        let (status, final_status, timed_out, error) = loop {
            match child.try_wait() {
                Ok(Some(status)) => break (Some(status), SessionStatus::Exited, false, None),
                Ok(None) => {}
                Err(error) => break (None, SessionStatus::Failed, false, Some(error.to_string())),
            }

            let kill_requested = session
                .state
                .lock()
                .map(|state| state.kill_requested)
                .unwrap_or(true);
            if kill_requested || Instant::now() >= deadline {
                let timed_out = !kill_requested;
                let final_status = if timed_out {
                    SessionStatus::TimedOut
                } else {
                    SessionStatus::Killed
                };
                let error = terminate_process_tree(&mut child)
                    .err()
                    .map(|e| e.to_string());
                let status = child.wait().ok();
                break (status, final_status, timed_out, error);
            }
            thread::sleep(Duration::from_millis(20));
        };

        let stdout_error = stdout_reader
            .join()
            .map_err(|_| "stdout reader thread panicked".to_owned())
            .and_then(|result| result.map_err(|error| error.to_string()))
            .err();
        let stderr_error = stderr_reader
            .join()
            .map_err(|_| "stderr reader thread panicked".to_owned())
            .and_then(|result| result.map_err(|error| error.to_string()))
            .err();

        if let Ok(mut state) = session.state.lock() {
            state.status = if error.is_some() || stdout_error.is_some() || stderr_error.is_some() {
                SessionStatus::Failed
            } else {
                final_status
            };
            state.exit_code = status.and_then(|value| value.code());
            state.timed_out = timed_out;
            state.stdin.take();
            state.completed_at = Some(Instant::now());
            state.error = error.or(stdout_error).or(stderr_error);
            session.changed.notify_all();
        }
    });
}

fn terminate_process_tree(child: &mut Child) -> Result<(), std::io::Error> {
    #[cfg(windows)]
    {
        let pid = child.id().to_string();
        let status = Command::new("taskkill")
            .args(["/PID", &pid, "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if !status.is_ok_and(|value| value.success()) {
            child.kill()?;
        }
    }
    #[cfg(not(windows))]
    {
        let group = format!("-{}", child.id());
        let status = Command::new("kill")
            .args(["-KILL", "--", &group])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if !status.is_ok_and(|value| value.success()) {
            child.kill()?;
        }
    }
    Ok(())
}
