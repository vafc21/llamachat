//! Full loop: the exact JSON a model emits -> ToolRegistry -> real desktop ->
//! screenshot encoded back into the model's coordinate space.
use llamachat_core::control::X11Controller;
use llamachat_core::tools::computer_use_tool::ComputerUseTool;
use llamachat_core::tools::{Tool, ToolRegistry, ToolRequest};
use llamachat_core::vision;

fn main() -> Result<(), String> {
    let display = std::env::var("LC_DISPLAY").unwrap_or_else(|_| ":99".into());
    let ctrl = Box::new(X11Controller::open(Some(&display))?);
    let tool = ComputerUseTool::new(ctrl)?;
    let vd = tool.display();
    println!("model's screen: {}x{}\n", vd.width, vd.height);
    println!("tool advertised to the model as: {}\n", tool.info().name);

    let mut reg = ToolRegistry::new(Default::default(), true);
    reg.register(Box::new(tool));

    // Exactly the JSON shapes Anthropic's computer use emits.
    let script = vec![
        serde_json::json!({"action": "screenshot"}),
        serde_json::json!({"action": "mouse_move", "coordinate": [200, 150]}),
        serde_json::json!({"action": "left_click", "coordinate": [512, 288]}),
        serde_json::json!({"action": "cursor_position"}),
        serde_json::json!({"action": "type", "text": "Hello from the model"}),
        serde_json::json!({"action": "key", "text": "ctrl+a"}),
        serde_json::json!({"action": "scroll", "coordinate": [600, 400],
                           "scroll_direction": "down", "scroll_amount": 2}),
        serde_json::json!({"action": "left_click_drag",
                           "start_coordinate": [100, 100], "coordinate": [800, 500]}),
        serde_json::json!({"action": "fly_to_the_moon"}),
    ];

    let mut saw_image = 0;
    for call in script {
        let r = reg.execute(&ToolRequest { name: "computer".into(), args: call.clone() });
        let act = call["action"].as_str().unwrap_or("?");
        if !r.ok {
            println!("{:<16} refused: {}", act, r.error.clone().unwrap_or_default());
            continue;
        }
        let mut line = format!("{:<16} {}", act, r.output.clone().unwrap_or_default());
        if let Some(p) = &r.media {
            let enc = vision::encode_for_display(p, &vd)?;
            assert_eq!((enc.width, enc.height), (vd.width, vd.height));
            saw_image += 1;
            line.push_str(&format!("   -> image {}", enc.summary()));
        }
        println!("{line}");
    }

    println!("\nscreenshots returned to the model: {saw_image}");
    assert!(saw_image >= 7, "every acting step should return a fresh image");
    println!("ALL CHECKS PASSED");
    Ok(())
}
