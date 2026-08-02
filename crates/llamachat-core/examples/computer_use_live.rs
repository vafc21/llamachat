//! End-to-end proof against a real X server: model coordinate -> real pixel ->
//! pointer actually moves -> position read back from X.
use llamachat_core::computer_use::{Action, ScrollDirection, VirtualDisplay};
use llamachat_core::control::{Controller, X11Controller};
use llamachat_core::vision;

fn main() -> Result<(), String> {
    let display = std::env::var("LC_DISPLAY").unwrap_or_else(|_| ":99".into());
    let mut c = X11Controller::open(Some(&display))?;

    let screen = c.screen()?;
    let vd = c.virtual_display()?;
    println!("screen      {}x{}", screen.width, screen.height);
    println!("model sees  {}x{}", vd.width, vd.height);
    assert_eq!((vd.width, vd.height), (1024, 576), "virtual display wrong");

    let mut fails = 0;
    // Walk a grid of model-space points and check the pointer lands where the
    // mapping promised.
    for (vx, vy) in [(0, 0), (512, 288), (1023, 575), (100, 500), (900, 50)] {
        let act = Action::LeftClick { x: vx, y: vy }.to_real(&vd);
        let (want_x, want_y) = match act {
            Action::LeftClick { x, y } => (x, y),
            _ => unreachable!(),
        };
        c.perform(&Action::MouseMove { x: want_x, y: want_y })?;
        let (got_x, got_y) = c.cursor()?;
        let ok = got_x == want_x && got_y == want_y;
        if !ok { fails += 1; }
        println!(
            "model({:4},{:4}) -> real({:4},{:4})  pointer({:4},{:4})  {}",
            vx, vy, want_x, want_y, got_x, got_y, if ok { "ok" } else { "MISMATCH" }
        );
    }
    if fails > 0 { return Err(format!("{fails} coordinate mismatches")); }

    // Real input events.
    c.perform(&Action::LeftClick { x: 400, y: 300 })?;
    println!("left_click        ok");
    c.perform(&Action::DoubleClick { x: 410, y: 310 })?;
    println!("double_click      ok");
    c.perform(&Action::RightClick { x: 420, y: 320 })?;
    println!("right_click       ok");
    c.perform(&Action::LeftClickDrag { from: (100, 100), to: (700, 500) })?;
    let after = c.cursor()?;
    println!("left_click_drag   ok  (pointer now {:?})", after);
    assert_eq!(after, (700, 500), "drag did not end at the target");
    c.perform(&Action::Scroll { x: 500, y: 400, direction: ScrollDirection::Down, amount: 3 })?;
    println!("scroll            ok");
    c.perform(&Action::Type { text: "hello world 123".to_string() })?;
    println!("type              ok");
    c.perform(&Action::Key { combo: "ctrl+s".into() })?;
    println!("key ctrl+s        ok");

    // Screenshot -> exactly the model's coordinate space.
    let shot = "/tmp/lc-live-shot.png";
    let out = std::process::Command::new("import")
        .env("DISPLAY", &display)
        .args(["-window", "root", shot])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("import failed: {}", String::from_utf8_lossy(&out.stderr)));
    }
    let enc = vision::encode_for_display(shot, &vd)?;
    println!("screenshot        {}", enc.summary());
    assert_eq!((enc.width, enc.height), (vd.width, vd.height), "picture != coordinate space");
    println!("\nALL CHECKS PASSED");
    Ok(())
}
