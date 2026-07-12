//! Computer-control tool — the hands behind Agent mode.
//!
//! Desktop actions: launch/quit apps, open URLs, web-search, type text, press
//! keys, and click at coordinates.
//! - **macOS**: text/keys via AppleScript `System Events` (needs Accessibility
//!   permission); clicking needs `cliclick`; apps launch via `open -a`.
//! - **Windows**: text/keys/click via `enigo` (native SendInput, no extra
//!   permission); apps launch by resolving Start-Menu shortcuts / App-Paths.
//! - **Other**: degrades gracefully so `fitllm-core` still builds/tests.

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
#[cfg(any(target_os = "macos", target_os = "windows"))]
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

// ── Windows ────────────────────────────────────────────────────────────────
// Mirrors the macOS `run_action`: launch/quit apps, open URLs, web-search, type
// text, press keys, click. App launch resolves Start-Menu shortcuts (fuzzy, like
// the macOS /Applications scan) then App-Paths tokens; type/key/click use enigo
// (native SendInput — you see the cursor/keys, no extra permission on Windows).
#[cfg(target_os = "windows")]
fn run_action(action: &str, target: &str, args: &serde_json::Value) -> Result<ToolResult, String> {
    match action {
        "open_app" | "open" | "launch" | "start" => win::open_app(target),
        "quit_app" | "quit" | "close_app" => win::quit_app(target),
        "open_url" | "url" | "goto" | "browse" => {
            if target.is_empty() { return fail("Provide a URL in `target`."); }
            let url = if target.starts_with("http") { target.to_string() } else { format!("https://{target}") };
            match win::launch(&url) {
                Ok(_) => done(format!("Opened {url}")),
                Err(e) => fail(format!("Couldn't open {url}: {e}")),
            }
        }
        "search_web" | "search" | "google" => {
            if target.is_empty() { return fail("Provide a search query in `target`."); }
            let url = format!("https://www.google.com/search?q={}", urlencode(target));
            match win::launch(&url) {
                Ok(_) => done(format!("Searched the web for \"{target}\".")),
                Err(e) => fail(format!("Search failed: {e}")),
            }
        }
        "type" | "write" => {
            if target.is_empty() { return fail("Provide the text to type in `target`."); }
            match win::type_text(target) {
                Ok(_) => done(format!("Typed: {target}")),
                Err(e) => fail(format!("Type failed: {e}")),
            }
        }
        "key" | "keypress" | "press" => {
            if target.is_empty() { return fail("Provide a key name in `target` (e.g. Return, Tab)."); }
            match win::press_key(target) {
                Ok(_) => done(format!("Pressed {target}.")),
                Err(e) => fail(format!("Key failed: {e}")),
            }
        }
        "click" => {
            let x = args["x"].as_f64().unwrap_or(-1.0);
            let y = args["y"].as_f64().unwrap_or(-1.0);
            if x < 0.0 || y < 0.0 { return fail("Provide x and y pixel coordinates for click."); }
            match win::click(x as i32, y as i32) {
                Ok(_) => done(format!("Clicked at {}, {}.", x as i64, y as i64)),
                Err(e) => fail(format!("Click failed: {e}")),
            }
        }
        _ => fail(format!("Unknown action \"{action}\". Use: open_app, quit_app, open_url, search_web, type, key, click.")),
    }
}

#[cfg(target_os = "windows")]
mod win {
    use super::{done, fail, norm};
    use crate::tools::ToolResult;
    use enigo::{Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};
    use std::os::windows::process::CommandExt;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    /// Run console helpers (powershell, taskkill) without flashing a window.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    fn hidden(program: &str) -> Command {
        let mut c = Command::new(program);
        c.creation_flags(CREATE_NO_WINDOW);
        c
    }

    /// Launch a program, URL, or shortcut path via PowerShell `Start-Process`,
    /// which resolves App-Paths / PATH / `.lnk` / `.url` and returns a nonzero
    /// exit (detectable) when the target does not exist.
    pub fn launch(target: &str) -> Result<(), String> {
        let esc = target.replace('\'', "''"); // single-quote escaping for PS
        let out = hidden("powershell")
            .args(["-NoProfile", "-Command", &format!("Start-Process -FilePath '{esc}'")])
            .output()
            .map_err(|e| e.to_string())?;
        if out.status.success() {
            Ok(())
        } else {
            let err = String::from_utf8_lossy(&out.stderr);
            Err(err.trim().to_string())
        }
    }

    /// Machine + per-user Start-Menu shortcut roots.
    fn start_menu_dirs() -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        for var in ["ProgramData", "APPDATA"] {
            if let Ok(base) = std::env::var(var) {
                dirs.push(PathBuf::from(base).join(r"Microsoft\Windows\Start Menu\Programs"));
            }
        }
        dirs
    }

    /// Collect `.lnk`/`.url` shortcuts under `dir` (bounded recursion).
    fn collect_shortcuts(dir: &Path, depth: u32, out: &mut Vec<PathBuf>) {
        if depth > 5 {
            return;
        }
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                collect_shortcuts(&p, depth + 1, out);
            } else if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
                let ext = ext.to_ascii_lowercase();
                if ext == "lnk" || ext == "url" {
                    out.push(p);
                }
            }
        }
    }

    /// Find the Start-Menu shortcut whose file stem best matches `want`
    /// (0 = exact, 1 = prefix, 2 = substring). Mirrors the macOS `find_app` scan.
    fn find_shortcut(want: &str) -> Option<PathBuf> {
        if want.len() < 2 {
            return None;
        }
        let mut all = Vec::new();
        for d in start_menu_dirs() {
            collect_shortcuts(&d, 0, &mut all);
        }
        let mut best: Option<(u8, PathBuf)> = None;
        for p in all {
            let Some(stem) = p.file_stem().and_then(|s| s.to_str()) else { continue };
            let n = norm(stem);
            if n.is_empty() {
                continue;
            }
            let rank = if n == want {
                0
            } else if n.starts_with(want) || want.starts_with(n.as_str()) {
                1
            } else if n.contains(want) || want.contains(n.as_str()) {
                2
            } else {
                continue;
            };
            if best.as_ref().is_none_or(|(r, _)| rank < *r) {
                if rank == 0 {
                    return Some(p);
                }
                best = Some((rank, p));
            }
        }
        best.map(|(_, p)| p)
    }

    /// System / UWP apps that resolve through App-Paths without a shortcut.
    fn known_token(want: &str) -> Option<&'static str> {
        Some(match want {
            "googlechrome" | "chrome" => "chrome",
            "microsoftedge" | "edge" | "msedge" => "msedge",
            "firefox" | "mozillafirefox" => "firefox",
            "notepad" => "notepad",
            "calculator" | "calc" => "calc",
            "paint" | "mspaint" => "mspaint",
            "wordpad" => "write",
            "fileexplorer" | "explorer" | "windowsexplorer" | "files" => "explorer",
            "commandprompt" | "cmd" | "command" => "cmd",
            "powershell" => "powershell",
            "windowsterminal" | "terminal" => "wt",
            "taskmanager" => "taskmgr",
            "controlpanel" => "control",
            "settings" => "ms-settings:",
            _ => return None,
        })
    }

    pub fn open_app(target: &str) -> Result<ToolResult, String> {
        if target.trim().is_empty() {
            return fail("Provide the app name in `target`.");
        }
        let want = norm(target);
        // 1) A real Start-Menu shortcut is the most reliable, honest match.
        if let Some(path) = find_shortcut(&want) {
            let label = path.file_stem().and_then(|s| s.to_str()).unwrap_or(target).to_string();
            return match launch(&path.to_string_lossy()) {
                Ok(_) => done(format!("Opened {label}.")),
                Err(e) => fail(format!("Found \"{label}\" but couldn't open it: {e}")),
            };
        }
        // 2) A known system/UWP app that resolves via App-Paths.
        if let Some(tok) = known_token(&want) {
            if launch(tok).is_ok() {
                return done(format!("Opened {target}."));
            }
        }
        // 3) Last resort: let Start-Process resolve the raw name (PATH / App-Paths).
        match launch(target) {
            Ok(_) => done(format!("Opened {target}.")),
            Err(_) => fail(format!(
                "No installed app matches \"{target}\" (checked the Start Menu and PATH). It may not be installed, or the name differs — do not assume it opened."
            )),
        }
    }

    pub fn quit_app(target: &str) -> Result<ToolResult, String> {
        if target.trim().is_empty() {
            return fail("Provide the app name in `target`.");
        }
        // Best-effort graceful close by image name (WM_CLOSE, no /F).
        let want = target.trim();
        let image = if want.to_ascii_lowercase().ends_with(".exe") {
            want.to_string()
        } else {
            format!("{want}.exe")
        };
        let _ = hidden("taskkill").args(["/IM", &image]).output();
        done(format!("Asked {target} to close."))
    }

    fn enigo() -> Result<Enigo, String> {
        Enigo::new(&Settings::default()).map_err(|e| format!("Input control unavailable ({e})."))
    }

    pub fn type_text(text: &str) -> Result<(), String> {
        enigo()?.text(text).map_err(|e| e.to_string())
    }

    pub fn press_key(name: &str) -> Result<(), String> {
        let mut e = enigo()?;
        let key = match name.trim().to_lowercase().as_str() {
            "return" | "enter" => Key::Return,
            "tab" => Key::Tab,
            "space" => Key::Space,
            "escape" | "esc" => Key::Escape,
            "delete" | "backspace" => Key::Backspace,
            "down" => Key::DownArrow,
            "up" => Key::UpArrow,
            "left" => Key::LeftArrow,
            "right" => Key::RightArrow,
            "home" => Key::Home,
            "end" => Key::End,
            other => {
                let cs: Vec<char> = other.chars().collect();
                if cs.len() == 1 {
                    Key::Unicode(cs[0])
                } else {
                    // Multi-char "key" name — type it as literal text instead.
                    return e.text(other).map_err(|er| er.to_string());
                }
            }
        };
        e.key(key, Direction::Click).map_err(|e| e.to_string())
    }

    pub fn click(x: i32, y: i32) -> Result<(), String> {
        let mut e = enigo()?;
        e.move_mouse(x, y, Coordinate::Abs).map_err(|er| er.to_string())?;
        e.button(Button::Left, Direction::Click).map_err(|er| er.to_string())
    }
}

// ── Linux / other (no native desktop control yet) ──────────────────────────
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn run_action(_action: &str, _target: &str, _args: &serde_json::Value) -> Result<ToolResult, String> {
    fail("Computer control (open_app/type/key) is not implemented on this platform yet.")
}
