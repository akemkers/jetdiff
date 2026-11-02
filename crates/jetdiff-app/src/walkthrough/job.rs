//! One `claude -p` call, without tools, MCP servers, settings or skills: the model only reads the
//! digest and answers in JSON. Without those flags a whole Claude Code setup rides along, and a small
//! change costs a hundred times more.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use super::cache::cache_key;
use super::digest::Digest;
use super::prompt::{prompt, schema, system};

/// How often the job looks at the process and the cancel flag.
const POLL: Duration = Duration::from_millis(100);
/// Opus can take minutes for a large change.
const BASE_TIMEOUT: Duration = Duration::from_secs(300);
const TIMEOUT_PER_HUNK: Duration = Duration::from_secs(1);
const MAX_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// Who writes a walkthrough, from the settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Writer {
    /// The program and the arguments before jetdiff's own, like `claude --team x`.
    pub command: Vec<String>,
    pub model: String,
    pub language: String,
}

impl Writer {
    /// The cache key for a walkthrough of `digest`.
    pub fn key(&self, digest: &Digest) -> String {
        cache_key(&self.model, &self.language, digest.text())
    }
}

/// Everything the call needs, so it can run on another thread.
pub struct Job {
    pub key: String,
    command: Vec<String>,
    model: String,
    system: String,
    prompt: String,
    timeout: Duration,
    cancel: Arc<AtomicBool>,
}

impl Job {
    /// `what` says what was read, like "Branch feature against main".
    pub fn new(digest: &Digest, what: &str, writer: &Writer) -> Self {
        let hunks = u32::try_from(digest.hunk_count()).unwrap_or(u32::MAX);
        Self {
            key: writer.key(digest),
            command: writer.command.clone(),
            model: writer.model.clone(),
            system: system(&writer.language),
            prompt: prompt(digest, what),
            timeout: (BASE_TIMEOUT + TIMEOUT_PER_HUNK * hunks).min(MAX_TIMEOUT),
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Stops `run` from another thread.
    pub fn canceller(&self) -> Arc<AtomicBool> {
        self.cancel.clone()
    }

    /// Blocks until the model has answered. The answer is what `claude` wrote, for
    /// `Walkthrough::from_answer` and the cache.
    pub fn run(self) -> Result<String, String> {
        let (program, arguments) = self
            .command
            .split_first()
            .ok_or("The walkthrough's command is empty")?;
        let mut child = Command::new(program)
            .args(arguments)
            .args(["-p", "--output-format", "json", "--tools", ""])
            .args(["--no-session-persistence", "--strict-mcp-config"])
            .args(["--setting-sources", "", "--disable-slash-commands"])
            .args(["--model", &self.model])
            .args(["--json-schema", &schema().to_string()])
            .args(["--system-prompt", &self.system])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => format!("{program} is not on the PATH"),
                _ => format!("Can't start {program}: {e}"),
            })?;
        let mut stdin = child.stdin.take().expect("stdin is piped");
        let prompt = self.prompt;
        thread::spawn(move || stdin.write_all(prompt.as_bytes()));
        let stdout = read_in_background(child.stdout.take().expect("stdout is piped"));
        let stderr = read_in_background(child.stderr.take().expect("stderr is piped"));
        let started = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                break status;
            }
            if self.cancel.load(Ordering::Relaxed) {
                let _ = child.kill();
                return Err("Cancelled".into());
            }
            if started.elapsed() > self.timeout {
                let _ = child.kill();
                return Err(format!(
                    "The model didn't answer within {} s",
                    self.timeout.as_secs()
                ));
            }
            thread::sleep(POLL);
        };
        let stdout = stdout.join().unwrap_or_default();
        if stdout.trim().is_empty() {
            let stderr = stderr.join().unwrap_or_default();
            let reason = stderr.lines().find(|line| !line.trim().is_empty());
            return Err(
                reason.map_or_else(|| format!("claude stopped with {status}"), str::to_owned)
            );
        }
        Ok(stdout)
    }
}

fn read_in_background(mut pipe: impl Read + Send + 'static) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let mut text = String::new();
        let _ = pipe.read_to_string(&mut text);
        text
    })
}
