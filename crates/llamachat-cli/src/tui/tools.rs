//! Tool wiring for the chat agent: build the tool registry, extract tool calls
//! from model output, and run them off-thread. Uses the core `ToolRegistry`
//! (shell / file / process), which already defines each tool's schema and
//! safety level; this module just adapts it to the chat loop and adds a
//! Claude-Code-style permission layer on top.

use std::collections::HashSet;
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::Duration;

use llamachat_core::tools::{
    FilesystemTool, ProcessTool, ShellTool, ToolLimits, ToolRegistry, ToolRequest, ToolResult,
};

use super::action;

/// Ensure the Ollama daemon is reachable, starting it if needed. Returns false
/// only if it still can't be reached after a short wait.
pub fn ollama_up() -> bool {
    if action::ollama_reachable() {
        return true;
    }
    let _ = std::process::Command::new("ollama")
        .arg("serve")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    for _ in 0..30 {
        std::thread::sleep(Duration::from_millis(200));
        if action::ollama_reachable() {
            return true;
        }
    }
    false
}

/// Build the registry the chat agent exposes. `destructive_allowed` is true
/// because *we* gate execution through the permission prompt, not the registry.
pub fn build_registry() -> Arc<ToolRegistry> {
    let mut r = ToolRegistry::new(ToolLimits::default(), true);
    r.register(Box::new(ShellTool::new(ToolLimits::default())));
    r.register(Box::new(FilesystemTool::new(ToolLimits::default())));
    r.register(Box::new(ProcessTool::new(ToolLimits::default())));
    Arc::new(r)
}

/// A parsed tool call plus the byte span it occupied in the model's text, so the
/// raw JSON can be hidden from the transcript.
pub struct ParsedCall {
    pub req: ToolRequest,
    pub start: usize,
    pub end: usize,
}

/// Find the first `{"tool": "...", "args": {...}}` object in `text` (optionally
/// fenced) and return it with its span. Mirrors the core agent's format.
pub fn extract_tool_call(text: &str) -> Option<ParsedCall> {
    // Locate the opening of a tool object, tolerating ```json fences.
    let key = "\"tool\"";
    let key_pos = text.find(key)?;
    // Walk back to the '{' that owns this key.
    let brace = text[..key_pos].rfind('{')?;
    let slice = &text[brace..];
    let mut depth = 0i32;
    let mut end_rel = 0;
    let mut in_str = false;
    let mut esc = false;
    for (i, ch) in slice.char_indices() {
        if in_str {
            if esc {
                esc = false;
            } else if ch == '\\' {
                esc = true;
            } else if ch == '"' {
                in_str = false;
            }
            continue;
        }
        match ch {
            '"' => in_str = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end_rel = i + 1;
                    break;
                }
            }
            _ => {}
        }
    }
    if end_rel == 0 {
        return None;
    }
    let json = &slice[..end_rel];
    let val: serde_json::Value = serde_json::from_str(json).ok()?;
    let name = val.get("tool")?.as_str()?.to_string();
    let args = val.get("args").cloned().unwrap_or(serde_json::json!({}));
    Some(ParsedCall {
        req: ToolRequest { name, args },
        start: brace,
        end: brace + end_rel,
    })
}

/// A one-line, human summary of what a tool call will do (for the transcript /
/// permission prompt), e.g. `shell · ls -la`.
pub fn describe(req: &ToolRequest) -> String {
    let a = &req.args;
    let detail = match req.name.as_str() {
        "shell" => a.get("command").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        "file" => {
            let action = a.get("action").and_then(|v| v.as_str()).unwrap_or("");
            let path = a.get("path").and_then(|v| v.as_str()).unwrap_or("");
            format!("{action} {path}").trim().to_string()
        }
        "process" => {
            let action = a.get("action").and_then(|v| v.as_str()).unwrap_or("");
            let c = a.get("command").and_then(|v| v.as_str()).unwrap_or("");
            format!("{action} {c}").trim().to_string()
        }
        _ => a.to_string(),
    };
    if detail.is_empty() {
        req.name.clone()
    } else {
        format!("{} · {}", req.name, detail)
    }
}

/// Condense a tool result for feeding back to the model and showing in the UI.
pub fn summarize(result: &ToolResult) -> String {
    if let Some(out) = &result.output {
        let out = out.trim();
        if out.is_empty() {
            "(no output)".into()
        } else {
            out.to_string()
        }
    } else if let Some(err) = &result.error {
        format!("error: {err}")
    } else {
        "(no output)".into()
    }
}

/// Run a tool off-thread; the result comes back on the returned channel.
pub fn run(registry: Arc<ToolRegistry>, req: ToolRequest) -> Receiver<ToolResult> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(registry.execute(&req));
    });
    rx
}

/// Claude-Code-style permission mode. Controls whether the chat agent asks
/// before editing files / running shell commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermMode {
    /// Ask before every destructive tool call (Claude Code's Manual/default).
    Manual,
    /// Auto-approve file edits + common filesystem commands (mkdir, touch, mv, cp).
    AcceptEdits,
    /// Reads only — all mutating tools are denied before they run.
    Plan,
    /// Everything auto-approved with safety checks.
    Auto,
    /// Everything auto-approved, no checks. For isolated containers.
    Bypass,
}

impl PermMode {
    pub const ALL: [PermMode; 5] = [
        PermMode::Manual,
        PermMode::AcceptEdits,
        PermMode::Plan,
        PermMode::Auto,
        PermMode::Bypass,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            PermMode::Manual => "manual",
            PermMode::AcceptEdits => "accept-edits",
            PermMode::Plan => "plan",
            PermMode::Auto => "auto",
            PermMode::Bypass => "bypass",
        }
    }

    pub fn badge(&self) -> &'static str {
        match self {
            PermMode::Manual => "⏸ manual",
            PermMode::AcceptEdits => "✎ accept-edits",
            PermMode::Plan => "◎ plan",
            PermMode::Auto => "▶ auto",
            PermMode::Bypass => "⚠ bypass",
        }
    }

    pub fn from_label(s: &str) -> Option<Self> {
        match s {
            "manual" | "default" => Some(PermMode::Manual),
            "accept-edits" | "acceptedits" => Some(PermMode::AcceptEdits),
            "plan" => Some(PermMode::Plan),
            "auto" => Some(PermMode::Auto),
            "bypass" | "bypassPermissions" => Some(PermMode::Bypass),
            _ => None,
        }
    }

    /// Next mode in the Shift+Tab cycle.
    pub fn next(&self) -> Self {
        let idx = PermMode::ALL.iter().position(|m| m == self).unwrap_or(0);
        PermMode::ALL[(idx + 1) % PermMode::ALL.len()]
    }
}

/// How hard the model should think before answering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effort {
    Low,
    Medium,
    High,
    Max,
}

impl Effort {
    pub const ALL: [Effort; 4] = [Effort::Low, Effort::Medium, Effort::High, Effort::Max];

    pub fn label(&self) -> &'static str {
        match self {
            Effort::Low => "low",
            Effort::Medium => "medium",
            Effort::High => "high",
            Effort::Max => "max",
        }
    }

    pub fn badge(&self) -> &'static str {
        match self {
            Effort::Low => "effort:low",
            Effort::Medium => "effort:medium",
            Effort::High => "effort:high",
            Effort::Max => "effort:max",
        }
    }

    pub fn from_label(s: &str) -> Option<Self> {
        match s {
            "low" => Some(Effort::Low),
            "medium" | "med" => Some(Effort::Medium),
            "high" => Some(Effort::High),
            "max" => Some(Effort::Max),
            _ => None,
        }
    }

    /// A short prompt prefix that hints the model how hard to think.
    /// Local models don't have native effort controls; this is the next best thing.
    pub fn system_hint(&self) -> &'static str {
        match self {
            Effort::Low => "Be concise. Give short, direct answers. Skip pleasantries.",
            Effort::Medium => "Think carefully. Provide thorough but focused answers.",
            Effort::High => "Reason step by step. Explore trade-offs, edge cases, and implications.",
            Effort::Max => "Think deeply about this problem. Consider multiple approaches, verify each step of your reasoning, anticipate follow-up questions, and give the most complete, well-structured answer you can.",
        }
    }
}

/// The user's standing tool permissions.
#[derive(Default)]
pub struct Perms {
    /// Tools the user chose "always allow" for.
    pub always: HashSet<String>,
    /// Bypass every prompt (Claude Code's "bypass permissions" mode).
    pub allow_all: bool,
}

impl Perms {
    pub fn allowed(&self, tool: &str) -> bool {
        self.allow_all || self.always.contains(tool)
    }
}
