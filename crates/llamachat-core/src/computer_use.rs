//! Computer use — the same tool surface Claude and ChatGPT's agents drive,
//! pointed at the local machine.
//!
//! Anthropic's `computer` tool is declared with a `display_width_px` /
//! `display_height_px`, and **every coordinate the model emits is in that
//! space**, not in real screen pixels. The harness is responsible for scaling
//! the screenshot down into that space on the way out, and scaling coordinates
//! back up to real pixels on the way in. Get that mapping wrong and every click
//! lands somewhere else — the model is not wrong, the harness is.
//!
//! That mapping is what this module owns. [`VirtualDisplay`] is the contract
//! between the picture the model sees and the pixels we actually click.
//!
//! The action set mirrors Anthropic's `computer_20250124` / `computer_20251124`
//! exactly, so a model prompted for "computer use" behaves the same here as it
//! does against their API — it will just take longer, because the model is
//! running on this machine instead of in a datacentre.

use serde::{Deserialize, Serialize};

/// Longest edge of the virtual display handed to the model.
///
/// Anthropic's reference implementation and docs use XGA (1024×768). Going
/// higher costs tokens and, past roughly this point, *hurts* pointing accuracy
/// rather than helping it — the model was trained against screenshots in this
/// neighbourhood.
pub const VIRTUAL_MAX_EDGE: u32 = 1024;

/// The mapping between what the model sees and what the mouse actually does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualDisplay {
    /// Real screen size in physical pixels.
    pub real_width: u32,
    pub real_height: u32,
    /// Size the model is told about, and which its coordinates refer to.
    pub width: u32,
    pub height: u32,
}

impl VirtualDisplay {
    /// Fit a real display into the virtual coordinate space, preserving aspect
    /// ratio. A display already smaller than the cap is passed through 1:1.
    pub fn fit(real_width: u32, real_height: u32) -> Self {
        let real_width = real_width.max(1);
        let real_height = real_height.max(1);
        let longest = real_width.max(real_height);
        if longest <= VIRTUAL_MAX_EDGE {
            return VirtualDisplay { real_width, real_height, width: real_width, height: real_height };
        }
        let scale = VIRTUAL_MAX_EDGE as f64 / longest as f64;
        VirtualDisplay {
            real_width,
            real_height,
            // round, don't truncate: truncating biases every mapping up-left.
            width: ((real_width as f64 * scale).round() as u32).max(1),
            height: ((real_height as f64 * scale).round() as u32).max(1),
        }
    }

    /// Is the model's view the same size as the screen?
    pub fn is_identity(&self) -> bool {
        self.width == self.real_width && self.height == self.real_height
    }

    /// Model coordinate → real screen pixel.
    ///
    /// Aims at the *centre* of the source pixel block rather than its corner.
    /// At a 2.5× scale factor a corner-aligned mapping is consistently off by
    /// more than a pixel toward the top-left, which is enough to miss the edge
    /// of a small control.
    pub fn to_real(&self, x: i32, y: i32) -> (i32, i32) {
        let sx = self.real_width as f64 / self.width as f64;
        let sy = self.real_height as f64 / self.height as f64;
        let rx = ((x as f64 + 0.5) * sx).floor() as i32;
        let ry = ((y as f64 + 0.5) * sy).floor() as i32;
        (
            rx.clamp(0, self.real_width as i32 - 1),
            ry.clamp(0, self.real_height as i32 - 1),
        )
    }

    /// Real screen pixel → model coordinate. Used to report the cursor position
    /// back in terms the model understands.
    pub fn to_virtual(&self, x: i32, y: i32) -> (i32, i32) {
        let sx = self.width as f64 / self.real_width as f64;
        let sy = self.height as f64 / self.real_height as f64;
        (
            ((x as f64 * sx).floor() as i32).clamp(0, self.width as i32 - 1),
            ((y as f64 * sy).floor() as i32).clamp(0, self.height as i32 - 1),
        )
    }
}

/// Which way a scroll goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

/// One computer-use action, named exactly as the model emits it.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Screenshot,
    CursorPosition,
    MouseMove { x: i32, y: i32 },
    LeftClick { x: i32, y: i32 },
    RightClick { x: i32, y: i32 },
    MiddleClick { x: i32, y: i32 },
    DoubleClick { x: i32, y: i32 },
    TripleClick { x: i32, y: i32 },
    LeftMouseDown,
    LeftMouseUp,
    LeftClickDrag { from: (i32, i32), to: (i32, i32) },
    Key { combo: String },
    HoldKey { combo: String, seconds: f64 },
    Type { text: String },
    Scroll { x: i32, y: i32, direction: ScrollDirection, amount: i32 },
    Wait { seconds: f64 },
    /// Re-crop the last screenshot to a region at full resolution, so the model
    /// can read small text without us sending the whole screen at 1:1.
    Zoom { region: (i32, i32, i32, i32) },
}

fn coord(v: &serde_json::Value, key: &str) -> Option<(i32, i32)> {
    let a = v.get(key)?.as_array()?;
    if a.len() < 2 {
        return None;
    }
    Some((a[0].as_i64()? as i32, a[1].as_i64()? as i32))
}

impl Action {
    /// Parse one action out of the model's tool call.
    ///
    /// Small local models are sloppier than Claude, so a few forgiving aliases
    /// are accepted (`click` for `left_click`, `text` or `key` for the key
    /// combo). Anything genuinely unknown is an error rather than a guess —
    /// silently doing the wrong thing to someone's desktop is worse than
    /// refusing.
    pub fn parse(v: &serde_json::Value) -> Result<Action, String> {
        let raw = v.get("action").and_then(|a| a.as_str()).unwrap_or("").trim().to_ascii_lowercase();
        let xy = |name: &str| -> Result<(i32, i32), String> {
            coord(v, "coordinate")
                .or_else(|| Some((v.get("x")?.as_i64()? as i32, v.get("y")?.as_i64()? as i32)))
                .ok_or_else(|| format!("`{name}` needs a `coordinate` of [x, y]"))
        };
        let text = || -> Result<String, String> {
            v.get("text")
                .and_then(|t| t.as_str())
                .map(str::to_string)
                .ok_or_else(|| "`type` needs `text`".to_string())
        };
        let combo = || -> Result<String, String> {
            v.get("text")
                .or_else(|| v.get("key"))
                .and_then(|t| t.as_str())
                .map(str::to_string)
                .ok_or_else(|| "`key` needs the combination in `text`".to_string())
        };

        Ok(match raw.as_str() {
            "screenshot" => Action::Screenshot,
            "cursor_position" => Action::CursorPosition,
            "mouse_move" => { let (x, y) = xy("mouse_move")?; Action::MouseMove { x, y } }
            "left_click" | "click" => { let (x, y) = xy("left_click")?; Action::LeftClick { x, y } }
            "right_click" => { let (x, y) = xy("right_click")?; Action::RightClick { x, y } }
            "middle_click" => { let (x, y) = xy("middle_click")?; Action::MiddleClick { x, y } }
            "double_click" => { let (x, y) = xy("double_click")?; Action::DoubleClick { x, y } }
            "triple_click" => { let (x, y) = xy("triple_click")?; Action::TripleClick { x, y } }
            "left_mouse_down" => Action::LeftMouseDown,
            "left_mouse_up" => Action::LeftMouseUp,
            "left_click_drag" => {
                let to = xy("left_click_drag")?;
                let from = coord(v, "start_coordinate")
                    .ok_or("`left_click_drag` needs `start_coordinate`")?;
                Action::LeftClickDrag { from, to }
            }
            "key" | "keypress" | "press" => Action::Key { combo: combo()? },
            "hold_key" => Action::HoldKey {
                combo: combo()?,
                seconds: v.get("duration").and_then(|d| d.as_f64()).unwrap_or(1.0),
            },
            "type" | "write" => Action::Type { text: text()? },
            "scroll" => {
                let (x, y) = xy("scroll")?;
                let direction = match v
                    .get("scroll_direction")
                    .and_then(|d| d.as_str())
                    .unwrap_or("down")
                    .to_ascii_lowercase()
                    .as_str()
                {
                    "up" => ScrollDirection::Up,
                    "left" => ScrollDirection::Left,
                    "right" => ScrollDirection::Right,
                    _ => ScrollDirection::Down,
                };
                Action::Scroll {
                    x,
                    y,
                    direction,
                    amount: v.get("scroll_amount").and_then(|a| a.as_i64()).unwrap_or(3) as i32,
                }
            }
            "wait" => Action::Wait {
                seconds: v.get("duration").and_then(|d| d.as_f64()).unwrap_or(1.0).clamp(0.0, 30.0),
            },
            "zoom" => {
                let r = v.get("region").and_then(|r| r.as_array()).ok_or("`zoom` needs a `region`")?;
                if r.len() < 4 {
                    return Err("`zoom` region must be [x1, y1, x2, y2]".into());
                }
                Action::Zoom {
                    region: (
                        r[0].as_i64().unwrap_or(0) as i32,
                        r[1].as_i64().unwrap_or(0) as i32,
                        r[2].as_i64().unwrap_or(0) as i32,
                        r[3].as_i64().unwrap_or(0) as i32,
                    ),
                }
            }
            "" => return Err("no `action` given".into()),
            other => return Err(format!("unknown action `{other}`")),
        })
    }

    /// Does this action move or press anything? Read-only actions can skip the
    /// approval prompt; anything that touches the desktop should not.
    pub fn is_read_only(&self) -> bool {
        matches!(self, Action::Screenshot | Action::CursorPosition | Action::Zoom { .. } | Action::Wait { .. })
    }

    /// One-line description for the transcript and the approval prompt, in the
    /// model's coordinate space.
    pub fn describe(&self) -> String {
        match self {
            Action::Screenshot => "take a screenshot".into(),
            Action::CursorPosition => "read the cursor position".into(),
            Action::MouseMove { x, y } => format!("move the pointer to {x}, {y}"),
            Action::LeftClick { x, y } => format!("click at {x}, {y}"),
            Action::RightClick { x, y } => format!("right-click at {x}, {y}"),
            Action::MiddleClick { x, y } => format!("middle-click at {x}, {y}"),
            Action::DoubleClick { x, y } => format!("double-click at {x}, {y}"),
            Action::TripleClick { x, y } => format!("triple-click at {x}, {y}"),
            Action::LeftMouseDown => "press and hold the left button".into(),
            Action::LeftMouseUp => "release the left button".into(),
            Action::LeftClickDrag { from, to } => {
                format!("drag from {}, {} to {}, {}", from.0, from.1, to.0, to.1)
            }
            Action::Key { combo } => format!("press {combo}"),
            Action::HoldKey { combo, seconds } => format!("hold {combo} for {seconds}s"),
            Action::Type { text } => {
                let t: String = text.chars().take(40).collect();
                format!("type \u{201c}{t}{}\u{201d}", if text.chars().count() > 40 { "…" } else { "" })
            }
            Action::Scroll { x, y, direction, amount } => {
                format!("scroll {direction:?} {amount} at {x}, {y}").to_lowercase()
            }
            Action::Wait { seconds } => format!("wait {seconds}s"),
            Action::Zoom { region } => {
                format!("zoom into {}, {} – {}, {}", region.0, region.1, region.2, region.3)
            }
        }
    }

    /// Rewrite this action's coordinates from the model's space into real
    /// screen pixels. Call this exactly once, immediately before execution.
    pub fn to_real(&self, d: &VirtualDisplay) -> Action {
        let m = |x: &i32, y: &i32| d.to_real(*x, *y);
        match self {
            Action::MouseMove { x, y } => { let (x, y) = m(x, y); Action::MouseMove { x, y } }
            Action::LeftClick { x, y } => { let (x, y) = m(x, y); Action::LeftClick { x, y } }
            Action::RightClick { x, y } => { let (x, y) = m(x, y); Action::RightClick { x, y } }
            Action::MiddleClick { x, y } => { let (x, y) = m(x, y); Action::MiddleClick { x, y } }
            Action::DoubleClick { x, y } => { let (x, y) = m(x, y); Action::DoubleClick { x, y } }
            Action::TripleClick { x, y } => { let (x, y) = m(x, y); Action::TripleClick { x, y } }
            Action::LeftClickDrag { from, to } => Action::LeftClickDrag {
                from: d.to_real(from.0, from.1),
                to: d.to_real(to.0, to.1),
            },
            Action::Scroll { x, y, direction, amount } => {
                let (x, y) = m(x, y);
                Action::Scroll { x, y, direction: *direction, amount: *amount }
            }
            Action::Zoom { region } => {
                let (x1, y1) = d.to_real(region.0, region.1);
                let (x2, y2) = d.to_real(region.2, region.3);
                Action::Zoom { region: (x1, y1, x2, y2) }
            }
            other => other.clone(),
        }
    }
}

/// The tool declaration handed to the model, mirroring Anthropic's shape so a
/// model trained on computer use recognises it.
pub fn tool_declaration(d: &VirtualDisplay) -> serde_json::Value {
    serde_json::json!({
        "type": "computer_20250124",
        "name": "computer",
        "display_width_px": d.width,
        "display_height_px": d.height,
        "display_number": 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_4k_display_fits_into_the_virtual_space_keeping_aspect() {
        let d = VirtualDisplay::fit(3840, 2160);
        assert_eq!((d.width, d.height), (1024, 576));
        assert!(!d.is_identity());
    }

    #[test]
    fn small_displays_map_one_to_one() {
        let d = VirtualDisplay::fit(800, 600);
        assert_eq!((d.width, d.height), (800, 600));
        assert!(d.is_identity());
        assert_eq!(d.to_real(400, 300), (400, 300));
    }

    #[test]
    fn corners_map_inside_the_real_screen() {
        let d = VirtualDisplay::fit(3840, 2160);
        assert_eq!(d.to_real(0, 0), (1, 1));
        let (x, y) = d.to_real(d.width as i32 - 1, d.height as i32 - 1);
        assert!(x < 3840 && y < 2160, "bottom-right {x},{y} must stay on screen");
        assert!(x > 3800 && y > 2140, "bottom-right {x},{y} should be near the corner");
    }

    #[test]
    fn a_centre_click_lands_in_the_centre() {
        let d = VirtualDisplay::fit(2560, 1440);
        let (x, y) = d.to_real(d.width as i32 / 2, d.height as i32 / 2);
        assert!((x - 1280).abs() <= 2, "x was {x}");
        assert!((y - 720).abs() <= 2, "y was {y}");
    }

    #[test]
    fn virtual_and_real_round_trip_within_a_pixel_block() {
        let d = VirtualDisplay::fit(1920, 1080);
        for (vx, vy) in [(0, 0), (10, 10), (511, 287), (1023, 575)] {
            let (rx, ry) = d.to_real(vx, vy);
            let (bx, by) = d.to_virtual(rx, ry);
            assert!((bx - vx).abs() <= 1 && (by - vy).abs() <= 1, "{vx},{vy} -> {rx},{ry} -> {bx},{by}");
        }
    }

    #[test]
    fn parses_the_anthropic_action_shapes() {
        let p = |j: serde_json::Value| Action::parse(&j).unwrap();
        assert_eq!(p(serde_json::json!({"action": "screenshot"})), Action::Screenshot);
        assert_eq!(
            p(serde_json::json!({"action": "left_click", "coordinate": [500, 300]})),
            Action::LeftClick { x: 500, y: 300 }
        );
        assert_eq!(
            p(serde_json::json!({"action": "type", "text": "Hello, world!"})),
            Action::Type { text: "Hello, world!".into() }
        );
        assert_eq!(
            p(serde_json::json!({"action": "key", "text": "ctrl+s"})),
            Action::Key { combo: "ctrl+s".into() }
        );
        assert_eq!(
            p(serde_json::json!({"action": "scroll", "coordinate": [500, 400],
                                 "scroll_direction": "down", "scroll_amount": 3})),
            Action::Scroll { x: 500, y: 400, direction: ScrollDirection::Down, amount: 3 }
        );
        assert_eq!(
            p(serde_json::json!({"action": "left_click_drag", "start_coordinate": [10, 20],
                                 "coordinate": [30, 40]})),
            Action::LeftClickDrag { from: (10, 20), to: (30, 40) }
        );
        assert_eq!(
            p(serde_json::json!({"action": "zoom", "region": [0, 0, 100, 80]})),
            Action::Zoom { region: (0, 0, 100, 80) }
        );
    }

    #[test]
    fn unknown_actions_are_refused_not_guessed() {
        assert!(Action::parse(&serde_json::json!({"action": "self_destruct"})).is_err());
        assert!(Action::parse(&serde_json::json!({})).is_err());
        // A click with no coordinate must fail rather than default to 0,0.
        assert!(Action::parse(&serde_json::json!({"action": "left_click"})).is_err());
    }

    #[test]
    fn only_looking_is_read_only() {
        assert!(Action::Screenshot.is_read_only());
        assert!(Action::Zoom { region: (0, 0, 1, 1) }.is_read_only());
        assert!(!Action::LeftClick { x: 1, y: 1 }.is_read_only());
        assert!(!Action::Type { text: "rm -rf".into() }.is_read_only());
    }

    #[test]
    fn actions_are_rescaled_before_execution() {
        let d = VirtualDisplay::fit(2560, 1440);
        let scaled = Action::LeftClick { x: 512, y: 288 }.to_real(&d);
        match scaled {
            Action::LeftClick { x, y } => {
                assert!((x - 1280).abs() <= 3, "x {x}");
                assert!((y - 720).abs() <= 3, "y {y}");
            }
            other => panic!("wrong variant: {other:?}"),
        }
        // Actions without coordinates pass through untouched.
        assert_eq!(Action::Screenshot.to_real(&d), Action::Screenshot);
    }

    #[test]
    fn the_declaration_reports_the_virtual_size_not_the_real_one() {
        let d = VirtualDisplay::fit(3840, 2160);
        let decl = tool_declaration(&d);
        assert_eq!(decl["display_width_px"], 1024);
        assert_eq!(decl["display_height_px"], 576);
        assert_eq!(decl["name"], "computer");
    }
}
