//! Bounded command execution and persistent terminal sessions.
//!
//! A [`Terminal`] owns a registry of child processes. Reader threads capture
//! stdout and stderr into a cursor-addressed ring buffer; a waiter thread
//! enforces deadlines and records the final session state. All externally
//! visible reads and retained output are size-bounded.

use serde::Serialize;
use std::{
    collections::{HashMap, VecDeque},
    io::{Read, Write},
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc, Condvar, Mutex, MutexGuard,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use thiserror::Error;

#[derive(Debug, Error)]
/// An error produced by command or session management.
pub enum TerminalError {
    #[error("terminal command execution is disabled")]
    /// Command execution is disabled by configuration.
    Disabled,
    #[error("command must not be empty")]
    /// The command contained no non-whitespace characters.
    EmptyCommand,
    #[error("timeout must be greater than zero and no more than {max} ms")]
    /// The requested timeout was zero or exceeded the configured maximum.
    InvalidTimeout {
        /// Largest timeout accepted by the terminal manager, in milliseconds.
        max: u64,
    },
    #[error("wait must not exceed {max} ms")]
    /// The requested long-poll wait exceeded the configured maximum.
    InvalidWait {
        /// Largest long-poll wait accepted, in milliseconds.
        max: u64,
    },
    #[error("read limit must be greater than zero and no more than {max} bytes")]
    /// The requested output read size was zero or too large.
    InvalidReadLimit {
        /// Largest output read accepted, in bytes.
        max: usize,
    },
    #[error("maximum number of concurrent terminal sessions ({max}) reached")]
    /// The configured concurrent-session limit has been reached.
    Busy {
        /// Maximum number of simultaneously running sessions.
        max: usize,
    },
    #[error("terminal session not found: {0}")]
    /// No retained session exists for the supplied identifier.
    SessionNotFound(String),
    #[error("terminal session is no longer running")]
    /// The operation requires a running session.
    NotRunning,
    #[error("terminal session stdin is closed")]
    /// The child process no longer has writable standard input.
    StdinClosed,
    #[error("terminal session lock was poisoned")]
    /// Shared session state was poisoned by a panic while locked.
    LockPoisoned,
    #[error(transparent)]
    /// The operating system reported an I/O error.
    Io(#[from] std::io::Error),
    #[error("output reader thread failed")]
    /// A background output-reader thread failed.
    ReaderThread,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
/// Lifecycle state of a terminal session.
pub enum SessionStatus {
    /// The child process is still running.
    Running,
    /// The child process exited without server intervention.
    Exited,
    /// Termination was explicitly requested.
    Killed,
    /// The configured execution deadline elapsed.
    TimedOut,
    /// Process supervision or output collection failed.
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
/// Source stream for one captured output event.
pub enum OutputStream {
    /// Standard output.
    Stdout,
    /// Standard error.
    Stderr,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// A contiguous chunk of captured terminal output.
pub struct OutputEvent {
    /// Cursor immediately after this event's bytes.
    pub cursor: u64,
    /// Child-process stream that produced the bytes.
    pub stream: OutputStream,
    /// UTF-8 text, with invalid byte sequences replaced lossily.
    pub text: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Result returned after starting a persistent command.
pub struct StartOutput {
    /// Opaque identifier used by subsequent session operations.
    pub session_id: String,
    /// Session state at the time of the response.
    pub status: SessionStatus,
    /// Cursor immediately after this event's bytes.
    pub cursor: u64,
    /// Operating-system process identifier of the shell.
    pub pid: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Snapshot returned by a cursor-based session read.
pub struct ReadOutput {
    /// Opaque identifier used by subsequent session operations.
    pub session_id: String,
    /// Session state at the time of the response.
    pub status: SessionStatus,
    /// Output chunks after the requested cursor, subject to the read limit.
    pub events: Vec<OutputEvent>,
    /// Cursor to supply on the next read.
    pub next_cursor: u64,
    /// Earliest cursor still retained in the session buffer.
    pub oldest_cursor: u64,
    /// Whether requested output had already been evicted.
    pub output_dropped: bool,
    /// Process exit code when available.
    pub exit_code: Option<i32>,
    /// Whether supervision terminated the process at its deadline.
    pub timed_out: bool,
    /// Whether output exceeded the retention limit.
    pub truncated: bool,
    /// Milliseconds elapsed since the session started.
    pub duration_ms: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Background supervision error, when one occurred.
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Aggregated result of a synchronous command run.
pub struct CommandOutput {
    /// Process exit code when available.
    pub exit_code: Option<i32>,
    /// Captured standard output.
    pub stdout: String,
    /// Captured standard error.
    pub stderr: String,
    /// Whether supervision terminated the process at its deadline.
    pub timed_out: bool,
    /// Whether output exceeded the retention limit.
    pub truncated: bool,
    /// Milliseconds elapsed since the session started.
    pub duration_ms: u128,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Result of writing bytes to a session's standard input.
pub struct WriteOutput {
    /// Opaque identifier used by subsequent session operations.
    pub session_id: String,
    /// Number of bytes accepted by the operating system.
    pub written: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Result of requesting termination of a session.
pub struct KillOutput {
    /// Opaque identifier used by subsequent session operations.
    pub session_id: String,
    /// Whether the session was running and a kill was newly requested.
    pub requested: bool,
    /// Session state at the time of the response.
    pub status: SessionStatus,
}

#[derive(Clone)]
/// Cloneable manager for bounded child-process sessions.
pub struct Terminal {
    inner: Arc<TerminalInner>,
}

struct TerminalInner {
    enabled: bool,
    max_concurrency: usize,
    default_timeout_ms: u64,
    max_timeout_ms: u64,
    max_output_bytes: usize,
    max_read_bytes: usize,
    max_wait_ms: u64,
    retention: Duration,
    next_id: AtomicU64,
    sessions: Mutex<HashMap<String, Arc<Session>>>,
}

struct Session {
    id: String,
    started: Instant,
    max_output_bytes: usize,
    state: Mutex<SessionState>,
    changed: Condvar,
}

struct SessionState {
    status: SessionStatus,
    exit_code: Option<i32>,
    timed_out: bool,
    truncated: bool,
    error: Option<String>,
    completed_at: Option<Instant>,
    kill_requested: bool,
    stdin: Option<ChildStdin>,
    events: VecDeque<BufferedEvent>,
    buffered_bytes: usize,
    next_cursor: u64,
    dropped_until: u64,
}

struct BufferedEvent {
    start_cursor: u64,
    end_cursor: u64,
    stream: OutputStream,
    bytes: Vec<u8>,
}

impl Terminal {
    #[allow(clippy::too_many_arguments)]
    /// Creates a terminal manager with explicit execution and resource limits.
    pub fn new(
        enabled: bool,
        max_concurrency: usize,
        default_timeout_ms: u64,
        max_timeout_ms: u64,
        max_output_bytes: usize,
        max_read_bytes: usize,
        max_wait_ms: u64,
        retention_ms: u64,
    ) -> Self {
        Self {
            inner: Arc::new(TerminalInner {
                enabled,
                max_concurrency,
                default_timeout_ms,
                max_timeout_ms,
                max_output_bytes,
                max_read_bytes,
                max_wait_ms,
                retention: Duration::from_millis(retention_ms),
                next_id: AtomicU64::new(1),
                sessions: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// Starts a shell command and returns immediately with a session identifier.
    ///
    /// Output collection and timeout enforcement continue in background threads.
    pub fn start(
        &self,
        command: &str,
        cwd: Option<&Path>,
        timeout_ms: Option<u64>,
    ) -> Result<StartOutput, TerminalError> {
        self.ensure_enabled()?;
        if command.trim().is_empty() {
            return Err(TerminalError::EmptyCommand);
        }
        let timeout_ms = self.validate_timeout(timeout_ms)?;
        self.cleanup()?;

        let mut sessions = lock(&self.inner.sessions)?;
        let active = sessions
            .values()
            .filter(|session| {
                lock(&session.state)
                    .map(|state| state.status == SessionStatus::Running)
                    .unwrap_or(true)
            })
            .count();
        if active >= self.inner.max_concurrency {
            return Err(TerminalError::Busy {
                max: self.inner.max_concurrency,
            });
        }

        let mut child = spawn_shell(command, cwd, true)?;
        let pid = child.id();
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| std::io::Error::other("failed to capture command stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| std::io::Error::other("failed to capture command stderr"))?;
        let stdin = child.stdin.take();
        let sequence = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let id = format!("term-{pid}-{sequence}");
        let session = Arc::new(Session {
            id: id.clone(),
            started: Instant::now(),
            max_output_bytes: self.inner.max_output_bytes,
            state: Mutex::new(SessionState {
                status: SessionStatus::Running,
                exit_code: None,
                timed_out: false,
                truncated: false,
                error: None,
                completed_at: None,
                kill_requested: false,
                stdin,
                events: VecDeque::new(),
                buffered_bytes: 0,
                next_cursor: 1,
                dropped_until: 0,
            }),
            changed: Condvar::new(),
        });
        sessions.insert(id.clone(), session.clone());
        drop(sessions);

        let stdout_reader = spawn_reader(session.clone(), stdout, OutputStream::Stdout);
        let stderr_reader = spawn_reader(session.clone(), stderr, OutputStream::Stderr);
        spawn_waiter(
            session,
            child,
            Duration::from_millis(timeout_ms),
            stdout_reader,
            stderr_reader,
        );

        Ok(StartOutput {
            session_id: id,
            status: SessionStatus::Running,
            cursor: 0,
            pid,
        })
    }

    /// Reads retained output after `cursor`, optionally waiting for new data.
    ///
    /// Reads are cursor-based and do not consume data. If the caller falls behind
    /// buffer eviction, [`ReadOutput::output_dropped`] is set.
    pub fn read(
        &self,
        session_id: &str,
        cursor: u64,
        wait_ms: Option<u64>,
        max_bytes: Option<usize>,
    ) -> Result<ReadOutput, TerminalError> {
        self.ensure_enabled()?;
        let wait_ms = wait_ms.unwrap_or(0);
        if wait_ms > self.inner.max_wait_ms {
            return Err(TerminalError::InvalidWait {
                max: self.inner.max_wait_ms,
            });
        }
        let max_bytes = max_bytes.unwrap_or(self.inner.max_read_bytes);
        if max_bytes == 0 || max_bytes > self.inner.max_read_bytes {
            return Err(TerminalError::InvalidReadLimit {
                max: self.inner.max_read_bytes,
            });
        }
        let session = self.get_session(session_id)?;
        let mut state = lock(&session.state)?;
        if state.status == SessionStatus::Running
            && !has_output_after(&state, cursor)
            && wait_ms > 0
        {
            let (next, _) = session
                .changed
                .wait_timeout_while(state, Duration::from_millis(wait_ms), |value| {
                    value.status == SessionStatus::Running && !has_output_after(value, cursor)
                })
                .map_err(|_| TerminalError::LockPoisoned)?;
            state = next;
        }

        // Cursors are absolute byte positions. Eviction advances `dropped_until`,
        // allowing clients to detect gaps instead of silently losing output.
        let oldest_cursor = state
            .events
            .front()
            .map(|event| event.start_cursor)
            .unwrap_or(state.next_cursor);
        let output_dropped = cursor < state.dropped_until;
        let effective_cursor = cursor.max(state.dropped_until);
        let mut used = 0_usize;
        let mut events = Vec::new();
        let mut next_cursor = effective_cursor;
        for event in state
            .events
            .iter()
            .filter(|event| event.end_cursor > effective_cursor)
        {
            let offset = effective_cursor.saturating_sub(event.start_cursor) as usize;
            let available = &event.bytes[offset.min(event.bytes.len())..];
            let kept = available.len().min(max_bytes.saturating_sub(used));
            if kept == 0 {
                break;
            }
            let bytes = &available[..kept];
            used += kept;
            next_cursor = event.start_cursor + offset as u64 + kept as u64;
            events.push(OutputEvent {
                cursor: next_cursor,
                stream: event.stream,
                text: String::from_utf8_lossy(bytes).into_owned(),
            });
            if used >= max_bytes {
                break;
            }
        }

        Ok(ReadOutput {
            session_id: session.id.clone(),
            status: state.status,
            events,
            next_cursor,
            oldest_cursor,
            output_dropped,
            exit_code: state.exit_code,
            timed_out: state.timed_out,
            truncated: state.truncated,
            duration_ms: session.started.elapsed().as_millis(),
            error: state.error.clone(),
        })
    }

    /// Writes `data` to a running session's standard input and flushes it.
    pub fn write(&self, session_id: &str, data: &[u8]) -> Result<WriteOutput, TerminalError> {
        self.ensure_enabled()?;
        let session = self.get_session(session_id)?;
        let mut state = lock(&session.state)?;
        if state.status != SessionStatus::Running {
            return Err(TerminalError::NotRunning);
        }
        let stdin = state.stdin.as_mut().ok_or(TerminalError::StdinClosed)?;
        stdin.write_all(data)?;
        stdin.flush()?;
        Ok(WriteOutput {
            session_id: session.id.clone(),
            written: data.len(),
        })
    }

    /// Closes a running session's standard input to deliver end-of-file.
    pub fn close_stdin(&self, session_id: &str) -> Result<(), TerminalError> {
        self.ensure_enabled()?;
        let session = self.get_session(session_id)?;
        let mut state = lock(&session.state)?;
        if state.status != SessionStatus::Running {
            return Err(TerminalError::NotRunning);
        }
        state.stdin.take().ok_or(TerminalError::StdinClosed)?;
        Ok(())
    }

    /// Requests asynchronous termination of a running process tree.
    pub fn kill(&self, session_id: &str) -> Result<KillOutput, TerminalError> {
        self.ensure_enabled()?;
        let session = self.get_session(session_id)?;
        let mut state = lock(&session.state)?;
        let requested = state.status == SessionStatus::Running;
        if requested {
            state.kill_requested = true;
            state.stdin.take();
            session.changed.notify_all();
        }
        Ok(KillOutput {
            session_id: session.id.clone(),
            requested,
            status: state.status,
        })
    }

    /// Removes a completed session and its retained output immediately.
    pub fn close(&self, session_id: &str) -> Result<(), TerminalError> {
        self.ensure_enabled()?;
        let mut sessions = lock(&self.inner.sessions)?;
        let session = sessions
            .get(session_id)
            .cloned()
            .ok_or_else(|| TerminalError::SessionNotFound(session_id.to_owned()))?;
        if lock(&session.state)?.status == SessionStatus::Running {
            return Err(TerminalError::NotRunning);
        }
        sessions.remove(session_id);
        Ok(())
    }

    /// Runs a command to completion and aggregates stdout and stderr.
    ///
    /// This convenience method is implemented with the same persistent-session
    /// primitives as [`Self::start`] and [`Self::read`].
    pub fn run(
        &self,
        command: &str,
        cwd: Option<&Path>,
        timeout_ms: Option<u64>,
    ) -> Result<CommandOutput, TerminalError> {
        let started = self.start(command, cwd, timeout_ms)?;
        let mut cursor = 0;
        let mut stdout = String::new();
        let mut stderr = String::new();
        loop {
            let output = self.read(
                &started.session_id,
                cursor,
                Some(self.inner.max_wait_ms),
                Some(self.inner.max_read_bytes),
            )?;
            for event in output.events {
                cursor = cursor.max(event.cursor);
                match event.stream {
                    OutputStream::Stdout => stdout.push_str(&event.text),
                    OutputStream::Stderr => stderr.push_str(&event.text),
                }
            }
            if output.status != SessionStatus::Running {
                let result = CommandOutput {
                    exit_code: output.exit_code,
                    stdout,
                    stderr,
                    timed_out: output.timed_out,
                    truncated: output.truncated || output.output_dropped,
                    duration_ms: output.duration_ms,
                };
                let _ = self.close(&started.session_id);
                return Ok(result);
            }
        }
    }

    fn ensure_enabled(&self) -> Result<(), TerminalError> {
        if self.inner.enabled {
            Ok(())
        } else {
            Err(TerminalError::Disabled)
        }
    }

    fn validate_timeout(&self, timeout_ms: Option<u64>) -> Result<u64, TerminalError> {
        let timeout_ms = timeout_ms.unwrap_or(self.inner.default_timeout_ms);
        if timeout_ms == 0 || timeout_ms > self.inner.max_timeout_ms {
            return Err(TerminalError::InvalidTimeout {
                max: self.inner.max_timeout_ms,
            });
        }
        Ok(timeout_ms)
    }

    fn get_session(&self, session_id: &str) -> Result<Arc<Session>, TerminalError> {
        self.cleanup()?;
        lock(&self.inner.sessions)?
            .get(session_id)
            .cloned()
            .ok_or_else(|| TerminalError::SessionNotFound(session_id.to_owned()))
    }

    fn cleanup(&self) -> Result<(), TerminalError> {
        let now = Instant::now();
        let retention = self.inner.retention;
        lock(&self.inner.sessions)?.retain(|_, session| {
            lock(&session.state)
                .map(|state| {
                    state
                        .completed_at
                        .is_none_or(|completed| now.duration_since(completed) < retention)
                })
                .unwrap_or(true)
        });
        Ok(())
    }
}

impl Drop for TerminalInner {
    fn drop(&mut self) {
        if let Ok(sessions) = self.sessions.lock() {
            for session in sessions.values() {
                if let Ok(mut state) = session.state.lock()
                    && state.status == SessionStatus::Running
                {
                    state.kill_requested = true;
                    state.stdin.take();
                    session.changed.notify_all();
                }
            }
        }
    }
}

fn spawn_shell(
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

fn spawn_reader(
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

fn spawn_waiter(
    session: Arc<Session>,
    mut child: Child,
    timeout: Duration,
    stdout_reader: thread::JoinHandle<Result<(), std::io::Error>>,
    stderr_reader: thread::JoinHandle<Result<(), std::io::Error>>,
) {
    thread::spawn(move || {
        // One waiter owns child reaping. Reader threads must finish before the
        // final state is published so a terminal read cannot miss trailing bytes.
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

fn append_output(session: &Session, state: &mut SessionState, stream: OutputStream, bytes: &[u8]) {
    // Retain only the newest bytes. Cursor values still account for discarded
    // prefixes, which makes truncation observable to incremental readers.
    let full_start = state.next_cursor;
    state.next_cursor = state.next_cursor.saturating_add(bytes.len() as u64);
    let retained = if bytes.len() > session.max_output_bytes {
        state.truncated = true;
        &bytes[bytes.len() - session.max_output_bytes..]
    } else {
        bytes
    };
    let start_cursor = full_start + (bytes.len() - retained.len()) as u64;
    if start_cursor > full_start {
        state.dropped_until = state.dropped_until.max(start_cursor);
    }
    while state.buffered_bytes.saturating_add(retained.len()) > session.max_output_bytes {
        if let Some(removed) = state.events.pop_front() {
            state.buffered_bytes = state.buffered_bytes.saturating_sub(removed.bytes.len());
            state.dropped_until = state.dropped_until.max(removed.end_cursor);
            state.truncated = true;
        } else {
            break;
        }
    }
    state.buffered_bytes += retained.len();
    state.events.push_back(BufferedEvent {
        start_cursor,
        end_cursor: start_cursor + retained.len() as u64,
        stream,
        bytes: retained.to_vec(),
    });
}

fn has_output_after(state: &SessionState, cursor: u64) -> bool {
    state.next_cursor > cursor
}

fn lock<T>(mutex: &Mutex<T>) -> Result<MutexGuard<'_, T>, TerminalError> {
    mutex.lock().map_err(|_| TerminalError::LockPoisoned)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn terminal() -> Terminal {
        Terminal::new(true, 2, 2_000, 5_000, 64 * 1024, 64 * 1024, 1_000, 60_000)
    }

    #[test]
    fn rejects_commands_when_disabled() {
        let terminal = Terminal::new(false, 1, 1_000, 2_000, 1024, 1024, 100, 1_000);
        assert!(matches!(
            terminal.run("echo hello", None, None),
            Err(TerminalError::Disabled)
        ));
    }

    #[test]
    fn runs_a_shell_command_compatibly() {
        let output = terminal().run("echo hello", None, None).unwrap();
        assert_eq!(output.exit_code, Some(0));
        assert!(output.stdout.contains("hello"));
        assert!(!output.timed_out);
    }

    #[test]
    fn reads_a_long_running_command_without_restarting_it() {
        #[cfg(windows)]
        let command = "echo first & ping -n 2 127.0.0.1 >nul & echo second";
        #[cfg(not(windows))]
        let command = "printf 'first\\n'; sleep 0.1; printf 'second\\n'";
        let terminal = terminal();
        let started = terminal.start(command, None, None).unwrap();
        let mut cursor = 0;
        let mut combined = String::new();
        loop {
            let output = terminal
                .read(&started.session_id, cursor, Some(1_000), None)
                .unwrap();
            cursor = output.next_cursor;
            combined.extend(output.events.into_iter().map(|event| event.text));
            if output.status != SessionStatus::Running {
                break;
            }
        }
        assert!(combined.contains("first"));
        assert!(combined.contains("second"));
    }

    #[test]
    fn can_write_to_stdin_and_close_it() {
        #[cfg(windows)]
        let command = "more";
        #[cfg(not(windows))]
        let command = "cat";
        let terminal = terminal();
        let started = terminal.start(command, None, None).unwrap();
        terminal
            .write(&started.session_id, b"hello stdin\n")
            .unwrap();
        terminal.close_stdin(&started.session_id).unwrap();
        let mut cursor = 0;
        let mut combined = String::new();
        loop {
            let output = terminal
                .read(&started.session_id, cursor, Some(1_000), None)
                .unwrap();
            cursor = output.next_cursor;
            combined.extend(output.events.into_iter().map(|event| event.text));
            if output.status != SessionStatus::Running {
                break;
            }
        }
        assert!(combined.contains("hello stdin"));
    }

    #[test]
    fn kill_stops_a_running_session() {
        #[cfg(windows)]
        let command = "ping -n 20 127.0.0.1 >nul";
        #[cfg(not(windows))]
        let command = "sleep 20";
        let terminal = terminal();
        let started = terminal.start(command, None, None).unwrap();
        assert!(terminal.kill(&started.session_id).unwrap().requested);
        let mut output = terminal
            .read(&started.session_id, 0, Some(1_000), None)
            .unwrap();
        while output.status == SessionStatus::Running {
            output = terminal
                .read(&started.session_id, output.next_cursor, Some(1_000), None)
                .unwrap();
        }
        assert_eq!(output.status, SessionStatus::Killed);
    }

    #[test]
    fn enforces_concurrency_limit_for_session_lifetime() {
        #[cfg(windows)]
        let command = "ping -n 20 127.0.0.1 >nul";
        #[cfg(not(windows))]
        let command = "sleep 20";
        let terminal = Terminal::new(true, 1, 2_000, 30_000, 1024, 1024, 100, 1_000);
        let started = terminal.start(command, None, None).unwrap();
        assert!(matches!(
            terminal.start(command, None, None),
            Err(TerminalError::Busy { max: 1 })
        ));
        terminal.kill(&started.session_id).unwrap();
    }
}
