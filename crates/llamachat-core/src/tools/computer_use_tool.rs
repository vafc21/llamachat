//! The `computer` tool the model actually calls.
//!
//! This is the join between the three pieces that already existed separately:
//! [`computer_use`](crate::computer_use) parses and rescales the action,
//! [`control`](crate::control) performs it, and [`vision`](crate::vision)
//! encodes the resulting screenshot so the model can see what it did.
//!
//! The loop is the same one Anthropic's computer use runs: **screenshot →
//! decide → act → screenshot**. Every non-look action returns a fresh capture,
//! because a model that acts blind and is never shown the result will keep
//! acting on a stale picture.

use crate::computer_use::{Action, VirtualDisplay};
use crate::control::Controller;
use crate::tools::{Tool, ToolInfo, ToolParam, ToolResult, ToolSafety};

pub struct ComputerUseTool {
    /// `Tool` is `&self` + `Sync`, so the controller lives behind a lock.
    controller: std::sync::Mutex<Box<dyn Controller + Send>>,
    display: VirtualDisplay,
    /// Where captures are written. One reused path — the model is shown the
    /// image, not the file, so history isn't worth keeping.
    shot_path: String,
}

impl ComputerUseTool {
    /// Connect to the desktop and size the virtual display to it.
    pub fn new(controller: Box<dyn Controller + Send>) -> Result<Self, String> {
        let display = controller.virtual_display()?;
        Ok(ComputerUseTool {
            controller: std::sync::Mutex::new(controller),
            display,
            shot_path: std::env::temp_dir()
                .join("llamachat-screen.png")
                .to_string_lossy()
                .into_owned(),
        })
    }

    /// The coordinate space the model is told to work in.
    pub fn display(&self) -> VirtualDisplay {
        self.display
    }

    /// Capture and hand back the path, for the caller to encode.
    fn capture(&self) -> Result<String, String> {
        self.controller.lock().map_err(|_| "controller lock poisoned")?.screenshot(&self.shot_path)?;
        Ok(self.shot_path.clone())
    }
}

impl Tool for ComputerUseTool {
    fn info(&self) -> ToolInfo {
        let (w, h) = (self.display.width, self.display.height);
        ToolInfo {
            name: "computer".into(),
            description: format!(
                "Control this computer by looking at the screen and using the mouse and keyboard. \
                 The screen is {w}x{h}. All coordinates are in that space, with [0, 0] at the \
                 top-left. Take a screenshot first to see what is there, then act. \
                 Actions: screenshot, left_click, right_click, middle_click, double_click, \
                 triple_click, mouse_move, left_click_drag, left_mouse_down, left_mouse_up, \
                 scroll, key, hold_key, type, cursor_position, wait, zoom. \
                 Pass a position as \"coordinate\": [x, y]; text to type or a key combination \
                 like \"ctrl+s\" as \"text\"."
            ),
            safety: ToolSafety::Destructive,
            parameters: vec![
                ToolParam {
                    name: "action".into(),
                    description: "Which action to take.".into(),
                    required: true,
                    param_type: "string".into(),
                },
                ToolParam {
                    name: "coordinate".into(),
                    description: format!("[x, y] within {w}x{h}."),
                    required: false,
                    param_type: "array".into(),
                },
                ToolParam {
                    name: "text".into(),
                    description: "Text to type, or a key combination.".into(),
                    required: false,
                    param_type: "string".into(),
                },
                ToolParam {
                    name: "scroll_direction".into(),
                    description: "up | down | left | right".into(),
                    required: false,
                    param_type: "string".into(),
                },
                ToolParam {
                    name: "scroll_amount".into(),
                    description: "Number of wheel clicks.".into(),
                    required: false,
                    param_type: "number".into(),
                },
            ],
        }
    }

    fn execute(&self, args: serde_json::Value) -> Result<ToolResult, String> {
        let started = std::time::Instant::now();
        let action = match Action::parse(&args) {
            Ok(a) => a,
            Err(e) => {
                return Ok(ToolResult {
                    ok: false,
                    output: None,
                    error: Some(e),
                    media: None,
                    elapsed_ms: started.elapsed().as_millis() as u64,
                })
            }
        };

        let described = action.describe();

        // Report the cursor back in the model's space, not real pixels — it has
        // no idea the real screen is bigger.
        if matches!(action, Action::CursorPosition) {
            let res = self
                .controller
                .lock()
                .map_err(|_| "controller lock poisoned".to_string())?
                .cursor()
                .map(|(x, y)| self.display.to_virtual(x, y));
            return Ok(match res {
                Ok((x, y)) => ToolResult {
                    ok: true,
                    output: Some(format!("cursor is at {x}, {y}")),
                    error: None,
                    media: None,
                    elapsed_ms: started.elapsed().as_millis() as u64,
                },
                Err(e) => ToolResult { ok: false, output: None, error: Some(e), media: None, elapsed_ms: 0 },
            });
        }

        // Rescale into real pixels exactly once, right before acting.
        let real = action.to_real(&self.display);
        let performed = self
            .controller
            .lock()
            .map_err(|_| "controller lock poisoned".to_string())?
            .perform(&real);
        if let Err(e) = performed {
            return Ok(ToolResult {
                ok: false,
                output: None,
                error: Some(format!("couldn't {described}: {e}")),
                media: None,
                elapsed_ms: started.elapsed().as_millis() as u64,
            });
        }

        // Every action hands back a fresh screenshot — that is what closes the
        // loop and stops the model reasoning about a screen it can no longer see.
        let (media, note) = match self.capture() {
            Ok(p) => (Some(p), String::new()),
            Err(e) => (None, format!(" (couldn't capture the screen afterwards: {e})")),
        };

        Ok(ToolResult {
            ok: true,
            output: Some(format!("did: {described}{note}")),
            error: None,
            media,
            elapsed_ms: started.elapsed().as_millis() as u64,
        })
    }

    fn safety(&self) -> ToolSafety {
        ToolSafety::Destructive
    }
}
