//! Real desktop control for Agent mode.
//!
//! - Native mouse/keyboard via `enigo` (you see the cursor move).
//! - `read_screen`: the frontmost app's accessibility tree as text, so a
//!   text-only model knows what's on screen and where (roles + labels + x,y).
//! - `describe_screen`: optional screenshot → a local vision model describes it
//!   as text for the main model (perception = "vision").
//!
//! macOS needs Accessibility permission (mouse/keys/AX read) and Screen
//! Recording (screenshots). Errors say so.

use serde_json::Value;

/// Actions handled here (the rest of the `computer` tool stays in fitllm-core).
pub const DESKTOP_ACTIONS: &[&str] = &[
    "read_screen", "read_ui", "screen", "mouse_move", "move", "move_mouse",
    "click", "left_click", "double_click", "right_click", "drag", "scroll",
];

pub fn is_desktop_action(action: &str) -> bool {
    DESKTOP_ACTIONS.contains(&action)
}

#[cfg(target_os = "macos")]
pub fn control(action: &str, args: &Value) -> Result<String, String> {
    mac::control(action, args)
}

#[cfg(target_os = "windows")]
pub fn control(action: &str, args: &Value) -> Result<String, String> {
    windows::control(action, args)
}

#[cfg(target_os = "linux")]
pub fn control(action: &str, args: &Value) -> Result<String, String> {
    linux::control(action, args)
}

/// Screenshot the screen and have a local vision model describe it as text.
/// The capture itself is platform-specific (`screenshot_to`); the vision call
/// is shared across all OSes.
pub fn describe_screen(vision_model: &str) -> Result<String, String> {
    use base64::Engine;
    let path = std::env::temp_dir().join("fitllm-agent-screen.png");
    if !screenshot_to(&path.to_string_lossy()) {
        return Err("Screenshot failed — grant Screen Recording permission (macOS: System Settings ▸ Privacy) or ensure a screenshot tool is installed (Linux: grim/scrot/imagemagick).".into());
    }
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    let body = serde_json::json!({
        "model": vision_model,
        "prompt": "You are the eyes of another AI agent. Describe this screen factually and concisely: the frontmost app, visible windows, and the key clickable elements (buttons, fields, links) with roughly where they are. Do not speculate.",
        "images": [b64],
        "stream": false,
    });
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .post("http://127.0.0.1:11434/api/generate")
        .json(&body)
        .send()
        .map_err(|e| format!("vision request failed: {e}"))?;
    let v: Value = resp.json().map_err(|e| e.to_string())?;
    v.get("response")
        .and_then(|r| r.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            let err = v.get("error").and_then(|e| e.as_str()).unwrap_or("no description");
            format!("Vision model \"{vision_model}\" returned nothing ({err}). Is it a vision model, and pulled?")
        })
}

// ── Platform screenshot (writes a PNG to `path`, returns success) ──────────

#[cfg(target_os = "macos")]
fn screenshot_to(path: &str) -> bool {
    std::process::Command::new("screencapture")
        .args(["-x", path])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
fn screenshot_to(path: &str) -> bool {
    // Full virtual-screen capture via .NET, no extra dependency.
    let ps = format!(
        "Add-Type -AssemblyName System.Windows.Forms,System.Drawing; \
         $b=[System.Windows.Forms.SystemInformation]::VirtualScreen; \
         $bmp=New-Object System.Drawing.Bitmap $b.Width,$b.Height; \
         $g=[System.Drawing.Graphics]::FromImage($bmp); \
         $g.CopyFromScreen($b.X,$b.Y,0,0,$bmp.Size); \
         $bmp.Save('{}');",
        path.replace('\\', "\\\\").replace('\'', "''")
    );
    std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn screenshot_to(path: &str) -> bool {
    // Try Wayland (grim), then X11 (scrot), then ImageMagick (import).
    let attempts: [(&str, Vec<&str>); 3] = [
        ("grim", vec![path]),
        ("scrot", vec!["-o", path]),
        ("import", vec!["-window", "root", path]),
    ];
    attempts.iter().any(|(tool, args)| {
        std::process::Command::new(tool)
            .args(args)
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    })
}

#[cfg(target_os = "macos")]
mod mac {
    use enigo::{Axis, Button, Coordinate, Direction, Enigo, Mouse, Settings};
    use serde_json::Value;
    use std::process::Command;

    // ── Force Electron/Chromium apps to expose their accessibility tree ──────
    // Chromium builds its AX tree lazily; until an assistive client sets
    // AXManualAccessibility (or AXEnhancedUserInterface, which VoiceOver sets),
    // apps like Discord/Slack expose only window chrome. We set both on the
    // target process so read_screen sees the real UI with real coordinates.
    // Requires LlamaChat to be a trusted Accessibility client (it is, for input).
    mod ax {
        use std::os::raw::{c_char, c_int, c_void};

        type AXUIElementRef = *const c_void;
        type CFStringRef = *const c_void;
        type CFTypeRef = *const c_void;
        type CFAllocatorRef = *const c_void;

        #[link(name = "ApplicationServices", kind = "framework")]
        extern "C" {
            fn AXUIElementCreateApplication(pid: c_int) -> AXUIElementRef;
            fn AXUIElementSetAttributeValue(
                element: AXUIElementRef,
                attribute: CFStringRef,
                value: CFTypeRef,
            ) -> c_int;
        }
        #[link(name = "CoreFoundation", kind = "framework")]
        extern "C" {
            static kCFBooleanTrue: CFTypeRef;
            fn CFStringCreateWithCString(
                alloc: CFAllocatorRef,
                c_str: *const c_char,
                encoding: u32,
            ) -> CFStringRef;
            fn CFRelease(cf: CFTypeRef);
        }
        const KCFSTRING_ENCODING_UTF8: u32 = 0x0800_0100;

        /// Turn on the full accessibility tree for the process `pid` (best-effort).
        pub fn force_tree(pid: i32) {
            unsafe {
                let app = AXUIElementCreateApplication(pid);
                if app.is_null() {
                    return;
                }
                for attr in ["AXManualAccessibility", "AXEnhancedUserInterface"] {
                    if let Ok(c) = std::ffi::CString::new(attr) {
                        let key = CFStringCreateWithCString(
                            std::ptr::null(),
                            c.as_ptr(),
                            KCFSTRING_ENCODING_UTF8,
                        );
                        if !key.is_null() {
                            AXUIElementSetAttributeValue(app, key, kCFBooleanTrue);
                            CFRelease(key);
                        }
                    }
                }
                CFRelease(app);
            }
        }
    }

    /// PID of the frontmost application process (to force its AX tree on).
    fn frontmost_pid() -> Option<i32> {
        let out = Command::new("osascript")
            .args(["-e", "tell application \"System Events\" to get unix id of first application process whose frontmost is true"])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        String::from_utf8_lossy(&out.stdout).trim().parse::<i32>().ok()
    }

    fn enigo() -> Result<Enigo, String> {
        Enigo::new(&Settings::default()).map_err(|e| {
            format!("Input control unavailable ({e}). Grant LlamaChat Accessibility permission: System Settings ▸ Privacy & Security ▸ Accessibility.")
        })
    }

    fn coords(args: &Value) -> Result<(i32, i32), String> {
        match (args.get("x").and_then(|v| v.as_f64()), args.get("y").and_then(|v| v.as_f64())) {
            (Some(x), Some(y)) => Ok((x as i32, y as i32)),
            _ => Err("Needs x and y pixel coordinates — call read_screen first to get element positions.".into()),
        }
    }

    pub fn control(action: &str, args: &Value) -> Result<String, String> {
        match action {
            "read_screen" | "read_ui" | "screen" => {
                let app = args.get("app").and_then(|v| v.as_str())
                    .or_else(|| args.get("target").and_then(|v| v.as_str()))
                    .or_else(|| args.get("window").and_then(|v| v.as_str()))
                    .map(str::trim)
                    .filter(|s| !s.is_empty());
                read_screen(app)
            }
            "mouse_move" | "move" | "move_mouse" => {
                let (x, y) = coords(args)?;
                enigo()?.move_mouse(x, y, Coordinate::Abs).map_err(|e| e.to_string())?;
                Ok(format!("Moved cursor to {x},{y}."))
            }
            "click" | "left_click" | "double_click" | "right_click" => {
                let (x, y) = coords(args)?;
                let mut e = enigo()?;
                e.move_mouse(x, y, Coordinate::Abs).map_err(|er| er.to_string())?;
                let button = if action == "right_click" { Button::Right } else { Button::Left };
                let times = if action == "double_click" { 2 } else { 1 };
                for _ in 0..times {
                    e.button(button, Direction::Click).map_err(|er| er.to_string())?;
                }
                Ok(format!("Clicked at {x},{y}."))
            }
            "drag" => {
                let (x, y) = coords(args)?;
                let x2 = args.get("x2").and_then(|v| v.as_f64()).ok_or("drag needs x2 and y2")? as i32;
                let y2 = args.get("y2").and_then(|v| v.as_f64()).ok_or("drag needs x2 and y2")? as i32;
                let mut e = enigo()?;
                e.move_mouse(x, y, Coordinate::Abs).map_err(|er| er.to_string())?;
                e.button(Button::Left, Direction::Press).map_err(|er| er.to_string())?;
                e.move_mouse(x2, y2, Coordinate::Abs).map_err(|er| er.to_string())?;
                e.button(Button::Left, Direction::Release).map_err(|er| er.to_string())?;
                Ok(format!("Dragged from {x},{y} to {x2},{y2}."))
            }
            "scroll" => {
                let amount = args.get("amount").and_then(|v| v.as_f64())
                    .or_else(|| args.get("y").and_then(|v| v.as_f64()))
                    .unwrap_or(5.0) as i32;
                let dir = args.get("direction").and_then(|v| v.as_str()).unwrap_or("down");
                let a = if dir == "up" { -amount.abs() } else { amount.abs() };
                enigo()?.scroll(a, Axis::Vertical).map_err(|e| e.to_string())?;
                Ok(format!("Scrolled {dir}."))
            }
            _ => Err(format!("Unknown desktop action \"{action}\".")),
        }
    }

    /// An app's interactive UI elements as text (role: label @ x,y). If `app`
    /// is given, bring THAT app to the front first (via `open -a`, which needs
    /// no extra permission) and read it — so the agent can see an app it opened
    /// even while LlamaChat's own window has keyboard focus. Otherwise reads
    /// whatever is frontmost.
    pub fn read_screen(app: Option<&str>) -> Result<String, String> {
        if let Some(a) = app {
            let _ = Command::new("open").args(["-a", a]).status();
            std::thread::sleep(std::time::Duration::from_millis(700));
        }
        // Force the frontmost app to build its full accessibility tree (Electron
        // apps like Discord hide it), then give it a moment before reading.
        if let Some(pid) = frontmost_pid() {
            ax::force_tree(pid);
            std::thread::sleep(std::time::Duration::from_millis(650));
        }
        let script = r#"
tell application "System Events"
  set frontApp to first application process whose frontmost is true
  set out to "Frontmost app: " & (name of frontApp) & linefeed
  set winName to ""
  try
    set winName to name of front window of frontApp
  end try
  set out to out & "Window: " & winName & linefeed & "Clickable elements (role: label @ x,y):" & linefeed
  set n to 0
  try
    set els to entire contents of front window of frontApp
    repeat with e in els
      if n is greater than 45 then exit repeat
      try
        set r to (role of e) as text
        if r is in {"AXButton", "AXTextField", "AXTextArea", "AXCheckBox", "AXRadioButton", "AXPopUpButton", "AXMenuButton", "AXLink", "AXComboBox"} then
          set lbl to ""
          try
            set lbl to (description of e) as text
          end try
          if lbl is "" then
            try
              set lbl to (name of e) as text
            end try
          end if
          if lbl is "" then
            try
              set lbl to (value of e) as text
            end try
          end if
          set p to position of e
          set out to out & r & ": " & lbl & " @ " & (item 1 of p) & "," & (item 2 of p) & linefeed
          set n to n + 1
        end if
      end try
    end repeat
  end try
  if n is 0 then set out to out & "(No accessibility elements exposed — this app may need the vision perception mode.)"
  return out
end tell
"#;
        let out = Command::new("osascript")
            .arg("-e")
            .arg(script)
            .output()
            .map_err(|e| e.to_string())?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        } else {
            let err = String::from_utf8_lossy(&out.stderr);
            Err(format!("read_screen failed — grant Accessibility permission. ({})", err.trim()))
        }
    }
}

// ── Shared enigo mouse input (non-macOS; enigo is cross-platform) ──────────
// macOS keeps its own copy in `mod mac`; this serves Windows and Linux so the
// agent can move/click/drag/scroll on those platforms today. Only perception
// (read_screen) is left per-OS to implement.
#[cfg(not(target_os = "macos"))]
mod input {
    use enigo::{Axis, Button, Coordinate, Direction, Enigo, Mouse, Settings};
    use serde_json::Value;

    fn enigo() -> Result<Enigo, String> {
        Enigo::new(&Settings::default()).map_err(|e| format!("Input control unavailable ({e})."))
    }
    fn coords(args: &Value) -> Result<(i32, i32), String> {
        match (args.get("x").and_then(|v| v.as_f64()), args.get("y").and_then(|v| v.as_f64())) {
            (Some(x), Some(y)) => Ok((x as i32, y as i32)),
            _ => Err("Needs x and y pixel coordinates — call read_screen first.".into()),
        }
    }
    pub fn control(action: &str, args: &Value) -> Result<String, String> {
        match action {
            "mouse_move" | "move" | "move_mouse" => {
                let (x, y) = coords(args)?;
                enigo()?.move_mouse(x, y, Coordinate::Abs).map_err(|e| e.to_string())?;
                Ok(format!("Moved cursor to {x},{y}."))
            }
            "click" | "left_click" | "double_click" | "right_click" => {
                let (x, y) = coords(args)?;
                let mut e = enigo()?;
                e.move_mouse(x, y, Coordinate::Abs).map_err(|er| er.to_string())?;
                let button = if action == "right_click" { Button::Right } else { Button::Left };
                let times = if action == "double_click" { 2 } else { 1 };
                for _ in 0..times {
                    e.button(button, Direction::Click).map_err(|er| er.to_string())?;
                }
                Ok(format!("Clicked at {x},{y}."))
            }
            "drag" => {
                let (x, y) = coords(args)?;
                let x2 = args.get("x2").and_then(|v| v.as_f64()).ok_or("drag needs x2 and y2")? as i32;
                let y2 = args.get("y2").and_then(|v| v.as_f64()).ok_or("drag needs x2 and y2")? as i32;
                let mut e = enigo()?;
                e.move_mouse(x, y, Coordinate::Abs).map_err(|er| er.to_string())?;
                e.button(Button::Left, Direction::Press).map_err(|er| er.to_string())?;
                e.move_mouse(x2, y2, Coordinate::Abs).map_err(|er| er.to_string())?;
                e.button(Button::Left, Direction::Release).map_err(|er| er.to_string())?;
                Ok(format!("Dragged from {x},{y} to {x2},{y2}."))
            }
            "scroll" => {
                let amount = args.get("amount").and_then(|v| v.as_f64())
                    .or_else(|| args.get("y").and_then(|v| v.as_f64())).unwrap_or(5.0) as i32;
                let dir = args.get("direction").and_then(|v| v.as_str()).unwrap_or("down");
                let a = if dir == "up" { -amount.abs() } else { amount.abs() };
                enigo()?.scroll(a, Axis::Vertical).map_err(|e| e.to_string())?;
                Ok(format!("Scrolled {dir}."))
            }
            _ => Err(format!("Unknown desktop action \"{action}\".")),
        }
    }
}

// ── Windows (scaffold) ────────────────────────────────────────────────────
#[cfg(target_os = "windows")]
mod windows {
    use serde_json::Value;

    pub fn control(action: &str, args: &Value) -> Result<String, String> {
        match action {
            "read_screen" | "read_ui" | "screen" => read_screen(args),
            _ => super::input::control(action, args),
        }
    }

    /// TODO(windows): read the foreground window's UI Automation tree (element
    /// roles + names + screen rects) into "AXRole: label @ x,y" lines, mirroring
    /// the macOS AX reader in `mod mac`. A good path is the `uiautomation` crate.
    /// Until then we return the "no elements" marker so the agent auto-switches
    /// to screenshot vision (see agent.rs::ax_is_empty + desktop::describe_screen).
    fn read_screen(_args: &Value) -> Result<String, String> {
        Ok("Frontmost app: (unknown)\nWindow: \nClickable elements (role: label @ x,y):\n(No accessibility elements exposed — this app may need the vision perception mode.)".into())
    }
}

// ── Linux (scaffold) ──────────────────────────────────────────────────────
#[cfg(target_os = "linux")]
mod linux {
    use serde_json::Value;

    pub fn control(action: &str, args: &Value) -> Result<String, String> {
        match action {
            "read_screen" | "read_ui" | "screen" => read_screen(args),
            _ => super::input::control(action, args),
        }
    }

    /// TODO(linux): read the focused window's AT-SPI accessibility tree into
    /// "AXRole: label @ x,y" lines (the `atspi` crate on X11/Wayland). Until then
    /// we return the "no elements" marker so the agent auto-switches to
    /// screenshot vision (needs grim/scrot/imagemagick installed).
    fn read_screen(_args: &Value) -> Result<String, String> {
        Ok("Frontmost app: (unknown)\nWindow: \nClickable elements (role: label @ x,y):\n(No accessibility elements exposed — this app may need the vision perception mode.)".into())
    }
}
