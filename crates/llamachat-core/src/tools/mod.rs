//! Tool engine — gives the assistant shell, filesystem, browser, and process
//! access. Each tool has a safety profile: some are read-only and auto-approved,
//! some require user confirmation per invocation, and all are resource-capped.

pub mod shell;
pub mod filesystem;
pub mod process;
pub mod desktop;
pub mod computer;

pub use shell::ShellTool;
pub use filesystem::FilesystemTool;
pub use process::ProcessTool;
pub use desktop::DesktopTool;
pub use computer::ComputerTool;

use serde::{Deserialize, Serialize};
use std::time::Duration;

// ── Tool definition ────────────────────────────────────────────

/// Safety level for a tool invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolSafety {
    /// Read-only, no side effects, auto-approved (e.g. read file, list processes).
    ReadOnly,
    /// Creates or modifies but is reversible / low-risk (e.g. write new file).
    Write,
    /// Potentially destructive — requires explicit user approval (e.g. shell exec, delete).
    Destructive,
}

/// A single tool invocation request from the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRequest {
    pub name: String,
    pub args: serde_json::Value,
}

/// The result of executing a tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Optional media path (screenshot, image, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media: Option<String>,
    /// Execution time in ms.
    pub elapsed_ms: u64,
}

/// Metadata about a tool, used for registration and the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub safety: ToolSafety,
    pub parameters: Vec<ToolParam>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParam {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub param_type: String, // "string", "number", "boolean"
}

// ── Tool trait ─────────────────────────────────────────────────

/// Every tool implements this trait.
pub trait Tool: Send + Sync {
    fn info(&self) -> ToolInfo;
    fn execute(&self, args: serde_json::Value) -> Result<ToolResult, String>;
    fn safety(&self) -> ToolSafety;
}

// ── Execution limits ───────────────────────────────────────────

/// Global safety caps applied to all tool executions.
#[derive(Debug, Clone)]
pub struct ToolLimits {
    /// Max shell command output bytes.
    pub max_shell_output: usize,
    /// Max file read size in bytes.
    pub max_file_read: usize,
    /// Max file write size in bytes.
    pub max_file_write: usize,
    /// Shell execution timeout.
    pub shell_timeout: Duration,
    /// Browser action timeout.
    pub browser_timeout: Duration,
    /// Process spawn timeout (before auto-kill).
    pub process_timeout: Duration,
}

impl Default for ToolLimits {
    fn default() -> Self {
        ToolLimits {
            max_shell_output: 256 * 1024,      // 256KB
            max_file_read: 10 * 1024 * 1024,    // 10MB
            max_file_write: 50 * 1024 * 1024,   // 50MB
            shell_timeout: Duration::from_secs(30),
            browser_timeout: Duration::from_secs(15),
            process_timeout: Duration::from_secs(300), // 5 min
        }
    }
}

// ── Tool registry ──────────────────────────────────────────────

/// Holds all registered tools and enforces safety policies.
pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
    limits: ToolLimits,
    /// Whether destructive tools are allowed at all.
    destructive_allowed: bool,
    /// Pending approval: tool name → whether approved.
    pending_approval: Option<String>,
}

impl ToolRegistry {
    pub fn new(limits: ToolLimits, destructive_allowed: bool) -> Self {
        ToolRegistry {
            tools: Vec::new(),
            limits,
            destructive_allowed,
            pending_approval: None,
        }
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.push(tool);
    }

    /// Allow or forbid destructive tools (kept in sync with the user's consent).
    pub fn set_destructive_allowed(&mut self, allowed: bool) {
        self.destructive_allowed = allowed;
    }

    pub fn list_tools(&self) -> Vec<ToolInfo> {
        self.tools.iter().map(|t| t.info()).collect()
    }

    /// Find a tool by name.
    pub fn find(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.iter().find(|t| t.info().name == name).map(|t| t.as_ref())
    }

    /// Check if a tool invocation needs user approval before executing.
    pub fn needs_approval(&self, name: &str) -> bool {
        if let Some(tool) = self.find(name) {
            match tool.safety() {
                ToolSafety::ReadOnly | ToolSafety::Write => false,
                ToolSafety::Destructive => true,
            }
        } else {
            true // unknown tool = needs approval
        }
    }

    /// Execute a tool, respecting safety constraints.
    pub fn execute(&self, request: &ToolRequest) -> ToolResult {
        let start = std::time::Instant::now();

        let tool = match self.find(&request.name) {
            Some(t) => t,
            None => {
                return ToolResult {
                    ok: false,
                    output: None,
                    error: Some(format!("Unknown tool: {}", request.name)),
                    media: None,
                    elapsed_ms: start.elapsed().as_millis() as u64,
                };
            }
        };

        // Check destructive tool permission
        if tool.safety() == ToolSafety::Destructive && !self.destructive_allowed {
            return ToolResult {
                ok: false,
                output: None,
                error: Some("Destructive tools are disabled. Enable in settings.".into()),
                media: None,
                elapsed_ms: start.elapsed().as_millis() as u64,
            };
        }

        match tool.execute(request.args.clone()) {
            Ok(mut result) => {
                result.elapsed_ms = start.elapsed().as_millis() as u64;
                result
            }
            Err(e) => ToolResult {
                ok: false,
                output: None,
                error: Some(e),
                media: None,
                elapsed_ms: start.elapsed().as_millis() as u64,
            },
        }
    }

    /// Generate a system prompt describing available tools for the model.
    pub fn system_prompt(&self) -> String {
        let mut s = String::from(
            "You have access to the following tools. To use a tool, output a JSON object with \"tool\" and \"args\":\n\n",
        );
        for info in &self.list_tools() {
            s.push_str(&format!("## {}\n{}\n", info.name, info.description));
            if !info.parameters.is_empty() {
                s.push_str("Parameters:\n");
                for p in &info.parameters {
                    let req = if p.required { "(required)" } else { "(optional)" };
                    s.push_str(&format!(
                        "  - {}: {} ({}) {}\n",
                        p.name, p.param_type, req, p.description
                    ));
                }
            }
            s.push('\n');
        }
        s.push_str("\nRespond with tool calls like:\n{\"tool\": \"shell\", \"args\": {\"command\": \"ls -la\"}}\n");
        s.push_str("You can use multiple tools in sequence. After tool results, continue your response.\n");
        s
    }
}
