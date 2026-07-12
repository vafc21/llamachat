//! Full-screen chat with a local model, in the spirit of Claude Code: a scrolling
//! transcript, a bottom input box, and a `/` slash-command palette. Responses
//! stream token-by-token straight from Ollama's `/api/chat` endpoint over
//! localhost — no shelling out to `ollama run`.

use std::io::{BufRead, BufReader};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use super::action;

/// Who said it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
    /// LlamaChat itself (slash-command output, notices).
    System,
}

pub struct Message {
    pub role: Role,
    pub content: String,
}

/// A slash command shown in the `/` palette.
pub struct SlashCmd {
    pub name: &'static str,
    pub desc: &'static str,
}

pub const SLASH: [SlashCmd; 5] = [
    SlashCmd { name: "help", desc: "Show what you can type" },
    SlashCmd { name: "clear", desc: "Clear this conversation" },
    SlashCmd { name: "retry", desc: "Regenerate the last reply" },
    SlashCmd { name: "models", desc: "Back to the model list" },
    SlashCmd { name: "quit", desc: "Quit LlamaChat" },
];

/// What a submitted slash command asks the app to do.
pub enum ChatAction {
    /// Handled internally (message appended / conversation changed) — stay put.
    None,
    /// Return to the model list.
    Back,
    /// Quit the whole app.
    Quit,
}

/// One streaming reply in progress.
struct Stream {
    rx: Receiver<StreamEvent>,
    started: Instant,
    tokens: usize,
}

enum StreamEvent {
    Token(String),
    Done,
    Error(String),
}

pub struct Chat {
    pub model: String,
    pub messages: Vec<Message>,
    pub input: String,
    pub slash_selected: usize,
    /// Lines scrolled up from the bottom (0 = pinned to newest).
    pub scroll: u16,
    stream: Option<Stream>,
}

impl Chat {
    pub fn new(model: String) -> Self {
        Chat {
            model,
            messages: Vec::new(),
            input: String::new(),
            slash_selected: 0,
            scroll: 0,
            stream: None,
        }
    }

    pub fn is_streaming(&self) -> bool {
        self.stream.is_some()
    }

    /// Seconds the current reply has been streaming (for the spinner).
    pub fn stream_elapsed(&self) -> u64 {
        self.stream.as_ref().map(|s| s.started.elapsed().as_secs()).unwrap_or(0)
    }

    pub fn stream_tokens(&self) -> usize {
        self.stream.as_ref().map(|s| s.tokens).unwrap_or(0)
    }

    // --- slash palette -------------------------------------------------------

    /// The `/`-query if the palette should be open (input is a bare `/word`).
    pub fn slash_query(&self) -> Option<&str> {
        let t = self.input.strip_prefix('/')?;
        if t.contains(char::is_whitespace) {
            None
        } else {
            Some(t)
        }
    }

    /// Commands matching the current `/`-query.
    pub fn slash_matches(&self) -> Vec<&'static SlashCmd> {
        let q = self.slash_query().unwrap_or("").to_ascii_lowercase();
        SLASH.iter().filter(|c| c.name.starts_with(&q)).collect()
    }

    pub fn move_slash(&mut self, delta: i32) {
        let n = self.slash_matches().len().max(1);
        let cur = self.slash_selected as i32;
        self.slash_selected = (cur + delta).rem_euclid(n as i32) as usize;
    }

    /// Tab-complete the highlighted command into the input line.
    pub fn complete_slash(&mut self) {
        let matches = self.slash_matches();
        if let Some(cmd) = matches.get(self.slash_selected).or_else(|| matches.first()) {
            self.input = format!("/{}", cmd.name);
        }
    }

    // --- editing -------------------------------------------------------------

    pub fn push_char(&mut self, c: char) {
        self.input.push(c);
        self.slash_selected = 0;
        self.scroll = 0;
    }

    pub fn backspace(&mut self) {
        self.input.pop();
        self.slash_selected = 0;
    }

    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_add(1);
    }

    pub fn scroll_down(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    // --- submit --------------------------------------------------------------

    /// Handle Enter. Returns what the app should do next.
    pub fn submit(&mut self) -> ChatAction {
        let text = self.input.trim().to_string();
        self.input.clear();
        self.slash_selected = 0;
        if text.is_empty() || self.is_streaming() {
            return ChatAction::None;
        }
        if let Some(cmd) = text.strip_prefix('/') {
            return self.run_command(cmd.split_whitespace().next().unwrap_or(""));
        }
        self.send(text);
        ChatAction::None
    }

    fn run_command(&mut self, name: &str) -> ChatAction {
        match name {
            "quit" | "exit" => ChatAction::Quit,
            "models" | "model" | "back" => ChatAction::Back,
            "clear" => {
                self.messages.clear();
                self.scroll = 0;
                ChatAction::None
            }
            "retry" => {
                // Drop the last assistant reply and re-send the last user turn.
                if matches!(self.messages.last().map(|m| m.role), Some(Role::Assistant)) {
                    self.messages.pop();
                }
                if let Some(last_user) = self
                    .messages
                    .iter()
                    .rposition(|m| m.role == Role::User)
                {
                    let prompt = self.messages[last_user].content.clone();
                    // Trim anything after that user turn, then resend it.
                    self.messages.truncate(last_user);
                    self.send(prompt);
                }
                ChatAction::None
            }
            "help" => {
                self.messages.push(Message {
                    role: Role::System,
                    content: help_text(),
                });
                ChatAction::None
            }
            other => {
                self.messages.push(Message {
                    role: Role::System,
                    content: format!("Unknown command `/{other}`. Type `/help` for the list."),
                });
                ChatAction::None
            }
        }
    }

    fn send(&mut self, text: String) {
        self.messages.push(Message { role: Role::User, content: text });
        // History to send: everything so far (ends with the user turn).
        let history: Vec<(Role, String)> = self
            .messages
            .iter()
            .filter(|m| m.role != Role::System)
            .map(|m| (m.role, m.content.clone()))
            .collect();
        // Placeholder the stream fills in.
        self.messages.push(Message { role: Role::Assistant, content: String::new() });
        self.scroll = 0;
        self.stream = Some(spawn_stream(self.model.clone(), history));
    }

    /// Stop the current reply.
    pub fn interrupt(&mut self) {
        if self.stream.take().is_some() {
            if let Some(last) = self.messages.last_mut() {
                if last.role == Role::Assistant {
                    last.content.push_str("  ⏹");
                }
            }
        }
    }

    /// Drain streamed tokens into the last assistant message. Call each tick.
    pub fn poll(&mut self) {
        let Some(stream) = self.stream.as_mut() else { return };
        let mut done = false;
        let mut err = None;
        loop {
            match stream.rx.try_recv() {
                Ok(StreamEvent::Token(t)) => {
                    stream.tokens += 1;
                    if let Some(last) = self.messages.last_mut() {
                        if last.role == Role::Assistant {
                            last.content.push_str(&t);
                        }
                    }
                }
                Ok(StreamEvent::Done) => {
                    done = true;
                    break;
                }
                Ok(StreamEvent::Error(e)) => {
                    err = Some(e);
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    done = true;
                    break;
                }
            }
        }
        if let Some(e) = err {
            if let Some(last) = self.messages.last_mut() {
                if last.role == Role::Assistant && last.content.is_empty() {
                    self.messages.pop();
                }
            }
            self.messages.push(Message {
                role: Role::System,
                content: format!("⚠ {e}"),
            });
            self.stream = None;
        } else if done {
            // Empty reply (model said nothing) — leave a small note.
            if let Some(last) = self.messages.last() {
                if last.role == Role::Assistant && last.content.is_empty() {
                    self.messages.pop();
                }
            }
            self.stream = None;
        }
    }
}

fn help_text() -> String {
    let mut s = String::from("**Slash commands**\n");
    for c in SLASH.iter() {
        s.push_str(&format!("- `/{}` — {}\n", c.name, c.desc));
    }
    s.push_str("\n**Keys**: `Enter` send · `↑/↓` scroll · `Esc` interrupt / back · `Ctrl-C` quit");
    s
}

/// Spawn the Ollama streaming request on a background thread.
fn spawn_stream(model: String, history: Vec<(Role, String)>) -> Stream {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        // Make sure the daemon is up first (chat is useless without it).
        if !action::ollama_reachable() {
            let _ = std::process::Command::new("ollama")
                .arg("serve")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
            let mut up = false;
            for _ in 0..30 {
                std::thread::sleep(Duration::from_millis(200));
                if action::ollama_reachable() {
                    up = true;
                    break;
                }
            }
            if !up {
                let _ = tx.send(StreamEvent::Error(
                    "Couldn't reach Ollama. Try `ollama serve` in another terminal.".into(),
                ));
                return;
            }
        }

        let messages: Vec<serde_json::Value> = history
            .iter()
            .map(|(role, content)| {
                let r = match role {
                    Role::User => "user",
                    Role::Assistant => "assistant",
                    Role::System => "system",
                };
                serde_json::json!({ "role": r, "content": content })
            })
            .collect();
        let body = serde_json::json!({
            "model": model,
            "messages": messages,
            "stream": true,
        })
        .to_string();

        let resp = ureq::post("http://127.0.0.1:11434/api/chat")
            .set("Content-Type", "application/json")
            .send_string(&body);
        let reader = match resp {
            Ok(r) => BufReader::new(r.into_reader()),
            Err(e) => {
                let _ = tx.send(StreamEvent::Error(format!("request failed: {e}")));
                return;
            }
        };

        for line in reader.lines() {
            let Ok(line) = line else { break };
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            match serde_json::from_str::<ChatChunk>(line) {
                Ok(chunk) => {
                    if let Some(msg) = chunk.message {
                        if !msg.content.is_empty() {
                            if tx.send(StreamEvent::Token(msg.content)).is_err() {
                                return; // UI dropped the receiver (interrupted)
                            }
                        }
                    }
                    if chunk.error.is_some() {
                        let _ = tx.send(StreamEvent::Error(chunk.error.unwrap()));
                        return;
                    }
                    if chunk.done {
                        break;
                    }
                }
                Err(_) => {
                    // A non-JSON line (e.g. an HTML error page) — surface it once.
                    let _ = tx.send(StreamEvent::Error(line.chars().take(200).collect()));
                    return;
                }
            }
        }
        let _ = tx.send(StreamEvent::Done);
    });

    Stream { rx, started: Instant::now(), tokens: 0 }
}

#[derive(serde::Deserialize)]
struct ChatChunk {
    message: Option<ChatMsg>,
    #[serde(default)]
    done: bool,
    #[serde(default)]
    error: Option<String>,
}

#[derive(serde::Deserialize)]
struct ChatMsg {
    #[serde(default)]
    content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slash_palette_filters_by_prefix() {
        let mut c = Chat::new("m".into());
        c.input = "/cl".into();
        assert_eq!(c.slash_query(), Some("cl"));
        let names: Vec<_> = c.slash_matches().iter().map(|x| x.name).collect();
        assert!(names.contains(&"clear"));
        assert!(!names.contains(&"help"));

        // A space after the command closes the palette.
        c.input = "/clear now".into();
        assert_eq!(c.slash_query(), None);
    }

    #[test]
    fn commands_route_correctly() {
        let mut c = Chat::new("m".into());
        c.messages.push(Message { role: Role::User, content: "hi".into() });

        c.input = "/clear".into();
        assert!(matches!(c.submit(), ChatAction::None));
        assert!(c.messages.is_empty());

        c.input = "/help".into();
        c.submit();
        assert!(c.messages.iter().any(|m| m.role == Role::System));

        c.input = "/models".into();
        assert!(matches!(c.submit(), ChatAction::Back));

        c.input = "/quit".into();
        assert!(matches!(c.submit(), ChatAction::Quit));
    }

    #[test]
    fn empty_or_streaming_submit_is_noop() {
        let mut c = Chat::new("m".into());
        c.input = "   ".into();
        assert!(matches!(c.submit(), ChatAction::None));
        assert!(c.messages.is_empty());
    }
}

