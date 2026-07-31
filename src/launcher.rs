use crate::settings::CompanionWindow;
use anyhow::{Context, Result, bail};
use std::{
    io::{BufRead, BufReader},
    net::IpAddr,
    path::Path,
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, TryRecvError},
    },
};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub struct CompanionProcess {
    child: Child,
    name: &'static str,
    output: Arc<Mutex<Vec<String>>>,
    failure: Option<String>,
}

pub enum Tunnel<'a> {
    Ngrok {
        program: &'a Path,
        extra_args: &'a [String],
    },
    Cloudflared {
        program: &'a Path,
        extra_args: &'a [String],
    },
    Zrok {
        program: &'a Path,
        extra_args: &'a [String],
    },
}

impl CompanionProcess {
    /// Returns whether the provider process is still running.
    pub fn is_running(&mut self) -> Result<bool> {
        match self.child.try_wait()? {
            None => {
                let diagnostic = diagnostic_output(&self.output);
                if diagnostic.is_empty() {
                    Ok(true)
                } else {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    self.failure = Some(format!(
                        "{} reported a tunnel failure: {diagnostic}",
                        self.name
                    ));
                    Ok(false)
                }
            }
            Some(status) => {
                self.failure = Some(format!(
                    "{} exited with status {status}: {}",
                    self.name,
                    diagnostic_output(&self.output)
                ));
                Ok(false)
            }
        }
    }

    pub fn failure(&self) -> Option<&str> {
        self.failure.as_deref()
    }

    pub fn start_tunnel(
        tunnel: Tunnel<'_>,
        host: IpAddr,
        port: u16,
        window: CompanionWindow,
    ) -> Result<Self> {
        Self::start_tunnel_inner(tunnel, host, port, window, None)
    }

    pub fn start_tunnel_with_url_callback(
        tunnel: Tunnel<'_>,
        host: IpAddr,
        port: u16,
        window: CompanionWindow,
        on_public_url: impl Fn(String) + Send + Sync + 'static,
    ) -> Result<Self> {
        Self::start_tunnel_inner(tunnel, host, port, window, Some(Arc::new(on_public_url)))
    }

    fn start_tunnel_inner(
        tunnel: Tunnel<'_>,
        host: IpAddr,
        port: u16,
        window: CompanionWindow,
        on_public_url: Option<Arc<dyn Fn(String) + Send + Sync>>,
    ) -> Result<Self> {
        let origin_host = if host.is_unspecified() {
            match host {
                IpAddr::V4(_) => "127.0.0.1".to_owned(),
                IpAddr::V6(_) => "[::1]".to_owned(),
            }
        } else {
            match host {
                IpAddr::V4(address) => address.to_string(),
                IpAddr::V6(address) => format!("[{address}]"),
            }
        };
        let origin = format!("http://{origin_host}:{port}");
        let (name, program, extra_args) = match tunnel {
            Tunnel::Ngrok {
                program,
                extra_args,
            } => ("ngrok", program, extra_args),
            Tunnel::Cloudflared {
                program,
                extra_args,
            } => ("cloudflared", program, extra_args),
            Tunnel::Zrok {
                program,
                extra_args,
            } => ("zrok", program, extra_args),
        };
        let mut command = Command::new(program);
        match tunnel {
            Tunnel::Ngrok { .. } => {
                command.arg("http").arg(port.to_string());
                if on_public_url.is_some() {
                    command.args(["--log", "stdout", "--log-format", "logfmt"]);
                }
            }
            Tunnel::Cloudflared { .. } => {
                command.args(["tunnel", "--url", &origin]);
            }
            Tunnel::Zrok { .. } => {
                command.args(["share", "public", &origin]);
            }
        }
        command.args(extra_args);

        if on_public_url.is_some() {
            command.stdout(Stdio::piped()).stderr(Stdio::piped());
        }

        match window {
            CompanionWindow::Hidden => {
                command.stdin(Stdio::null());
                #[cfg(windows)]
                command.creation_flags(CREATE_NO_WINDOW);
            }
            CompanionWindow::New => {
                #[cfg(windows)]
                command.creation_flags(CREATE_NEW_CONSOLE);
            }
            CompanionWindow::Inherit => {}
        }

        let mut child = command.spawn().with_context(|| {
            format!(
                "failed to start {name} using `{}`; install it or pass --{name}-bin <PROGRAM>",
                program.display()
            )
        })?;

        let output = Arc::new(Mutex::new(Vec::new()));
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);

        let wait_for_ready = on_public_url.is_some();
        if let Some(callback) = on_public_url {
            let callback_sent = Arc::new(AtomicBool::new(false));
            let output_for_callback = Arc::clone(&output);
            let callback: Arc<dyn Fn(String) + Send + Sync> = Arc::new(move |url: String| {
                let _ = ready_tx.send(url.clone());
                callback(url);
            });
            if let Some(stdout) = child.stdout.take() {
                read_public_url(
                    stdout,
                    name,
                    Arc::clone(&callback),
                    Arc::clone(&callback_sent),
                    Arc::clone(&output_for_callback),
                );
            }
            if let Some(stderr) = child.stderr.take() {
                read_public_url(stderr, name, callback, callback_sent, Arc::clone(&output));
            }
        }

        if wait_for_ready {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
            loop {
                match ready_rx.try_recv() {
                    Ok(_) => break,
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => {
                        let _ = child.kill();
                        let _ = child.wait();
                        bail!(
                            "{name} stopped before publishing a public URL: {}",
                            diagnostic_output(&output)
                        );
                    }
                }
                if let Some(status) = child.try_wait()? {
                    let diagnostic = diagnostic_output(&output);
                    bail!(
                        "{name} exited before the tunnel became ready with status {status}: {}",
                        diagnostic
                    );
                }
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    bail!(
                        "{name} did not publish a public URL within 15 seconds: {}",
                        diagnostic_output(&output)
                    );
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        } else {
            std::thread::sleep(std::time::Duration::from_millis(150));
            if let Some(status) = child
                .try_wait()
                .with_context(|| format!("failed to inspect {name} process"))?
            {
                let _ = child.kill();
                let _ = child.wait();
                bail!("{name} exited immediately with status {status}");
            }
        }

        tracing::info!(pid = child.id(), %origin, ?window, provider = name, "tunnel started");
        Ok(Self {
            child,
            name,
            output,
            failure: None,
        })
    }
}

fn read_public_url(
    stream: impl std::io::Read + Send + 'static,
    provider: &'static str,
    callback: Arc<dyn Fn(String) + Send + Sync>,
    callback_sent: Arc<AtomicBool>,
    output: Arc<Mutex<Vec<String>>>,
) {
    std::thread::spawn(move || {
        for line in BufReader::new(stream)
            .lines()
            .map_while(std::result::Result::ok)
        {
            tracing::info!(provider, output = %line, "tunnel output");
            if let Ok(mut lines) = output.lock() {
                lines.push(line.clone());
                if lines.len() > 40 {
                    lines.remove(0);
                }
            }
            if let Some(url) = extract_public_url(&line, provider) {
                if !callback_sent.swap(true, Ordering::AcqRel) {
                    callback(url);
                }
            }
        }
    });
}

fn diagnostic_output(output: &Arc<Mutex<Vec<String>>>) -> String {
    output
        .lock()
        .map(|lines| {
            lines
                .iter()
                .filter(|line| {
                    let lower = line.to_ascii_lowercase();
                    lower.contains("error") || lower.contains("err_") || lower.contains("failed")
                })
                .cloned()
                .collect::<Vec<_>>()
                .join(" | ")
        })
        .unwrap_or_default()
}

fn extract_public_url(line: &str, provider: &str) -> Option<String> {
    let start = line.find("https://")?;
    let candidate = &line[start..];
    let end = candidate
        .find(|character: char| {
            character.is_whitespace()
                || matches!(character, '"' | '\'' | ')' | ']' | '}' | '|' | '\x1b')
        })
        .unwrap_or(candidate.len());
    let url = candidate[..end].trim_end_matches(['/', ',', ';', '.']);
    let expected_host = match provider {
        "ngrok" => "ngrok",
        "cloudflared" => "trycloudflare.com",
        "zrok" => "zrok",
        _ => return None,
    };
    url.contains(expected_host).then(|| url.to_owned())
}

#[cfg(test)]
mod tests {
    use super::extract_public_url;

    #[test]
    fn extracts_provider_urls_from_common_output_formats() {
        assert_eq!(
            extract_public_url(
                "lvl=info msg=started url=https://example.ngrok-free.app",
                "ngrok"
            ),
            Some("https://example.ngrok-free.app".into())
        );
        assert_eq!(
            extract_public_url(
                "|  https://random-name.trycloudflare.com  |\u{1b}[0m",
                "cloudflared"
            ),
            Some("https://random-name.trycloudflare.com".into())
        );
        assert_eq!(
            extract_public_url("access your zrok share at https://share.zrok.io/", "zrok"),
            Some("https://share.zrok.io".into())
        );
    }

    #[test]
    fn ignores_urls_for_another_provider() {
        assert_eq!(
            extract_public_url("https://example.ngrok-free.app", "cloudflared"),
            None
        );
    }
}

impl Drop for CompanionProcess {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            if let Err(error) = self.child.kill() {
                tracing::warn!(process = self.name, %error, "failed to stop companion process");
                return;
            }
            let _ = self.child.wait();
        }
    }
}
