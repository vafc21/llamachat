//! Model actions: pull a model with Ollama (streaming progress) and hand off to
//! an interactive `ollama run` chat. This is what makes the Models tab do
//! something when you press Enter, instead of just showing information.

use std::collections::{HashSet, VecDeque};
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How many progress lines we keep for the overlay.
const LOG_CAP: usize = 40;

/// A running `ollama pull`, streaming its progress into `log`.
pub struct PullJob {
    pub tag: String,
    pub display: String,
    pub log: Arc<Mutex<VecDeque<String>>>,
    pub rx: Receiver<Result<(), String>>,
    pub started: Instant,
}

impl PullJob {
    /// The lines to show right now (cheap clone of the tail).
    pub fn lines(&self) -> Vec<String> {
        self.log
            .lock()
            .map(|g| g.iter().cloned().collect())
            .unwrap_or_default()
    }
}

/// Is the `ollama` CLI installed at all?
pub fn ollama_installed() -> bool {
    Command::new("ollama")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Can we reach a running Ollama daemon? (`ollama list` needs the server.)
pub fn ollama_reachable() -> bool {
    Command::new("ollama")
        .arg("list")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// The set of model tags already pulled locally (first column of `ollama list`).
pub fn installed_models() -> HashSet<String> {
    let mut set = HashSet::new();
    if let Ok(out) = Command::new("ollama").arg("list").output() {
        if out.status.success() {
            for line in String::from_utf8_lossy(&out.stdout).lines().skip(1) {
                if let Some(name) = line.split_whitespace().next() {
                    if !name.is_empty() {
                        set.insert(name.to_string());
                    }
                }
            }
        }
    }
    set
}

/// Start `ollama pull <tag>` on a background thread, auto-starting the daemon if
/// it isn't up. Progress lines land in the returned job's `log`; the final
/// `Ok(())`/`Err(msg)` arrives on its `rx`.
pub fn start_pull(tag: &str, display: &str) -> PullJob {
    let log: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));
    let (tx, rx) = mpsc::channel();
    let tag_owned = tag.to_string();
    let log_bg = log.clone();

    std::thread::spawn(move || {
        // Make sure the daemon is reachable; start it ourselves if not.
        if !ollama_reachable() {
            add_line(&log_bg, "Ollama isn't running — starting it…".into());
            let _ = Command::new("ollama")
                .arg("serve")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();
            let mut up = false;
            for _ in 0..30 {
                std::thread::sleep(Duration::from_millis(200));
                if ollama_reachable() {
                    up = true;
                    break;
                }
            }
            if !up {
                let _ = tx.send(Err(
                    "Couldn't reach Ollama. Try running `ollama serve` in another terminal.".into(),
                ));
                return;
            }
            add_line(&log_bg, "Ollama is up.".into());
        }

        add_line(&log_bg, format!("Pulling {tag_owned}…"));
        let mut child = match Command::new("ollama")
            .arg("pull")
            .arg(&tag_owned)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(Err(format!("Failed to launch ollama: {e}")));
                return;
            }
        };

        // Ollama writes its progress bar to stderr; read both streams.
        let mut handles = Vec::new();
        if let Some(out) = child.stdout.take() {
            handles.push(spawn_reader(out, log_bg.clone()));
        }
        if let Some(err) = child.stderr.take() {
            handles.push(spawn_reader(err, log_bg.clone()));
        }

        let status = child.wait();
        for h in handles {
            let _ = h.join();
        }
        match status {
            Ok(s) if s.success() => {
                add_line(&log_bg, "Done ✓".into());
                let _ = tx.send(Ok(()));
            }
            Ok(_) => {
                let last = log_bg
                    .lock()
                    .ok()
                    .and_then(|g| g.back().cloned())
                    .unwrap_or_else(|| "pull failed".into());
                let _ = tx.send(Err(last));
            }
            Err(e) => {
                let _ = tx.send(Err(format!("pull error: {e}")));
            }
        }
    });

    PullJob {
        tag: tag.to_string(),
        display: display.to_string(),
        log,
        rx,
        started: Instant::now(),
    }
}

/// Read a child stream and feed each rendered line into `log`, splitting on both
/// `\n` and `\r` so Ollama's carriage-return progress bar updates in place.
fn spawn_reader(
    stream: impl Read + Send + 'static,
    log: Arc<Mutex<VecDeque<String>>>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut reader = std::io::BufReader::new(stream);
        let mut buf: Vec<u8> = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            match reader.read(&mut byte) {
                Ok(0) => break,
                Ok(_) => match byte[0] {
                    b'\n' | b'\r' => flush(&log, &mut buf),
                    b => buf.push(b),
                },
                Err(_) => break,
            }
        }
        flush(&log, &mut buf);
    })
}

fn flush(log: &Arc<Mutex<VecDeque<String>>>, buf: &mut Vec<u8>) {
    let s = String::from_utf8_lossy(buf).trim().to_string();
    buf.clear();
    if s.is_empty() {
        return;
    }
    add_line(log, s);
}

/// Append a line, but collapse consecutive progress updates (same prefix + a
/// percentage) so the download bar refreshes in place instead of scrolling.
fn add_line(log: &Arc<Mutex<VecDeque<String>>>, line: String) {
    let Ok(mut g) = log.lock() else { return };
    let collapse = g
        .back()
        .map(|last| line.contains('%') && common_prefix(last, &line) >= 8)
        .unwrap_or(false);
    if collapse {
        if let Some(last) = g.back_mut() {
            *last = line;
            return;
        }
    }
    g.push_back(line);
    while g.len() > LOG_CAP {
        g.pop_front();
    }
}

fn common_prefix(a: &str, b: &str) -> usize {
    a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count()
}
