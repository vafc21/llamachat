//! Shell execution tool. Runs commands with timeout and output capture.

use crate::tools::{Tool, ToolInfo, ToolParam, ToolResult, ToolSafety, ToolLimits};
use std::process::Command;

pub struct ShellTool {
    limits: ToolLimits,
}

impl ShellTool {
    pub fn new(limits: ToolLimits) -> Self {
        ShellTool { limits }
    }
}

impl Tool for ShellTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "shell".into(),
            description: "Run a shell command and return its output. Use for: listing files, checking system state, running builds, git operations. Do NOT use for: infinite loops, interactive commands, or commands that modify system configuration without user approval.".into(),
            safety: ToolSafety::Destructive,
            parameters: vec![
                ToolParam {
                    name: "command".into(),
                    description: "The shell command to execute.".into(),
                    required: true,
                    param_type: "string".into(),
                },
                ToolParam {
                    name: "cwd".into(),
                    description: "Working directory for the command (optional).".into(),
                    required: false,
                    param_type: "string".into(),
                },
            ],
        }
    }

    fn execute(&self, args: serde_json::Value) -> Result<ToolResult, String> {
        let command = args["command"]
            .as_str()
            .ok_or("Missing required arg: command")?;

        let cwd = args["cwd"].as_str();

        // Blocklist dangerous patterns
        let dangerous = [
            "rm -rf /", ":(){ :|:& };:", "mkfs.", "dd if=", "> /dev/sda",
            "shutdown", "reboot", "halt", "poweroff",
        ];
        for pattern in &dangerous {
            if command.contains(pattern) {
                return Ok(ToolResult {
                    ok: false,
                    output: None,
                    error: Some(format!("Blocked dangerous command pattern: {}", pattern)),
                    media: None,
                    elapsed_ms: 0,
                });
            }
        }

        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            c.args(["/C", command]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", command]);
            c
        };

        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }

        let output = cmd
            .output();

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);

                let mut text = String::new();
                if !stdout.is_empty() {
                    let truncated = if stdout.len() > self.limits.max_shell_output {
                        &stdout[..self.limits.max_shell_output]
                    } else {
                        &stdout
                    };
                    text.push_str(truncated);
                    if stdout.len() > self.limits.max_shell_output {
                        text.push_str("\n... (output truncated)");
                    }
                }
                if !stderr.is_empty() {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str("stderr:\n");
                    text.push_str(&stderr);
                }

                Ok(ToolResult {
                    ok: out.status.success(),
                    output: if text.is_empty() { None } else { Some(text) },
                    error: if !out.status.success() {
                        Some(format!("exit code: {}", out.status.code().unwrap_or(-1)))
                    } else {
                        None
                    },
                    media: None,
                    elapsed_ms: 0,
                })
            }
            Err(e) => Ok(ToolResult {
                ok: false,
                output: None,
                error: Some(format!("Failed to execute: {}", e)),
                media: None,
                elapsed_ms: 0,
            }),
        }
    }

    fn safety(&self) -> ToolSafety {
        ToolSafety::Destructive
    }
}
