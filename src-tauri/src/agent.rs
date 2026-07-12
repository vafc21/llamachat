//! Agent mode — a tool-use loop with Claude-style permission modes.
//!
//! The model is given the tools and drives them itself: it emits a tool call,
//! we run it, feed the result back, and loop until the task is done. Modes:
//!   plan   — describe a plan, execute nothing
//!   ask    — pause for the user's yes/no before each tool call
//!   auto   — run automatically (a Stop button + step cap keep it bounded)
//!   bypass — run automatically with no gating at all
//!
//! Events emitted to the UI: `agent_step` (a tool call), `agent_result` (its
//! output), `agent_answer` (the final reply), `agent_plan`, `agent_approval`
//! (ask-mode request), `agent_status`, `agent_error`, and `agent_done`.

use crate::ollama;
use crate::sidecar;
use crate::state::AppState;
use fitllm_core::tools::ToolRequest;
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use tauri::{Emitter, Manager};

const MAX_STEPS: usize = 12;

fn agent_system_prompt(tools_prompt: &str, memory: &str, plan_mode: bool, perception: &str) -> String {
    let base = if plan_mode {
        "You are LlamaChat's agent, able to control this Mac. The user wants a PLAN only — do NOT act. \
         Reply with a short numbered plan of the steps/tools you would use. Do not output any tool JSON."
            .to_string()
    } else {
        "You are LlamaChat's agent, controlling this Mac to accomplish the user's task. Work step by step.\n\
         - To use a tool, reply with ONLY one JSON object: {\"tool\": \"<name>\", \"args\": { ... }} — no other text, no code fences.\n\
         - `args` MUST be a flat object. `action` is a single word. Never put JSON inside a string.\n\
         - I run it and reply with the result; then you take the next step.\n\
         - When the task is fully done, reply with a short plain-language summary and NO JSON.\n\
         - Prefer the `computer` tool for desktop actions.\n\
         Examples:\n\
         Open an app:        {\"tool\": \"computer\", \"args\": {\"action\": \"open_app\", \"target\": \"Google Chrome\"}}\n\
         Search the web:     {\"tool\": \"computer\", \"args\": {\"action\": \"search_web\", \"target\": \"weather today\"}}\n\
         Run a command:      {\"tool\": \"shell\", \"args\": {\"command\": \"ls -la\"}}"
            .to_string()
    };
    let desktop = if plan_mode {
        String::new()
    } else {
        let see = if perception == "vision" {
            "read_screen returns a vision model's plain-text DESCRIPTION of the screen."
        } else {
            "read_screen returns on-screen elements as text `role: label @ x,y`; use those x,y to click."
        };
        format!(
            "\n\nThe `computer` tool ALSO controls the mouse and reads the screen:\n\
             - read_screen — see what's on screen. {see}\n\
             - click / double_click / right_click — need x and y pixel coordinates (get them from read_screen).\n\
             - mouse_move (x,y), drag (x,y,x2,y2), scroll (direction: up|down).\n\
             To operate an app: open_app, then read_screen, then click the element you want by its x,y."
        )
    };
    let mut s = format!("{base}\n\n{tools_prompt}{desktop}");
    if !memory.trim().is_empty() {
        s.push_str(&format!("\n\nWhat you know about the user:\n{}", memory.trim()));
    }
    s
}

/// Find the first balanced `{...}` object containing a "tool" key.
pub fn parse_tool_call(text: &str) -> Option<(String, Value)> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' {
            let mut depth = 0i32;
            let mut j = i;
            let mut in_str = false;
            let mut esc = false;
            while j < bytes.len() {
                let c = bytes[j];
                if in_str {
                    if esc {
                        esc = false;
                    } else if c == b'\\' {
                        esc = true;
                    } else if c == b'"' {
                        in_str = false;
                    }
                } else {
                    match c {
                        b'"' => in_str = true,
                        b'{' => depth += 1,
                        b'}' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                j += 1;
            }
            if depth == 0 && j < bytes.len() {
                if let Ok(v) = serde_json::from_str::<Value>(&text[i..=j]) {
                    if let Some(tool) = v.get("tool").and_then(|t| t.as_str()) {
                        let args = v.get("args").cloned().unwrap_or_else(|| json!({}));
                        return Some((tool.to_string(), args));
                    }
                }
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }
    None
}

/// Run the agent loop to completion (blocking — call from a spawned thread).
pub fn run(app: tauri::AppHandle, mut messages: Vec<Value>, model: String, mode: String) {
    let emit = |ev: &str, payload: Value| {
        let _ = app.emit(ev, payload);
    };

    if let Err(e) = ollama::ensure_running() {
        emit("agent_error", json!({ "error": e }));
        emit("agent_done", json!({}));
        return;
    }

    let state = app.state::<AppState>();
    let plan_mode = mode == "plan";

    // Build the system prompt and reset run flags. In non-plan modes the user
    // chose to let the agent act, so unlock destructive tools for this run.
    let (sys, perception, vision_model) = {
        let mut inner = match state.0.lock() {
            Ok(i) => i,
            Err(_) => {
                emit("agent_error", json!({ "error": "state busy" }));
                emit("agent_done", json!({}));
                return;
            }
        };
        inner.agent_stop = false;
        inner.agent_decision = None;
        if !plan_mode {
            inner.tools.set_destructive_allowed(true);
        }
        let tp = inner.tools.system_prompt();
        let mem = crate::memory::read_memory(&inner.settings.memory_dir);
        let perception = inner.settings.perception.clone();
        let vision_model = inner.settings.vision_model.clone().unwrap_or_else(|| "llava".into());
        (agent_system_prompt(&tp, &mem, plan_mode, &perception), perception, vision_model)
    };

    let stopped = || state.0.lock().map(|i| i.agent_stop).unwrap_or(false);

    for step in 0..MAX_STEPS {
        if stopped() {
            emit("agent_status", json!({ "text": "Stopped." }));
            break;
        }

        emit("agent_status", json!({ "text": "Thinking…" }));
        let reply = match sidecar::chat(&model, &messages, &sys, |_| {}) {
            Ok(r) => r,
            Err(e) => {
                emit("agent_error", json!({ "error": e.to_string() }));
                break;
            }
        };
        messages.push(json!({ "role": "assistant", "content": reply }));

        if plan_mode {
            emit("agent_plan", json!({ "text": reply.trim() }));
            break;
        }

        let Some((tool, args)) = parse_tool_call(&reply) else {
            // No tool call → this is the final answer.
            emit("agent_answer", json!({ "text": reply.trim() }));
            break;
        };

        // Ask mode: wait for the user's approval.
        if mode == "ask" {
            emit("agent_approval", json!({ "tool": tool, "args": args }));
            let deadline = Instant::now() + Duration::from_secs(180);
            let mut decision: Option<bool> = None;
            while Instant::now() < deadline && decision.is_none() {
                if let Ok(mut inner) = state.0.lock() {
                    if inner.agent_stop {
                        decision = Some(false);
                    } else if let Some(d) = inner.agent_decision.take() {
                        decision = Some(d);
                    }
                }
                if decision.is_none() {
                    std::thread::sleep(Duration::from_millis(150));
                }
            }
            if decision != Some(true) {
                emit("agent_status", json!({ "text": "Skipped." }));
                messages.push(json!({ "role": "user", "content": "[The user declined that tool. Try another approach or finish.]" }));
                continue;
            }
        }

        emit("agent_step", json!({ "n": step + 1, "tool": tool, "args": args }));

        // Desktop control (mouse / read_screen) is handled natively; everything
        // else goes through the tool registry.
        let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("");
        // Route by ACTION, not tool name — small models call it "computer" or
        // "desktop" interchangeably. read_screen/click/mouse/scroll always go native.
        let (ok, text) = if crate::desktop::is_desktop_action(action) {
            let r = if matches!(action, "read_screen" | "read_ui" | "screen") && perception == "vision" {
                crate::desktop::describe_screen(&vision_model)
            } else {
                crate::desktop::control(action, &args)
            };
            match r {
                Ok(t) => (true, t),
                Err(e) => (false, format!("ERROR: {e}")),
            }
        } else {
            match state.0.lock() {
                Ok(inner) => {
                    let result = inner.tools.execute(&ToolRequest { name: tool.clone(), args: args.clone() });
                    if result.ok {
                        (true, result.output.unwrap_or_else(|| "(done)".into()))
                    } else {
                        (false, format!("ERROR: {}", result.error.unwrap_or_else(|| "failed".into())))
                    }
                }
                Err(_) => {
                    emit("agent_error", json!({ "error": "state busy" }));
                    break;
                }
            }
        };
        emit("agent_result", json!({ "tool": tool, "ok": ok, "text": text }));
        // Trim long tool output before feeding it back to the (small) model.
        let fed = if text.len() > 4000 { format!("{}\n…(truncated)", &text[..4000]) } else { text };
        messages.push(json!({ "role": "user", "content": format!("Result of `{tool}`:\n{fed}") }));

        if step == MAX_STEPS - 1 {
            emit("agent_answer", json!({ "text": "Reached the step limit — stopping here." }));
        }
    }

    emit("agent_done", json!({}));
}
