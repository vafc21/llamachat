//! Markdown-file persistence for chats and long-term memory.
//!
//! Everything the app remembers lives as human-readable `.md` files the user
//! owns and can edit or move:
//!   <root>/conversations/<id>.md   one transcript per chat
//!   <root>/memory.md               durable facts, injected into every chat
//!
//! `<root>` defaults to the app data dir but can be changed in Settings
//! (`AppSettings.memory_dir`). Conversations use HTML-comment markers so they
//! stay valid, readable markdown yet round-trip back into the app.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ConvMsg {
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub timestamp: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ConvDto {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    pub messages: Vec<ConvMsg>,
}

/// Expand a leading `~` to the user's home directory.
fn expand_tilde(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(p)
}

/// The memory root: the Settings override (if any) or the default data dir.
pub fn root(dir_override: &Option<String>) -> PathBuf {
    match dir_override {
        Some(d) if !d.trim().is_empty() => expand_tilde(d.trim()),
        _ => crate::state::data_dir(),
    }
}

fn conversations_dir(dir_override: &Option<String>) -> PathBuf {
    root(dir_override).join("conversations")
}

fn memory_path(dir_override: &Option<String>) -> PathBuf {
    root(dir_override).join("memory.md")
}

/// Sanitize an id into a safe filename stem.
fn safe_stem(id: &str) -> String {
    id.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

// ── Long-term memory (memory.md) ───────────────────────────

pub fn read_memory(dir_override: &Option<String>) -> String {
    fs::read_to_string(memory_path(dir_override)).unwrap_or_default()
}

pub fn write_memory(dir_override: &Option<String>, content: &str) -> Result<(), String> {
    let path = memory_path(dir_override);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(path, content).map_err(|e| e.to_string())
}

// ── Conversations ──────────────────────────────────────────

pub fn save_conversation(dir_override: &Option<String>, conv: &ConvDto) -> Result<(), String> {
    // Don't litter the disk with empty "New conversation" placeholders.
    if conv.messages.iter().all(|m| m.content.trim().is_empty()) {
        return Ok(());
    }
    let dir = conversations_dir(dir_override);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}.md", safe_stem(&conv.id)));
    fs::write(path, to_markdown(conv)).map_err(|e| e.to_string())
}

pub fn delete_conversation(dir_override: &Option<String>, id: &str) -> Result<(), String> {
    let path = conversations_dir(dir_override).join(format!("{}.md", safe_stem(id)));
    if path.exists() {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Load every saved conversation, newest first (by `created_at`).
pub fn list_conversations(dir_override: &Option<String>) -> Vec<ConvDto> {
    let dir = conversations_dir(dir_override);
    let mut out: Vec<ConvDto> = Vec::new();
    if let Ok(entries) = fs::read_dir(&dir) {
        for e in entries.flatten() {
            let path = e.path();
            if path.extension().and_then(|s| s.to_str()) != Some("md") {
                continue;
            }
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
            if let Ok(text) = fs::read_to_string(&path) {
                out.push(from_markdown(&stem, &text));
            }
        }
    }
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    out
}

// ── Markdown (de)serialization ─────────────────────────────

fn to_markdown(conv: &ConvDto) -> String {
    let mut s = String::new();
    s.push_str("---\n");
    s.push_str(&format!("title: {}\n", conv.title.replace('\n', " ")));
    s.push_str(&format!("created: {}\n", conv.created_at));
    if let Some(sp) = &conv.system_prompt {
        if !sp.trim().is_empty() {
            s.push_str(&format!("system: {}\n", sp.replace('\n', "\\n")));
        }
    }
    s.push_str("---\n\n");
    for m in &conv.messages {
        if m.content.trim().is_empty() {
            continue;
        }
        s.push_str(&format!("<!--m:{} t:{}-->\n", m.role, m.timestamp));
        s.push_str(m.content.trim_end());
        s.push_str("\n\n");
    }
    s
}

fn from_markdown(id: &str, text: &str) -> ConvDto {
    let mut title = String::new();
    let mut created_at = String::new();
    let mut system_prompt: Option<String> = None;
    let mut body = text;

    // Frontmatter between the first two `---` lines.
    if let Some(rest) = text.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---\n") {
            let front = &rest[..end];
            body = &rest[end + 5..];
            for line in front.lines() {
                if let Some(v) = line.strip_prefix("title:") {
                    title = v.trim().to_string();
                } else if let Some(v) = line.strip_prefix("created:") {
                    created_at = v.trim().to_string();
                } else if let Some(v) = line.strip_prefix("system:") {
                    let sp = v.trim().replace("\\n", "\n");
                    if !sp.is_empty() {
                        system_prompt = Some(sp);
                    }
                }
            }
        }
    }

    // Messages: each begins with `<!--m:<role> t:<ts>-->`; content follows until
    // the next marker.
    let mut messages: Vec<ConvMsg> = Vec::new();
    let mut cur_role: Option<String> = None;
    let mut cur_ts = String::new();
    let mut cur_content = String::new();
    let flush = |role: &Option<String>, ts: &str, content: &str, out: &mut Vec<ConvMsg>| {
        if let Some(r) = role {
            let c = content.trim();
            if !c.is_empty() {
                out.push(ConvMsg { role: r.clone(), content: c.to_string(), timestamp: ts.to_string() });
            }
        }
    };
    for line in body.lines() {
        if let Some(marker) = line.strip_prefix("<!--m:").and_then(|s| s.strip_suffix("-->")) {
            flush(&cur_role, &cur_ts, &cur_content, &mut messages);
            cur_content.clear();
            // marker = "<role> t:<ts>"
            let (role, ts) = match marker.split_once(" t:") {
                Some((r, t)) => (r.trim().to_string(), t.trim().to_string()),
                None => (marker.trim().to_string(), String::new()),
            };
            cur_role = Some(role);
            cur_ts = ts;
        } else if cur_role.is_some() {
            cur_content.push_str(line);
            cur_content.push('\n');
        }
    }
    flush(&cur_role, &cur_ts, &cur_content, &mut messages);

    if title.is_empty() {
        title = messages.first().map(|m| {
            let t: String = m.content.chars().take(60).collect();
            t
        }).unwrap_or_else(|| "Conversation".into());
    }

    ConvDto { id: id.to_string(), title, created_at, system_prompt, messages }
}
