//! Process management tool: list, spawn, and kill background processes.

use crate::tools::{Tool, ToolInfo, ToolParam, ToolResult, ToolSafety, ToolLimits};
use std::process::Command;

pub struct ProcessTool {
    limits: ToolLimits,
}

impl ProcessTool {
    pub fn new(limits: ToolLimits) -> Self {
        ProcessTool { limits }
    }
}

impl Tool for ProcessTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "process".into(),
            description: "List running processes or manage background tasks. Use 'list' to see what's running, 'spawn' to start a background command, 'kill' to stop a process by PID.".into(),
            safety: ToolSafety::Destructive,
            parameters: vec![
                ToolParam {
                    name: "action".into(),
                    description: "One of: list, spawn, kill".into(),
                    required: true,
                    param_type: "string".into(),
                },
                ToolParam {
                    name: "command".into(),
                    description: "Command to run (required for spawn).".into(),
                    required: false,
                    param_type: "string".into(),
                },
                ToolParam {
                    name: "pid".into(),
                    description: "Process ID to kill (required for kill).".into(),
                    required: false,
                    param_type: "number".into(),
                },
            ],
        }
    }

    fn execute(&self, args: serde_json::Value) -> Result<ToolResult, String> {
        let action = args["action"]
            .as_str()
            .ok_or("Missing required arg: action (list|spawn|kill)")?;

        match action {
            "list" => self.list(),
            "spawn" => {
                let cmd = args["command"]
                    .as_str()
                    .ok_or("Missing required arg: command")?;
                self.spawn(cmd)
            }
            "kill" => {
                let pid = args["pid"]
                    .as_u64()
                    .ok_or("Missing required arg: pid")?;
                self.kill(pid)
            }
            _ => Err(format!("Unknown process action: {}", action)),
        }
    }

    fn safety(&self) -> ToolSafety {
        ToolSafety::Destructive
    }
}

impl ProcessTool {
    fn list(&self) -> Result<ToolResult, String> {
        let output = if cfg!(target_os = "windows") {
            Command::new("tasklist")
                .args(["/NH", "/FO", "CSV"])
                .output()
        } else {
            Command::new("ps")
                .args(["aux", "--sort=-%mem"])
                .output()
        };

        match output {
            Ok(out) => {
                let text = String::from_utf8_lossy(&out.stdout);
                let lines: Vec<&str> = text.lines().take(30).collect();
                Ok(ToolResult {
                    ok: true,
                    output: Some(lines.join("\n")),
                    error: None,
                    media: None,
                    elapsed_ms: 0,
                })
            }
            Err(e) => Ok(ToolResult {
                ok: false,
                output: None,
                error: Some(format!("Failed to list processes: {}", e)),
                media: None,
                elapsed_ms: 0,
            }),
        }
    }

    fn spawn(&self, command: &str) -> Result<ToolResult, String> {
        // Blocklist
        let dangerous = ["fork bomb", ":(){", "while true", "yes "];
        for p in &dangerous {
            if command.contains(p) {
                return Ok(ToolResult {
                    ok: false,
                    output: None,
                    error: Some("Blocked potentially infinite command".into()),
                    media: None,
                    elapsed_ms: 0,
                });
            }
        }

        let child = if cfg!(target_os = "windows") {
            Command::new("cmd")
                .args(["/C", "start", "/B", command])
                .spawn()
        } else {
            Command::new("sh")
                .args(["-c", command])
                .spawn()
        };

        match child {
            Ok(c) => Ok(ToolResult {
                ok: true,
                output: Some(format!("Spawned process PID {}", c.id())),
                error: None,
                media: None,
                elapsed_ms: 0,
            }),
            Err(e) => Ok(ToolResult {
                ok: false,
                output: None,
                error: Some(format!("Failed to spawn: {}", e)),
                media: None,
                elapsed_ms: 0,
            }),
        }
    }

    fn kill(&self, pid: u64) -> Result<ToolResult, String> {
        let result = if cfg!(target_os = "windows") {
            Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/F"])
                .output()
        } else {
            Command::new("kill")
                .args(["-9", &pid.to_string()])
                .output()
        };

        match result {
            Ok(_) => Ok(ToolResult {
                ok: true,
                output: Some(format!("Killed process {}", pid)),
                error: None,
                media: None,
                elapsed_ms: 0,
            }),
            Err(e) => Ok(ToolResult {
                ok: false,
                output: None,
                error: Some(format!("Failed to kill PID {}: {}", pid, e)),
                media: None,
                elapsed_ms: 0,
            }),
        }
    }
}
