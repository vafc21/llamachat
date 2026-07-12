//! Full-screen chat with a local model, in the spirit of Claude Code: a scrolling
//! transcript, a bottom input box, a `/` slash-command palette, tool use (run
//! commands, read/write files) and a Claude-Code-style permission prompt before
//! anything touches the machine. Responses stream token-by-token straight from
//! Ollama's `/api/chat`.

use std::io::{BufRead, BufReader};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::Instant;

use llamachat_core::tools::{ToolRegistry, ToolRequest, ToolResult};

use super::tools;

/// Max tool calls per user turn, to stop runaway agent loops.
const MAX_ROUNDS: u32 = 8;

/// Who said it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
    /// LlamaChat itself (slash-command output, notices).
    System,
    /// A tool result — shown as a result block, sent to the model as context.
    Tool,
}

pub struct Message {
    pub role: Role,
    pub content: String,
    /// Tool name, for `Role::Tool` messages.
    pub tool: Option<String>,
}

impl Message {
    fn user(s: impl Into<String>) -> Self {
        Message { role: Role::User, content: s.into(), tool: None }
    }
    fn assistant(s: impl Into<String>) -> Self {
        Message { role: Role::Assistant, content: s.into(), tool: None }
    }
    fn system(s: impl Into<String>) -> Self {
        Message { role: Role::System, content: s.into(), tool: None }
    }
    fn tool(name: impl Into<String>, s: impl Into<String>) -> Self {
        Message { role: Role::Tool, content: s.into(), tool: Some(name.into()) }
    }
}

/// A slash command shown in the `/` palette.
pub struct SlashCmd {
    pub name: &'static str,
    pub desc: &'static str,
}

pub const SLASH: [SlashCmd; 11] = [
    SlashCmd { name: "help", desc: "Show commands and keys" },
    SlashCmd { name: "commands", desc: "List all slash commands" },
    SlashCmd { name: "tools", desc: "Show tools the model can use (on/off)" },
    SlashCmd { name: "permissions", desc: "View or change tool permissions / mode" },
    SlashCmd { name: "effort", desc: "Set reasoning effort (low/medium/high/max)" },
    SlashCmd { name: "clear", desc: "Clear this conversation" },
    SlashCmd { name: "retry", desc: "Regenerate the last reply" },
    SlashCmd { name: "model", desc: "Back to the model list" },
    SlashCmd { name: "status", desc: "Show session status" },
    SlashCmd { name: "quit", desc: "Quit LlamaChat" },
    SlashCmd { name: "mode", desc: "Set permission mode (same as Shift+Tab)" },
];

/// What a submitted slash command asks the app to do.
pub enum ChatAction {
    None,
    Back,
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

    // Tool agent state.
    registry: Arc<ToolRegistry>,
    pub tools_on: bool,
    pub perms: tools::Perms,
    /// A tool call awaiting the user's permission decision.
    pending: Option<ToolRequest>,
    /// A tool currently executing.
    tool_rx: Option<Receiver<ToolResult>>,
    running: Option<String>,
    rounds: u32,

    /// Claude-Code-style permission mode (cycled with Shift+Tab).
    pub mode: tools::PermMode,
    /// How hard the model should reason.
    pub effort: tools::Effort,
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
            registry: tools::build_registry(),
            tools_on: true,
            perms: tools::Perms::default(),
            pending: None,
            tool_rx: None,
            running: None,
            rounds: 0,
            mode: tools::PermMode::Manual,
            effort: tools::Effort::Medium,
        }
    }

    /// Cycle the permission mode (Shift+Tab).
    pub fn cycle_mode(&mut self) {
        self.mode = self.mode.next();
        self.messages.push(Message::system(format!(
            "Mode → {}  ({})",
            self.mode.badge(),
            self.mode.explain()
        )));
    }

    fn mode_explanation(&self) -> &'static str {
        self.mode.explain()
    }

    pub fn is_streaming(&self) -> bool {
        self.stream.is_some()
    }

    pub fn is_tool_running(&self) -> bool {
        self.tool_rx.is_some()
    }

    /// A tool call is waiting for the user's yes/no.
    pub fn pending(&self) -> Option<&ToolRequest> {
        self.pending.as_ref()
    }

    /// Whether the current permission mode should auto-approve this tool.
    fn mode_allows(&self, req: &ToolRequest) -> bool {
        match self.mode {
            tools::PermMode::Manual => false,
            tools::PermMode::Plan => false, // plan is read-only — deny all mutations
            tools::PermMode::AcceptEdits => {
                // Auto-approve file edits + common filesystem commands.
                matches!(req.name.as_str(), "file") || {
                    if req.name == "shell" {
                        let cmd = req.args.get("command").and_then(|v| v.as_str()).unwrap_or("");
                        tools::PermMode::is_safe_cmd(cmd)
                    } else {
                        false
                    }
                }
            }
            tools::PermMode::Auto | tools::PermMode::Bypass => true,
        }
    }

    /// Busy = can't accept a new user message right now.
    pub fn busy(&self) -> bool {
        self.stream.is_some() || self.tool_rx.is_some() || self.pending.is_some()
    }

    pub fn running_tool(&self) -> Option<&str> {
        self.running.as_deref()
    }

    pub fn stream_elapsed(&self) -> u64 {
        self.stream.as_ref().map(|s| s.started.elapsed().as_secs()).unwrap_or(0)
    }

    pub fn stream_tokens(&self) -> usize {
        self.stream.as_ref().map(|s| s.tokens).unwrap_or(0)
    }

    // --- slash palette -------------------------------------------------------

    pub fn slash_query(&self) -> Option<&str> {
        let t = self.input.strip_prefix('/')?;
        if t.contains(char::is_whitespace) {
            None
        } else {
            Some(t)
        }
    }

    pub fn slash_matches(&self) -> Vec<&'static SlashCmd> {
        let q = self.slash_query().unwrap_or("").to_ascii_lowercase();
        SLASH.iter().filter(|c| c.name.starts_with(&q)).collect()
    }

    pub fn move_slash(&mut self, delta: i32) {
        let n = self.slash_matches().len().max(1);
        let cur = self.slash_selected as i32;
        self.slash_selected = (cur + delta).rem_euclid(n as i32) as usize;
    }

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

    // --- permission decisions ------------------------------------------------

    pub fn allow_once(&mut self) {
        if let Some(req) = self.pending.take() {
            self.start_tool(req);
        }
    }

    pub fn allow_always(&mut self) {
        if let Some(req) = self.pending.take() {
            self.perms.always.insert(req.name.clone());
            self.start_tool(req);
        }
    }

    pub fn deny(&mut self) {
        if let Some(req) = self.pending.take() {
            self.messages.push(Message::system(format!("Denied `{}`.", req.name)));
            self.messages.push(Message::tool(req.name, "Permission denied by the user."));
            self.begin_turn();
        }
    }

    // --- submit --------------------------------------------------------------

    pub fn submit(&mut self) -> ChatAction {
        let text = self.input.trim().to_string();
        self.input.clear();
        self.slash_selected = 0;
        if text.is_empty() || self.busy() {
            return ChatAction::None;
        }
        if let Some(rest) = text.strip_prefix('/') {
            return self.run_command(rest);
        }
        self.messages.push(Message::user(text));
        self.rounds = 0;
        self.begin_turn();
        ChatAction::None
    }

    fn run_command(&mut self, rest: &str) -> ChatAction {
        let mut parts = rest.split_whitespace();
        let name = parts.next().unwrap_or("");
        let a1 = parts.next();
        let a2 = parts.next();
        match name {
            "quit" | "exit" => return ChatAction::Quit,
            "models" | "model" | "back" => return ChatAction::Back,
            "clear" => {
                self.messages.clear();
                self.scroll = 0;
            }
            "retry" => {
                if matches!(self.messages.last().map(|m| m.role), Some(Role::Assistant)) {
                    self.messages.pop();
                }
                if let Some(i) = self.messages.iter().rposition(|m| m.role == Role::User) {
                    let prompt = self.messages[i].content.clone();
                    self.messages.truncate(i);
                    self.messages.push(Message::user(prompt));
                    self.rounds = 0;
                    self.begin_turn();
                }
            }
            "help" | "commands" => self.messages.push(Message::system(help_text())),
            "tools" => match a1 {
                Some("off") => {
                    self.tools_on = false;
                    self.messages.push(Message::system("Tools **disabled** — the model can't run anything."));
                }
                Some("on") => {
                    self.tools_on = true;
                    self.messages.push(Message::system("Tools **enabled**."));
                }
                _ => self.messages.push(Message::system(self.tools_text())),
            },
            "permissions" | "perms" => match (a1, a2) {
                (Some("mode"), Some(m)) => {
                    if let Some(new_mode) = tools::PermMode::from_label(m) {
                        self.mode = new_mode;
                        self.messages.push(Message::system(format!(
                            "Mode → {}  ({})",
                            self.mode.badge(),
                            self.mode_explanation()
                        )));
                    } else {
                        let modes: Vec<_> =
                            tools::PermMode::ALL.iter().map(|m| m.label()).collect();
                        self.messages.push(Message::system(format!(
                            "Unknown mode `{m}`. Try: {}",
                            modes.join(", ")
                        )));
                    }
                }
                (Some("allow"), Some(tool)) => {
                    self.perms.always.insert(tool.to_string());
                    self.messages.push(Message::system(format!("Always allowing `{tool}`.")));
                }
                (Some("allow-all"), _) | (Some("bypass"), _) => {
                    self.mode = tools::PermMode::Bypass;
                    self.messages.push(Message::system(
                        "**Bypass mode ON** — all tools auto-approved, no prompts.",
                    ));
                }
                (Some("reset"), _) => {
                    self.perms.always.clear();
                    self.mode = tools::PermMode::Manual;
                    self.messages
                        .push(Message::system("Permissions reset — manual mode, asking for each tool."));
                }
                _ => self.messages.push(Message::system(self.permissions_text())),
            },
            "effort" => match a1 {
                Some(m) => {
                    if let Some(e) = tools::Effort::from_label(m) {
                        self.effort = e;
                        self.messages.push(Message::system(format!(
                            "Effort → {}  ·  {}",
                            self.effort.badge(),
                            self.effort.system_hint()
                        )));
                    } else {
                        let levels: Vec<_> = tools::Effort::ALL.iter().map(|e| e.label()).collect();
                        self.messages.push(Message::system(format!(
                            "Unknown effort `{m}`. Try: {}",
                            levels.join(", ")
                        )));
                    }
                }
                None => {
                    self.messages.push(Message::system(format!(
                        "Effort: **{}**  ·  {}\n\n`/effort <low|medium|high|max>`",
                        self.effort.label(),
                        self.effort.system_hint()
                    )));
                }
            },
            "mode" => {
                let new_mode = match a1 {
                    Some(m) => tools::PermMode::from_label(m),
                    None => Some(self.mode.next()),
                };
                if let Some(new_mode) = new_mode {
                    self.mode = new_mode;
                    self.messages.push(Message::system(format!(
                        "Mode → {}  ({})",
                        self.mode.badge(),
                        self.mode_explanation()
                    )));
                }
            }
            "status" => self.messages.push(Message::system(self.status_text())),
            other => self
                .messages
                .push(Message::system(format!("Unknown command `/{other}`. Try `/help`."))),
        }
        ChatAction::None
    }

    // --- the agent loop ------------------------------------------------------

    /// Build the request from the current transcript and stream one turn.
    fn begin_turn(&mut self) {
        let mut msgs: Vec<serde_json::Value> = Vec::new();
        if self.tools_on {
            msgs.push(serde_json::json!({ "role": "system", "content": self.system_prompt() }));
        }
        for m in &self.messages {
            let (role, content) = match m.role {
                Role::User => ("user", m.content.clone()),
                Role::Assistant => ("assistant", m.content.clone()),
                Role::Tool => (
                    "user",
                    format!(
                        "Tool result for {}:\n{}",
                        m.tool.as_deref().unwrap_or("tool"),
                        m.content
                    ),
                ),
                Role::System => continue, // UI-only notices
            };
            if !content.is_empty() {
                msgs.push(serde_json::json!({ "role": role, "content": content }));
            }
        }
        self.messages.push(Message::assistant(String::new()));
        self.scroll = 0;
        self.stream = Some(spawn_stream(self.model.clone(), msgs));
    }

    fn start_tool(&mut self, req: ToolRequest) {
        self.running = Some(req.name.clone());
        self.tool_rx = Some(tools::run(self.registry.clone(), req));
    }

    /// After a streamed turn completes, look for a tool call and act on it.
    pub(crate) fn after_turn(&mut self) {
        if !self.tools_on {
            return;
        }
        let text = match self.messages.last() {
            Some(m) if m.role == Role::Assistant => m.content.clone(),
            _ => return,
        };
        let Some(parsed) = tools::extract_tool_call(&text) else { return };
        if self.rounds >= MAX_ROUNDS {
            self.messages
                .push(Message::system(format!("Stopped after {MAX_ROUNDS} tool calls.")));
            return;
        }
        self.rounds += 1;
        let req = parsed.req;

        // Check the permission mode first, then individual allow rules.
        if self.mode == tools::PermMode::Plan {
            // Plan mode — deny all mutations.
            self.messages
                .push(Message::system(format!("Plan mode: auto-denied `{}` (read-only).", req.name)));
            self.messages.push(Message::tool(&req.name, "In plan mode — reads only."));
            // Feed the denial back so the model can mention what it *would* do.
            self.begin_turn();
            return;
        }

        let auto = self.mode_allows(&req)
            || self.perms.allowed(&req.name)
            || !self.registry.needs_approval(&req.name);
        if auto {
            self.start_tool(req);
        } else {
            self.pending = Some(req);
        }
    }

    pub fn interrupt(&mut self) {
        let was_busy = self.busy();
        self.stream = None;
        self.pending = None;
        self.tool_rx = None;
        self.running = None;
        if was_busy {
            if let Some(last) = self.messages.last_mut() {
                if last.role == Role::Assistant && last.content.is_empty() {
                    last.content.push_str("(stopped)");
                }
            }
        }
    }

    /// Pump streaming + tool jobs; call each tick.
    pub fn poll(&mut self) {
        // 1. Streaming reply.
        if let Some(stream) = self.stream.as_mut() {
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
                if matches!(self.messages.last().map(|m| (m.role, m.content.is_empty())), Some((Role::Assistant, true))) {
                    self.messages.pop();
                }
                self.messages.push(Message::system(format!("⚠ {e}")));
                self.stream = None;
            } else if done {
                if matches!(self.messages.last().map(|m| (m.role, m.content.is_empty())), Some((Role::Assistant, true))) {
                    self.messages.pop();
                }
                self.stream = None;
                self.after_turn();
            }
        }

        // 2. Tool execution result.
        if let Some(rx) = self.tool_rx.as_ref() {
            match rx.try_recv() {
                Ok(result) => {
                    let name = self.running.take().unwrap_or_else(|| "tool".into());
                    self.tool_rx = None;
                    self.messages.push(Message::tool(name, tools::summarize(&result)));
                    self.begin_turn();
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.tool_rx = None;
                    self.running = None;
                    self.messages.push(Message::system("Tool crashed."));
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
    }

    // --- help / status text --------------------------------------------------

    fn system_prompt(&self) -> String {
        let mut s = String::from(
            "You are LlamaChat, a helpful assistant running in the user's terminal with access to their machine through tools.\n\n",
        );
        // Effort level as a direct style instruction.
        s.push_str(&format!("**Response style**: {}\n\n", self.effort.system_hint()));
        s.push_str(&self.registry.system_prompt());
        s.push_str(
            "\nRules:\n- Call a tool ONLY when you need it to answer; otherwise just reply.\n- To call a tool, output ONLY the JSON object, nothing else.\n- After a tool result, either call another tool or give a concise final answer in plain language.\n",
        );
        s
    }

    fn tools_text(&self) -> String {
        let mut s = format!(
            "**Tools** ({})\n",
            if self.tools_on { "enabled" } else { "disabled — /tools on" }
        );
        for t in self.registry.list_tools() {
            let safety = match t.safety {
                llamachat_core::tools::ToolSafety::ReadOnly => "read-only, auto",
                llamachat_core::tools::ToolSafety::Write => "write, auto",
                llamachat_core::tools::ToolSafety::Destructive => "needs approval",
            };
            s.push_str(&format!("- `{}` ({}) — {}\n", t.name, safety, t.description));
        }
        s.push_str("\nToggle with `/tools on` or `/tools off`.");
        s
    }

    fn permissions_text(&self) -> String {
        let mut s = String::from("**Permissions**\n");
        if self.perms.allow_all {
            s.push_str("- Bypass mode: **ON** (all tools auto-approved)\n");
        } else {
            s.push_str("- Bypass mode: off — destructive tools ask first\n");
        }
        if self.perms.always.is_empty() {
            s.push_str("- Always-allowed: none\n");
        } else {
            let mut v: Vec<_> = self.perms.always.iter().cloned().collect();
            v.sort();
            s.push_str(&format!("- Always-allowed: {}\n", v.join(", ")));
        }
        s.push_str("\n`/permissions allow <tool>` · `/permissions allow-all` · `/permissions reset`");
        s
    }

    fn status_text(&self) -> String {
        format!(
            "**Status**\n- Model: `{}`\n- Mode: {} ({})\n- Effort: {}\n- Tools: {}\n- Messages: {}",
            self.model,
            self.mode.badge(),
            self.mode_explanation(),
            self.effort.label(),
            if self.tools_on { "on" } else { "off" },
            self.messages
                .iter()
                .filter(|m| matches!(m.role, Role::User | Role::Assistant))
                .count(),
        )
    }
}

fn help_text() -> String {
    let mut s = String::from("**Slash commands**\n");
    for c in SLASH.iter() {
        s.push_str(&format!("- `/{}` — {}\n", c.name, c.desc));
    }
    s.push_str("\n**Keys**: `Enter` send · `↑/↓` scroll · `Esc` interrupt / back · `Ctrl-C` quit\n");
    s.push_str("**Shift+Tab** cycle permission mode: Manual → AcceptEdits → Plan → Auto → Bypass\n");
    s.push_str("**Tool prompts**: `a` allow once · `A` always allow · `d` deny");
    s
}

/// Spawn the Ollama streaming request on a background thread.
fn spawn_stream(model: String, messages: Vec<serde_json::Value>) -> Stream {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        if !tools::ollama_up() {
            let _ = tx.send(StreamEvent::Error(
                "Couldn't reach Ollama. Try `ollama serve` in another terminal.".into(),
            ));
            return;
        }
        let body = serde_json::json!({ "model": model, "messages": messages, "stream": true }).to_string();
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
                        if !msg.content.is_empty() && tx.send(StreamEvent::Token(msg.content)).is_err() {
                            return;
                        }
                    }
                    if let Some(e) = chunk.error {
                        let _ = tx.send(StreamEvent::Error(e));
                        return;
                    }
                    if chunk.done {
                        break;
                    }
                }
                Err(_) => {
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
        c.input = "/per".into();
        assert_eq!(c.slash_query(), Some("per"));
        let names: Vec<_> = c.slash_matches().iter().map(|x| x.name).collect();
        assert!(names.contains(&"permissions"));
        assert!(!names.contains(&"help"));
        c.input = "/clear now".into();
        assert_eq!(c.slash_query(), None);
    }

    #[test]
    fn commands_route_correctly() {
        let mut c = Chat::new("m".into());
        c.messages.push(Message::user("hi"));
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
    fn permissions_command_updates_state() {
        let mut c = Chat::new("m".into());
        c.input = "/permissions allow shell".into();
        c.submit();
        assert!(c.perms.allowed("shell"));
        c.input = "/permissions reset".into();
        c.submit();
        assert!(!c.perms.allowed("shell"));
        assert_eq!(c.mode, tools::PermMode::Manual);
        c.input = "/permissions allow-all".into();
        c.submit();
        assert_eq!(c.mode, tools::PermMode::Bypass);
    }

    #[test]
    fn tool_call_needs_approval_for_shell() {
        // A shell call is destructive → should land in `pending`, not auto-run.
        let mut c = Chat::new("m".into());
        c.messages.push(Message::user("list files"));
        c.messages
            .push(Message::assistant("{\"tool\": \"shell\", \"args\": {\"command\": \"ls\"}}"));
        c.rounds = 0;
        c.after_turn();
        assert!(c.pending().is_some());
        assert_eq!(c.pending().unwrap().name, "shell");
    }
}
