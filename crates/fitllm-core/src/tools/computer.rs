//! Computer-control tool — the hands behind Agent mode.
//!
//! macOS desktop actions: launch/quit apps, open URLs, web-search, type text,
//! press keys, and (best-effort) click at coordinates. Text/keys use AppleScript
//! `System Events` (needs Accessibility permission); clicking needs `cliclick`.
//! On non-macOS it degrades gracefully so `fitllm-core` still builds/tests.

use crate::tools::{Tool, ToolInfo, ToolParam, ToolResult, ToolSafety};

pub struct ComputerTool;

impl ComputerTool {
    pub fn new() -> Self {
        ComputerTool
    }
}

fn done(msg: impl Into<String>) -> Result<ToolResult, String> {
    Ok(ToolResult { ok: true, output: Some(msg.into()), error: None, media: None, elapsed_ms: 0 })
}
fn fail(msg: impl Into<String>) -> Result<ToolResult, String> {
    Ok(ToolResult { ok: false, output: None, error: Some(msg.into()), media: None, elapsed_ms: 0 })
}

/// Minimal percent-encoding for a web-search query.
fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

impl Tool for ComputerTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "computer".into(),
            description: "Control the computer. actions: open_app (launch an app by name, e.g. \"Google Chrome\"), quit_app, open_url (open a web address), search_web (open a Google search for a query), type (type text into the focused field), key (press a key like \"Return\" or \"Tab\"), click (click at x,y pixels). Put the app name / url / query / text in `target`.".into(),
            safety: ToolSafety::Destructive,
            parameters: vec![
                ToolParam { name: "action".into(), description: "open_app | quit_app | open_url | search_web | type | key | click".into(), required: true, param_type: "string".into() },
                ToolParam { name: "target".into(), description: "App name, URL, search query, text to type, or key name.".into(), required: false, param_type: "string".into() },
                ToolParam { name: "x".into(), description: "X pixel for click.".into(), required: false, param_type: "number".into() },
                ToolParam { name: "y".into(), description: "Y pixel for click.".into(), required: false, param_type: "number".into() },
            ],
        }
    }

    fn execute(&self, args: serde_json::Value) -> Result<ToolResult, String> {
        // Small models are sloppy: the `action` may carry junk ("open_url {…}")
        // and the real target may be missing or nested. Normalize forgivingly so
        // a malformed-but-understandable call still works.
        let action_raw = args["action"].as_str().unwrap_or("");
        let action: String = action_raw
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect::<String>()
            .to_lowercase();

        let mut target = args["target"].as_str()
            .or_else(|| args["text"].as_str())
            .or_else(|| args["app"].as_str())
            .or_else(|| args["url"].as_str())
            .or_else(|| args["query"].as_str())
            .or_else(|| args["q"].as_str())
            .or_else(|| args["name"].as_str())
            .unwrap_or("")
            .to_string();

        // Salvage a URL from anywhere in the raw args (e.g. crammed into `action`).
        if target.is_empty() {
            let raw = serde_json::to_string(&args).unwrap_or_default();
            if let Some(i) = raw.find("http") {
                let rest = &raw[i..];
                let end = rest.find(['"', '\\', ' ', '}']).unwrap_or(rest.len());
                target = rest[..end].to_string();
            }
        }

        run_action(&action, &target, &args)
    }

    fn safety(&self) -> ToolSafety {
        ToolSafety::Destructive
    }
}

/// Normalize an app name for fuzzy matching: keep letters/digits, lowercase.
#[cfg(target_os = "macos")]
fn norm(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect()
}

/// Find an installed `.app` whose name fuzzily matches `target`.
#[cfg(target_os = "macos")]
fn find_app(target: &str) -> Option<String> {
    let want = norm(target);
    if want.len() < 2 {
        return None;
    }
    let mut dirs = vec![
        "/Applications".to_string(),
        "/Applications/Utilities".to_string(),
        "/System/Applications".to_string(),
    ];
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(format!("{home}/Applications"));
    }
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let Some(stem) = name.strip_suffix(".app") else { continue };
            let n = norm(stem);
            if !n.is_empty() && (n == want || n.contains(&want) || want.contains(&n)) {
                return Some(stem.to_string());
            }
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn run_action(action: &str, target: &str, args: &serde_json::Value) -> Result<ToolResult, String> {
    use std::process::Command;
    let run = |cmd: &str, a: &[&str]| -> Result<(), String> {
        let out = Command::new(cmd).args(a).output().map_err(|e| e.to_string())?;
        if out.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&out.stderr).trim().to_string()) }
    };
    let osa = |script: String| run("osascript", &["-e", &script]);

    match action {
        "open_app" | "open" | "launch" | "start" => {
            if target.is_empty() { return fail("Provide the app name in `target`."); }
            match run("open", &["-a", target]) {
                Ok(_) => done(format!("Opened {target}.")),
                // `open -a` is picky about exact names. Fall back to a fuzzy scan
                // of the app folders so near-miss names still launch, and give a
                // definitive "not installed" answer when nothing matches.
                Err(_) => match find_app(target) {
                    Some(found) => match run("open", &["-a", &found]) {
                        Ok(_) => done(format!("Opened {found}.")),
                        Err(e) => fail(format!("Found \"{found}\" but couldn't open it: {e}")),
                    },
                    None => fail(format!(
                        "No installed app matches \"{target}\" (checked Applications). It may not be installed, or the name differs — do not assume it opened."
                    )),
                },
            }
        }
        "quit_app" | "quit" | "close_app" => {
            osa(format!("tell application \"{}\" to quit", target.replace('"', "'"))).ok();
            done(format!("Quit {target}."))
        }
        "open_url" | "url" | "goto" | "browse" => {
            let url = if target.starts_with("http") { target.to_string() } else { format!("https://{target}") };
            match run("open", &[&url]) {
                Ok(_) => done(format!("Opened {url}")),
                Err(e) => fail(format!("Couldn't open {url}: {e}")),
            }
        }
        "search_web" | "search" | "google" => {
            if target.is_empty() { return fail("Provide a search query in `target`."); }
            let url = format!("https://www.google.com/search?q={}", urlencode(target));
            match run("open", &[&url]) {
                Ok(_) => done(format!("Searched the web for \"{target}\".")),
                Err(e) => fail(format!("Search failed: {e}")),
            }
        }
        "type" | "write" => {
            let script = format!(
                "tell application \"System Events\" to keystroke \"{}\"",
                target.replace('\\', "\\\\").replace('"', "\\\"")
            );
            match osa(script) {
                Ok(_) => done(format!("Typed: {target}")),
                Err(e) => fail(format!("Type failed — grant Accessibility permission to LlamaChat in System Settings ▸ Privacy. ({e})")),
            }
        }
        "key" | "keypress" | "press" => {
            let inner = match target.to_lowercase().as_str() {
                "return" | "enter" => "key code 36".to_string(),
                "tab" => "key code 48".to_string(),
                "space" => "key code 49".to_string(),
                "escape" | "esc" => "key code 53".to_string(),
                "delete" | "backspace" => "key code 51".to_string(),
                "down" => "key code 125".to_string(),
                "up" => "key code 126".to_string(),
                _ => format!("keystroke \"{}\"", target.replace('"', "\\\"")),
            };
            match osa(format!("tell application \"System Events\" to {inner}")) {
                Ok(_) => done(format!("Pressed {target}.")),
                Err(e) => fail(format!("Key failed — grant Accessibility permission. ({e})")),
            }
        }
        "click" => {
            let x = args["x"].as_f64().unwrap_or(-1.0);
            let y = args["y"].as_f64().unwrap_or(-1.0);
            if x < 0.0 || y < 0.0 { return fail("Provide x and y pixel coordinates for click."); }
            if run("which", &["cliclick"]).is_ok() {
                match run("cliclick", &[&format!("c:{},{}", x as i64, y as i64)]) {
                    Ok(_) => done(format!("Clicked at {}, {}.", x as i64, y as i64)),
                    Err(e) => fail(format!("Click failed: {e}")),
                }
            } else {
                fail("Clicking at coordinates needs `cliclick` (run `brew install cliclick`). App-launch, web-search, typing and keys work without it.")
            }
        }
        _ => fail(format!("Unknown action \"{action}\". Use: open_app, quit_app, open_url, search_web, type, key, click.")),
    }
}

#[cfg(not(target_os = "macos"))]
fn run_action(_action: &str, _target: &str, _args: &serde_json::Value) -> Result<ToolResult, String> {
    fail("Computer control is only available on macOS.")
}
