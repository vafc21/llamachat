//! The hands: actually moving the pointer and pressing keys.
//!
//! [`computer_use`](crate::computer_use) decides *what* to do and in which
//! coordinate space; this module does it. The split matters because the
//! coordinate maths is testable anywhere, while the execution needs a real
//! display.
//!
//! On Linux this talks X11 directly over `x11rb` using the **XTEST** extension,
//! rather than shelling out to `xdotool`. That means no runtime dependency to
//! install, it works against a headless `Xvfb` the same as a real desktop, and
//! errors come back as values instead of a parsed stderr string.

use crate::computer_use::{Action, ScrollDirection, VirtualDisplay};

/// What a backend can tell us about the screen it drives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Screen {
    pub width: u32,
    pub height: u32,
}

/// A thing that can drive a desktop.
pub trait Controller {
    /// Physical size of the display being driven.
    fn screen(&self) -> Result<Screen, String>;
    /// Where the pointer is now, in real pixels.
    fn cursor(&self) -> Result<(i32, i32), String>;
    /// Run one action. Coordinates must already be in **real pixels** — call
    /// [`Action::to_real`] first.
    fn perform(&mut self, action: &Action) -> Result<(), String>;
    /// Grab the whole screen and write it to `path` as PNG.
    fn screenshot(&self, path: &str) -> Result<(), String>;

    /// Build the virtual display that matches this screen.
    fn virtual_display(&self) -> Result<VirtualDisplay, String> {
        let s = self.screen()?;
        Ok(VirtualDisplay::fit(s.width, s.height))
    }
}

// ── X11 ────────────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
pub mod x11 {
    use super::*;
    use std::collections::HashMap;
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConnectionExt as _, ImageFormat, Screen as XScreen};
    use x11rb::protocol::xtest::ConnectionExt as _;
    use x11rb::rust_connection::RustConnection;

    // XTEST event types, from the core X protocol.
    const KEY_PRESS: u8 = 2;
    const KEY_RELEASE: u8 = 3;
    const BUTTON_PRESS: u8 = 4;
    const BUTTON_RELEASE: u8 = 5;
    const MOTION_NOTIFY: u8 = 6;

    /// Buttons 4/5 are wheel up/down and 6/7 are horizontal — X has no separate
    /// scroll event, a scroll *is* a button press.
    fn wheel_button(d: ScrollDirection) -> u8 {
        match d {
            ScrollDirection::Up => 4,
            ScrollDirection::Down => 5,
            ScrollDirection::Left => 6,
            ScrollDirection::Right => 7,
        }
    }

    pub struct X11Controller {
        conn: RustConnection,
        root: u32,
        width: u32,
        height: u32,
        /// keysym -> (keycode, needs_shift), built once from the server's map.
        keys: HashMap<u32, (u8, bool)>,
    }

    impl X11Controller {
        /// Connect to `display` (or `$DISPLAY` when `None`).
        pub fn open(display: Option<&str>) -> Result<Self, String> {
            let (conn, screen_num) =
                x11rb::connect(display).map_err(|e| format!("Can't reach the X display: {e}"))?;
            let screen: &XScreen = &conn.setup().roots[screen_num];
            let (root, width, height) = (
                screen.root,
                screen.width_in_pixels as u32,
                screen.height_in_pixels as u32,
            );

            // XTEST has to actually be present; without it fake_input is a no-op
            // that silently does nothing, which is the worst possible failure.
            conn.xtest_get_version(2, 2)
                .map_err(|e| format!("XTEST unavailable: {e}"))?
                .reply()
                .map_err(|e| format!("XTEST unavailable: {e}"))?;

            let mut me = X11Controller { conn, root, width, height, keys: HashMap::new() };
            me.load_keymap()?;
            Ok(me)
        }

        /// Build keysym → keycode once. Typing needs to go from a character to
        /// whatever physical key produces it on *this* layout.
        fn load_keymap(&mut self) -> Result<(), String> {
            let setup = self.conn.setup();
            let (min, max) = (setup.min_keycode, setup.max_keycode);
            let count = max - min + 1;
            let map = self
                .conn
                .get_keyboard_mapping(min, count)
                .map_err(|e| e.to_string())?
                .reply()
                .map_err(|e| e.to_string())?;
            let per = map.keysyms_per_keycode as usize;
            for (i, chunk) in map.keysyms.chunks(per).enumerate() {
                let keycode = min + i as u8;
                // Index 0 is unshifted, 1 is shifted.
                if let Some(&unshifted) = chunk.first() {
                    if unshifted != 0 {
                        self.keys.entry(unshifted).or_insert((keycode, false));
                    }
                }
                if let Some(&shifted) = chunk.get(1) {
                    if shifted != 0 {
                        self.keys.entry(shifted).or_insert((keycode, true));
                    }
                }
            }
            Ok(())
        }

        fn fake(&self, ty: u8, detail: u8, x: i16, y: i16) -> Result<(), String> {
            self.conn
                .xtest_fake_input(ty, detail, 0, self.root, x, y, 0)
                .map_err(|e| e.to_string())?;
            self.conn.flush().map_err(|e| e.to_string())
        }

        fn move_to(&self, x: i32, y: i32) -> Result<(), String> {
            self.fake(MOTION_NOTIFY, 0, x as i16, y as i16)
        }

        fn click_at(&self, x: i32, y: i32, button: u8, times: u32) -> Result<(), String> {
            self.move_to(x, y)?;
            for _ in 0..times {
                self.fake(BUTTON_PRESS, button, 0, 0)?;
                self.fake(BUTTON_RELEASE, button, 0, 0)?;
            }
            Ok(())
        }

        /// Map one character to a keysym. ASCII maps to itself in X11; anything
        /// else uses the Unicode keysym range.
        fn keysym_for(c: char) -> u32 {
            let u = c as u32;
            if u < 0x80 { u } else { u + 0x0100_0000 }
        }

        fn tap_keysym(&self, keysym: u32) -> Result<(), String> {
            let (code, shift) = *self
                .keys
                .get(&keysym)
                .ok_or_else(|| format!("no key on this layout produces keysym {keysym:#x}"))?;
            // 0xffe1 is Shift_L.
            let shift_code = self.keys.get(&0xffe1).map(|(c, _)| *c);
            if shift {
                if let Some(sc) = shift_code {
                    self.fake(KEY_PRESS, sc, 0, 0)?;
                }
            }
            self.fake(KEY_PRESS, code, 0, 0)?;
            self.fake(KEY_RELEASE, code, 0, 0)?;
            if shift {
                if let Some(sc) = shift_code {
                    self.fake(KEY_RELEASE, sc, 0, 0)?;
                }
            }
            Ok(())
        }

        /// Named keys and `ctrl+alt+x` style combinations.
        fn named_keysym(name: &str) -> Option<u32> {
            Some(match name.trim().to_ascii_lowercase().as_str() {
                "return" | "enter" => 0xff0d,
                "tab" => 0xff09,
                "space" => 0x0020,
                "escape" | "esc" => 0xff1b,
                "backspace" => 0xff08,
                "delete" => 0xffff,
                "home" => 0xff50,
                "end" => 0xff57,
                "page_up" | "pageup" => 0xff55,
                "page_down" | "pagedown" => 0xff56,
                "left" => 0xff51,
                "up" => 0xff52,
                "right" => 0xff53,
                "down" => 0xff54,
                "ctrl" | "control" => 0xffe3,
                "alt" => 0xffe9,
                "shift" => 0xffe1,
                "super" | "cmd" | "meta" => 0xffeb,
                other => {
                    let mut ch = other.chars();
                    let c = ch.next()?;
                    if ch.next().is_some() {
                        return None;
                    }
                    Self::keysym_for(c)
                }
            })
        }

        fn press_combo(&self, combo: &str) -> Result<(), String> {
            let parts: Vec<&str> = combo.split(['+', '-']).filter(|p| !p.is_empty()).collect();
            if parts.is_empty() {
                return Err("empty key combination".into());
            }
            let (last, mods) = parts.split_last().unwrap();

            let mut held = Vec::new();
            for m in mods {
                let sym = Self::named_keysym(m).ok_or_else(|| format!("unknown modifier `{m}`"))?;
                let (code, _) = *self
                    .keys
                    .get(&sym)
                    .ok_or_else(|| format!("no key for modifier `{m}`"))?;
                self.fake(KEY_PRESS, code, 0, 0)?;
                held.push(code);
            }
            let sym = Self::named_keysym(last).ok_or_else(|| format!("unknown key `{last}`"))?;
            let res = self.tap_keysym(sym);
            // Always release modifiers, even if the main key failed — leaving
            // ctrl stuck down would wreck the desktop.
            for code in held.into_iter().rev() {
                self.fake(KEY_RELEASE, code, 0, 0)?;
            }
            res
        }
    }

    impl Controller for X11Controller {
        /// Capture through X11 `GetImage` rather than shelling out to
        /// ImageMagick. One fewer thing to install, and the pixels come back as
        /// data instead of via a temp file we then have to re-read.
        fn screenshot(&self, path: &str) -> Result<(), String> {
            let img = self
                .conn
                .get_image(
                    ImageFormat::Z_PIXMAP,
                    self.root,
                    0,
                    0,
                    self.width as u16,
                    self.height as u16,
                    !0,
                )
                .map_err(|e| format!("GetImage failed: {e}"))?
                .reply()
                .map_err(|e| format!("GetImage failed: {e}"))?;

            let (w, h) = (self.width, self.height);
            let px = (w as usize) * (h as usize);
            // TrueColor X servers hand back Z_PIXMAP as BGRX (4 bytes) at depth
            // 24; some give a packed 3-byte form. Detect from the length rather
            // than assuming, so this doesn't silently produce a sheared image.
            let stride = if img.data.len() >= px * 4 { 4 } else { 3 };
            if img.data.len() < px * stride {
                return Err(format!(
                    "screen data too short: {} bytes for {w}x{h}",
                    img.data.len()
                ));
            }
            let mut rgb = image::RgbImage::new(w, h);
            for y in 0..h {
                for x in 0..w {
                    let i = ((y as usize) * (w as usize) + x as usize) * stride;
                    // X sends blue first.
                    let p = image::Rgb([img.data[i + 2], img.data[i + 1], img.data[i]]);
                    rgb.put_pixel(x, y, p);
                }
            }
            rgb.save(path).map_err(|e| format!("Couldn't write {path}: {e}"))
        }

        fn screen(&self) -> Result<Screen, String> {
            Ok(Screen { width: self.width, height: self.height })
        }

        fn cursor(&self) -> Result<(i32, i32), String> {
            let p = self
                .conn
                .query_pointer(self.root)
                .map_err(|e| e.to_string())?
                .reply()
                .map_err(|e| e.to_string())?;
            Ok((p.root_x as i32, p.root_y as i32))
        }

        fn perform(&mut self, action: &Action) -> Result<(), String> {
            match action {
                Action::Screenshot | Action::CursorPosition | Action::Zoom { .. } => Ok(()),
                Action::Wait { seconds } => {
                    std::thread::sleep(std::time::Duration::from_secs_f64(seconds.clamp(0.0, 30.0)));
                    Ok(())
                }
                Action::MouseMove { x, y } => self.move_to(*x, *y),
                Action::LeftClick { x, y } => self.click_at(*x, *y, 1, 1),
                Action::MiddleClick { x, y } => self.click_at(*x, *y, 2, 1),
                Action::RightClick { x, y } => self.click_at(*x, *y, 3, 1),
                Action::DoubleClick { x, y } => self.click_at(*x, *y, 1, 2),
                Action::TripleClick { x, y } => self.click_at(*x, *y, 1, 3),
                Action::LeftMouseDown => self.fake(BUTTON_PRESS, 1, 0, 0),
                Action::LeftMouseUp => self.fake(BUTTON_RELEASE, 1, 0, 0),
                Action::LeftClickDrag { from, to } => {
                    self.move_to(from.0, from.1)?;
                    self.fake(BUTTON_PRESS, 1, 0, 0)?;
                    // Step the drag so apps that track motion see a path rather
                    // than a teleport; a single jump is ignored by many widgets.
                    for i in 1..=10 {
                        let x = from.0 + (to.0 - from.0) * i / 10;
                        let y = from.1 + (to.1 - from.1) * i / 10;
                        self.move_to(x, y)?;
                    }
                    self.fake(BUTTON_RELEASE, 1, 0, 0)
                }
                Action::Scroll { x, y, direction, amount } => {
                    self.move_to(*x, *y)?;
                    let b = wheel_button(*direction);
                    for _ in 0..(*amount).max(1) {
                        self.fake(BUTTON_PRESS, b, 0, 0)?;
                        self.fake(BUTTON_RELEASE, b, 0, 0)?;
                    }
                    Ok(())
                }
                Action::Key { combo } => self.press_combo(combo),
                Action::HoldKey { combo, seconds } => {
                    let sym = Self::named_keysym(combo).ok_or_else(|| format!("unknown key `{combo}`"))?;
                    let (code, _) = *self.keys.get(&sym).ok_or("no key for that keysym")?;
                    self.fake(KEY_PRESS, code, 0, 0)?;
                    std::thread::sleep(std::time::Duration::from_secs_f64(seconds.clamp(0.0, 10.0)));
                    self.fake(KEY_RELEASE, code, 0, 0)
                }
                Action::Type { text } => {
                    for c in text.chars() {
                        self.tap_keysym(Self::keysym_for(c))?;
                    }
                    Ok(())
                }
            }
        }
    }
}

#[cfg(target_os = "linux")]
pub use x11::X11Controller;

/// Open the best controller for this platform.
///
/// Returns `Err` with a human explanation when there is no desktop to drive —
/// a headless server, or macOS without Accessibility permission. Callers should
/// treat that as "the computer tool is unavailable", not as a crash.
pub fn open() -> Result<Box<dyn Controller + Send>, String> {
    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
            return Err("No desktop session on this machine (no DISPLAY). Computer control is unavailable.".into());
        }
        Ok(Box::new(X11Controller::open(None)?))
    }
    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(MacController::open()?))
    }
    #[cfg(target_os = "windows")]
    {
        Ok(Box::new(WindowsController::open()?))
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        Err("No controller for this platform yet.".into())
    }
}

// ── macOS ──────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
pub mod macos {
    use super::*;
    use core_graphics::display::CGDisplay;
    use core_graphics::event::{
        CGEvent, CGEventTapLocation, CGEventType, CGMouseButton, ScrollEventUnit,
    };
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    use core_graphics::geometry::CGPoint;

    pub struct MacController {
        width: u32,
        height: u32,
    }

    impl MacController {
        pub fn open() -> Result<Self, String> {
            let d = CGDisplay::main();
            Ok(MacController { width: d.pixels_wide() as u32, height: d.pixels_high() as u32 })
        }

        fn source() -> Result<CGEventSource, String> {
            // HIDSystemState posts as though the hardware did it, which is what
            // makes the events reach other applications.
            CGEventSource::new(CGEventSourceStateID::HIDSystemState)
                .map_err(|_| "Couldn't create an event source. Grant Accessibility permission to LlamaChat in System Settings ▸ Privacy & Security.".to_string())
        }

        fn mouse(&self, ty: CGEventType, x: i32, y: i32, button: CGMouseButton) -> Result<(), String> {
            let src = Self::source()?;
            let p = CGPoint::new(x as f64, y as f64);
            let ev = CGEvent::new_mouse_event(src, ty, p, button)
                .map_err(|_| "Couldn't build the mouse event".to_string())?;
            ev.post(CGEventTapLocation::HID);
            Ok(())
        }

        fn click(&self, x: i32, y: i32, button: CGMouseButton, times: u32) -> Result<(), String> {
            let (down, up) = match button {
                CGMouseButton::Left => (CGEventType::LeftMouseDown, CGEventType::LeftMouseUp),
                CGMouseButton::Right => (CGEventType::RightMouseDown, CGEventType::RightMouseUp),
                _ => (CGEventType::OtherMouseDown, CGEventType::OtherMouseUp),
            };
            self.mouse(CGEventType::MouseMoved, x, y, CGMouseButton::Left)?;
            for _ in 0..times {
                self.mouse(down, x, y, button)?;
                self.mouse(up, x, y, button)?;
            }
            Ok(())
        }

        /// Virtual key codes for the keys we name. macOS keycodes are positional,
        /// not character-based, so these are the US-layout physical positions.
        fn keycode(name: &str) -> Option<u16> {
            Some(match name.trim().to_ascii_lowercase().as_str() {
                "return" | "enter" => 36,
                "tab" => 48,
                "space" => 49,
                "delete" | "backspace" => 51,
                "escape" | "esc" => 53,
                "left" => 123,
                "right" => 124,
                "down" => 125,
                "up" => 126,
                "home" => 115,
                "end" => 119,
                "page_up" | "pageup" => 116,
                "page_down" | "pagedown" => 121,
                "cmd" | "command" | "super" | "meta" => 55,
                "shift" => 56,
                "alt" | "option" => 58,
                "ctrl" | "control" => 59,
                "a" => 0, "s" => 1, "d" => 2, "f" => 3, "h" => 4, "g" => 5,
                "z" => 6, "x" => 7, "c" => 8, "v" => 9, "b" => 11, "q" => 12,
                "w" => 13, "e" => 14, "r" => 15, "y" => 16, "t" => 17,
                "o" => 31, "u" => 32, "i" => 34, "p" => 35, "l" => 37,
                "j" => 38, "k" => 40, "n" => 45, "m" => 46,
                _ => return None,
            })
        }

        fn tap(&self, code: u16, down: bool) -> Result<(), String> {
            let src = Self::source()?;
            let ev = CGEvent::new_keyboard_event(src, code, down)
                .map_err(|_| "Couldn't build the key event".to_string())?;
            ev.post(CGEventTapLocation::HID);
            Ok(())
        }
    }

    impl Controller for MacController {
        fn screen(&self) -> Result<Screen, String> {
            Ok(Screen { width: self.width, height: self.height })
        }

        fn cursor(&self) -> Result<(i32, i32), String> {
            let src = Self::source()?;
            let ev = CGEvent::new(src).map_err(|_| "Couldn't read the cursor".to_string())?;
            let p = ev.location();
            Ok((p.x as i32, p.y as i32))
        }

        fn screenshot(&self, path: &str) -> Result<(), String> {
            // `screencapture` ships with macOS; -x suppresses the shutter sound.
            let out = std::process::Command::new("screencapture")
                .args(["-x", "-t", "png", path])
                .output()
                .map_err(|e| format!("screencapture failed: {e}"))?;
            if out.status.success() {
                Ok(())
            } else {
                Err(format!(
                    "screencapture failed: {}. Grant Screen Recording permission in System Settings ▸ Privacy & Security.",
                    String::from_utf8_lossy(&out.stderr).trim()
                ))
            }
        }

        fn perform(&mut self, action: &Action) -> Result<(), String> {
            match action {
                Action::Screenshot | Action::CursorPosition | Action::Zoom { .. } => Ok(()),
                Action::Wait { seconds } => {
                    std::thread::sleep(std::time::Duration::from_secs_f64(seconds.clamp(0.0, 30.0)));
                    Ok(())
                }
                Action::MouseMove { x, y } => {
                    self.mouse(CGEventType::MouseMoved, *x, *y, CGMouseButton::Left)
                }
                Action::LeftClick { x, y } => self.click(*x, *y, CGMouseButton::Left, 1),
                Action::RightClick { x, y } => self.click(*x, *y, CGMouseButton::Right, 1),
                Action::MiddleClick { x, y } => self.click(*x, *y, CGMouseButton::Center, 1),
                Action::DoubleClick { x, y } => self.click(*x, *y, CGMouseButton::Left, 2),
                Action::TripleClick { x, y } => self.click(*x, *y, CGMouseButton::Left, 3),
                Action::LeftMouseDown => {
                    let (x, y) = self.cursor()?;
                    self.mouse(CGEventType::LeftMouseDown, x, y, CGMouseButton::Left)
                }
                Action::LeftMouseUp => {
                    let (x, y) = self.cursor()?;
                    self.mouse(CGEventType::LeftMouseUp, x, y, CGMouseButton::Left)
                }
                Action::LeftClickDrag { from, to } => {
                    self.mouse(CGEventType::MouseMoved, from.0, from.1, CGMouseButton::Left)?;
                    self.mouse(CGEventType::LeftMouseDown, from.0, from.1, CGMouseButton::Left)?;
                    for i in 1..=10 {
                        let x = from.0 + (to.0 - from.0) * i / 10;
                        let y = from.1 + (to.1 - from.1) * i / 10;
                        self.mouse(CGEventType::LeftMouseDragged, x, y, CGMouseButton::Left)?;
                    }
                    self.mouse(CGEventType::LeftMouseUp, to.0, to.1, CGMouseButton::Left)
                }
                Action::Scroll { x, y, direction, amount } => {
                    self.mouse(CGEventType::MouseMoved, *x, *y, CGMouseButton::Left)?;
                    let n = (*amount).max(1);
                    let (dy, dx) = match direction {
                        ScrollDirection::Up => (n, 0),
                        ScrollDirection::Down => (-n, 0),
                        ScrollDirection::Left => (0, n),
                        ScrollDirection::Right => (0, -n),
                    };
                    let src = Self::source()?;
                    let ev = CGEvent::new_scroll_event(src, ScrollEventUnit::LINE, 2, dy, dx, 0)
                        .map_err(|_| "Couldn't build the scroll event".to_string())?;
                    ev.post(CGEventTapLocation::HID);
                    Ok(())
                }
                Action::Key { combo } => {
                    let parts: Vec<&str> = combo.split(['+', '-']).filter(|p| !p.is_empty()).collect();
                    let (last, mods) = parts.split_last().ok_or("empty key combination")?;
                    let mut held = Vec::new();
                    for m in mods {
                        let c = Self::keycode(m).ok_or_else(|| format!("unknown modifier `{m}`"))?;
                        self.tap(c, true)?;
                        held.push(c);
                    }
                    let c = Self::keycode(last).ok_or_else(|| format!("unknown key `{last}`"))?;
                    let res = self.tap(c, true).and_then(|_| self.tap(c, false));
                    // Release modifiers no matter what — a stuck Cmd is nasty.
                    for c in held.into_iter().rev() {
                        self.tap(c, false)?;
                    }
                    res
                }
                Action::HoldKey { combo, seconds } => {
                    let c = Self::keycode(combo).ok_or_else(|| format!("unknown key `{combo}`"))?;
                    self.tap(c, true)?;
                    std::thread::sleep(std::time::Duration::from_secs_f64(seconds.clamp(0.0, 10.0)));
                    self.tap(c, false)
                }
                Action::Type { text } => {
                    // Post the text as a unicode string rather than synthesising
                    // per-key events, so it is layout-independent.
                    let src = Self::source()?;
                    let ev = CGEvent::new_keyboard_event(src, 0, true)
                        .map_err(|_| "Couldn't build the key event".to_string())?;
                    ev.set_string(text);
                    ev.post(CGEventTapLocation::HID);
                    Ok(())
                }
            }
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos::MacController;

// ── Windows ────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
pub mod windows {
    use super::*;
    use enigo::{
        Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings,
    };

    pub struct WindowsController {
        enigo: Enigo,
        width: u32,
        height: u32,
    }

    impl WindowsController {
        pub fn open() -> Result<Self, String> {
            let enigo = Enigo::new(&Settings::default())
                .map_err(|e| format!("Input control unavailable: {e}"))?;
            let (w, h) = enigo.main_display().map_err(|e| format!("Can't read the display: {e}"))?;
            Ok(WindowsController { enigo, width: w as u32, height: h as u32 })
        }

        fn key_for(name: &str) -> Option<Key> {
            Some(match name.trim().to_ascii_lowercase().as_str() {
                "return" | "enter" => Key::Return,
                "tab" => Key::Tab,
                "space" => Key::Space,
                "escape" | "esc" => Key::Escape,
                "delete" | "backspace" => Key::Backspace,
                "left" => Key::LeftArrow,
                "right" => Key::RightArrow,
                "up" => Key::UpArrow,
                "down" => Key::DownArrow,
                "home" => Key::Home,
                "end" => Key::End,
                "page_up" | "pageup" => Key::PageUp,
                "page_down" | "pagedown" => Key::PageDown,
                "ctrl" | "control" => Key::Control,
                "alt" => Key::Alt,
                "shift" => Key::Shift,
                "super" | "cmd" | "meta" | "win" => Key::Meta,
                other => {
                    let mut cs = other.chars();
                    let c = cs.next()?;
                    if cs.next().is_some() {
                        return None;
                    }
                    Key::Unicode(c)
                }
            })
        }

        fn click(&mut self, x: i32, y: i32, b: Button, times: u32) -> Result<(), String> {
            self.enigo.move_mouse(x, y, Coordinate::Abs).map_err(|e| e.to_string())?;
            for _ in 0..times {
                self.enigo.button(b, Direction::Click).map_err(|e| e.to_string())?;
            }
            Ok(())
        }
    }

    impl Controller for WindowsController {
        fn screen(&self) -> Result<Screen, String> {
            Ok(Screen { width: self.width, height: self.height })
        }

        fn cursor(&self) -> Result<(i32, i32), String> {
            self.enigo.location().map_err(|e| e.to_string())
        }

        fn screenshot(&self, path: &str) -> Result<(), String> {
            // PowerShell + System.Drawing is always present on Windows, so this
            // needs no extra crate or install.
            let script = format!(
                "Add-Type -AssemblyName System.Windows.Forms,System.Drawing; \
                 $b=[System.Windows.Forms.SystemInformation]::VirtualScreen; \
                 $bmp=New-Object System.Drawing.Bitmap $b.Width,$b.Height; \
                 $g=[System.Drawing.Graphics]::FromImage($bmp); \
                 $g.CopyFromScreen($b.Location,[System.Drawing.Point]::Empty,$b.Size); \
                 $bmp.Save('{path}',[System.Drawing.Imaging.ImageFormat]::Png)"
            );
            let out = std::process::Command::new("powershell")
                .args(["-NoProfile", "-Command", &script])
                .output()
                .map_err(|e| format!("powershell screenshot failed: {e}"))?;
            if out.status.success() {
                Ok(())
            } else {
                Err(format!("screenshot failed: {}", String::from_utf8_lossy(&out.stderr).trim()))
            }
        }

        fn perform(&mut self, action: &Action) -> Result<(), String> {
            match action {
                Action::Screenshot | Action::CursorPosition | Action::Zoom { .. } => Ok(()),
                Action::Wait { seconds } => {
                    std::thread::sleep(std::time::Duration::from_secs_f64(seconds.clamp(0.0, 30.0)));
                    Ok(())
                }
                Action::MouseMove { x, y } => {
                    self.enigo.move_mouse(*x, *y, Coordinate::Abs).map_err(|e| e.to_string())
                }
                Action::LeftClick { x, y } => self.click(*x, *y, Button::Left, 1),
                Action::RightClick { x, y } => self.click(*x, *y, Button::Right, 1),
                Action::MiddleClick { x, y } => self.click(*x, *y, Button::Middle, 1),
                Action::DoubleClick { x, y } => self.click(*x, *y, Button::Left, 2),
                Action::TripleClick { x, y } => self.click(*x, *y, Button::Left, 3),
                Action::LeftMouseDown => {
                    self.enigo.button(Button::Left, Direction::Press).map_err(|e| e.to_string())
                }
                Action::LeftMouseUp => {
                    self.enigo.button(Button::Left, Direction::Release).map_err(|e| e.to_string())
                }
                Action::LeftClickDrag { from, to } => {
                    self.enigo.move_mouse(from.0, from.1, Coordinate::Abs).map_err(|e| e.to_string())?;
                    self.enigo.button(Button::Left, Direction::Press).map_err(|e| e.to_string())?;
                    for i in 1..=10 {
                        let x = from.0 + (to.0 - from.0) * i / 10;
                        let y = from.1 + (to.1 - from.1) * i / 10;
                        self.enigo.move_mouse(x, y, Coordinate::Abs).map_err(|e| e.to_string())?;
                    }
                    self.enigo.button(Button::Left, Direction::Release).map_err(|e| e.to_string())
                }
                Action::Scroll { x, y, direction, amount } => {
                    self.enigo.move_mouse(*x, *y, Coordinate::Abs).map_err(|e| e.to_string())?;
                    let n = (*amount).max(1);
                    let (axis, delta) = match direction {
                        ScrollDirection::Up => (Axis::Vertical, -n),
                        ScrollDirection::Down => (Axis::Vertical, n),
                        ScrollDirection::Left => (Axis::Horizontal, -n),
                        ScrollDirection::Right => (Axis::Horizontal, n),
                    };
                    self.enigo.scroll(delta, axis).map_err(|e| e.to_string())
                }
                Action::Key { combo } => {
                    let parts: Vec<&str> = combo.split(['+', '-']).filter(|p| !p.is_empty()).collect();
                    let (last, mods) = parts.split_last().ok_or("empty key combination")?;
                    let mut held = Vec::new();
                    for m in mods {
                        let k = Self::key_for(m).ok_or_else(|| format!("unknown modifier `{m}`"))?;
                        self.enigo.key(k, Direction::Press).map_err(|e| e.to_string())?;
                        held.push(k);
                    }
                    let k = Self::key_for(last).ok_or_else(|| format!("unknown key `{last}`"))?;
                    let res = self.enigo.key(k, Direction::Click).map_err(|e| e.to_string());
                    for k in held.into_iter().rev() {
                        self.enigo.key(k, Direction::Release).map_err(|e| e.to_string())?;
                    }
                    res
                }
                Action::HoldKey { combo, seconds } => {
                    let k = Self::key_for(combo).ok_or_else(|| format!("unknown key `{combo}`"))?;
                    self.enigo.key(k, Direction::Press).map_err(|e| e.to_string())?;
                    std::thread::sleep(std::time::Duration::from_secs_f64(seconds.clamp(0.0, 10.0)));
                    self.enigo.key(k, Direction::Release).map_err(|e| e.to_string())
                }
                Action::Type { text } => self.enigo.text(text).map_err(|e| e.to_string()),
            }
        }
    }
}

#[cfg(target_os = "windows")]
pub use windows::WindowsController;
