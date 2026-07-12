//! Desktop tool: screenshots and window interaction. Cross-platform.
//! Lets the assistant "see" what's on screen, similar to Desktop Commander's
//! visual capabilities.

use crate::tools::{Tool, ToolInfo, ToolParam, ToolResult, ToolSafety};
use std::process::Command;

pub struct DesktopTool;

impl DesktopTool {
    pub fn new() -> Self {
        DesktopTool
    }

    /// Take a screenshot and return the file path. Platform-specific backends.
    fn screenshot(&self, path: &str) -> Result<String, String> {
        if cfg!(target_os = "linux") {
            let backends: &[(&str, &[&str])] = &[
                ("import", &["-window", "root", path] as &[&str]),
                ("gnome-screenshot", &["-f", path]),
                ("scrot", &[path]),
                ("grim", &[path]),
                ("spectacle", &["-b", "-n", "-o", path]),
            ];

            let mut found_any = false;
            let mut last_display_error = String::new();

            for (cmd, args) in backends {
                match Command::new(cmd).args(*args).output() {
                    Ok(output) if output.status.success() => return Ok(path.to_string()),
                    Ok(output) => {
                        found_any = true;
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        if stderr.contains("X server") || stderr.contains("display") || stderr.contains("DISPLAY") {
                            last_display_error = format!("{} present but no display available", cmd);
                        }
                    }
                    Err(_) => continue,
                }
            }

            if !last_display_error.is_empty() {
                Err(format!("Screenshot tools found but no graphical display is available. This host runs headless — screenshots require a desktop environment. Details: {}", last_display_error))
            } else if found_any {
                Err("Screenshot tools are installed but failed to capture. Is a desktop environment running?".into())
            } else {
                Err("No screenshot tool found. Install: sudo apt install imagemagick".into())
            }
        } else if cfg!(target_os = "macos") {
            let output = Command::new("screencapture")
                .args(["-x", path]) // -x = no sound
                .output()
                .map_err(|e| format!("screencapture failed: {}", e))?;
            if output.status.success() {
                Ok(path.to_string())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(format!("screencapture error: {}", stderr))
            }
        } else if cfg!(target_os = "windows") {
            // Windows: use PowerShell to take screenshot
            let ps_script = format!(
                r#"Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
$screen = [System.Windows.Forms.Screen]::PrimaryScreen
$bitmap = New-Object System.Drawing.Bitmap($screen.Bounds.Width, $screen.Bounds.Height)
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.CopyFromScreen($screen.Bounds.X, $screen.Bounds.Y, 0, 0, $bitmap.Size)
$bitmap.Save('{}', [System.Drawing.Imaging.ImageFormat]::Png)
$graphics.Dispose()
$bitmap.Dispose()
"#,
                path.replace("'", "''")
            );
            let output = Command::new("powershell")
                .args(["-NoProfile", "-Command", &ps_script])
                .output()
                .map_err(|e| format!("powershell screenshot failed: {}", e))?;
            if output.status.success() {
                Ok(path.to_string())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(format!("Screenshot error: {}", stderr))
            }
        } else {
            Err("Unsupported platform".into())
        }
    }
}

impl Tool for DesktopTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "desktop".into(),
            description: "Take screenshots of the desktop to see what's on screen. Use to inspect UI, read error messages, or verify visual state before interacting. Returns the file path of the screenshot.".into(),
            safety: ToolSafety::ReadOnly,
            parameters: vec![
                ToolParam {
                    name: "action".into(),
                    description: "Action: 'screenshot' to capture the full screen".into(),
                    required: true,
                    param_type: "string".into(),
                },
                ToolParam {
                    name: "path".into(),
                    description: "Where to save the screenshot (optional, defaults to temp file)".into(),
                    required: false,
                    param_type: "string".into(),
                },
            ],
        }
    }

    fn execute(&self, args: serde_json::Value) -> Result<ToolResult, String> {
        let action = args["action"]
            .as_str()
            .ok_or("Missing action (use 'screenshot')")?;

        match action {
            "screenshot" => {
                let path = args["path"]
                    .as_str()
                    .unwrap_or(&format!("/tmp/llamachat-screenshot-{}.png",
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs()))
                    .to_string();

                match self.screenshot(&path) {
                    Ok(p) => Ok(ToolResult {
                        ok: true,
                        output: Some(format!("Screenshot saved: {}", p)),
                        error: None,
                        media: Some(p),
                        elapsed_ms: 0,
                    }),
                    Err(e) => Ok(ToolResult {
                        ok: false,
                        output: None,
                        error: Some(e),
                        media: None,
                        elapsed_ms: 0,
                    }),
                }
            }
            _ => Err(format!("Unknown desktop action: {}", action)),
        }
    }

    fn safety(&self) -> ToolSafety {
        ToolSafety::ReadOnly
    }
}
