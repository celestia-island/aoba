/// Smoke test to capture and analyze TUI interface structure
/// This helps us understand the actual screen layout before writing navigation logic
use anyhow::Result;
use aoba::ci::TerminalCapture;
use expectrl::{spawn, Expect};
use std::time::Duration;

fn main() -> Result<()> {
    println!("🔍 TUI Smoke Test - Understanding Interface Structure");
    println!("================================================\n");

    // Create virtual COM ports first
    println!("📡 Creating virtual COM ports...");
    let _vcom_process =
        spawn("socat -d -d pty,raw,echo=0,link=/dev/vcom1 pty,raw,echo=0,link=/dev/vcom2")?;
    std::thread::sleep(Duration::from_secs(2));
    println!("✓ Virtual COM ports created: /dev/vcom1, /dev/vcom2\n");

    // Launch TUI
    println!("🚀 Launching TUI application...");
    // Use the already-built binary with absolute path
    let aoba_bin = std::env::current_dir()?.join("target/release/aoba");
    let mut tui_session = spawn(format!("{} --tui", aoba_bin.display()))?;
    let mut cap = TerminalCapture::new(30, 80);
    std::thread::sleep(Duration::from_secs(3));
    println!("✓ TUI launched\n");

    // Capture initial screen
    println!("📸 SCREEN CAPTURE 1: Initial TUI Screen");
    println!("========================================");
    let screen1 = capture_screen(&mut cap, &mut tui_session, "initial")?;
    println!("{screen1}");
    println!("\n");

    // Press Down to see what happens
    println!("⌨️  ACTION: Press Down arrow");
    tui_session.send("\x1b[B")?; // Down arrow
    std::thread::sleep(Duration::from_millis(500));

    println!("📸 SCREEN CAPTURE 2: After Down arrow");
    println!("========================================");
    let screen2 = capture_screen(&mut cap, &mut tui_session, "after_down_1")?;
    println!("{screen2}");
    println!("\n");

    // Press Down again
    println!("⌨️  ACTION: Press Down arrow again");
    tui_session.send("\x1b[B")?;
    std::thread::sleep(Duration::from_millis(500));

    println!("📸 SCREEN CAPTURE 3: After second Down arrow");
    println!("========================================");
    let screen3 = capture_screen(&mut cap, &mut tui_session, "after_down_2")?;
    println!("{screen3}");
    println!("\n");

    // Press Enter to open port details
    println!("⌨️  ACTION: Press Enter");
    tui_session.send("\r")?;
    std::thread::sleep(Duration::from_millis(500));

    println!("📸 SCREEN CAPTURE 4: After Enter (port details)");
    println!("========================================");
    let screen4 = capture_screen(&mut cap, &mut tui_session, "port_details")?;
    println!("{screen4}");
    println!("\n");

    // Navigate down in port details
    println!("⌨️  ACTION: Press Down in port details");
    tui_session.send("\x1b[B")?;
    std::thread::sleep(Duration::from_millis(500));

    println!("📸 SCREEN CAPTURE 5: After Down in port details");
    println!("========================================");
    let screen5 = capture_screen(&mut cap, &mut tui_session, "port_details_down")?;
    println!("{screen5}");
    println!("\n");

    // Press Enter on Modbus Settings
    println!("⌨️  ACTION: Press Enter on Modbus Settings");
    tui_session.send("\r")?;
    std::thread::sleep(Duration::from_millis(500));

    println!("📸 SCREEN CAPTURE 6: Inside Modbus Settings");
    println!("========================================");
    let screen6 = capture_screen(&mut cap, &mut tui_session, "modbus_settings")?;
    println!("{screen6}");
    println!("\n");

    // Analysis
    println!("📊 ANALYSIS");
    println!("===========");
    println!("Please review the screen captures above to understand:");
    println!("1. How ports are listed (which appears first: vcom1 or vcom2?)");
    println!("2. How cursor position is indicated (> character?)");
    println!("3. Navigation structure (how many Down presses to reach different items)");
    println!("4. Panel titles and content patterns");
    println!("5. Field labels and value formats");
    println!("\nThis information will guide the rewrite of hybrid tests.");

    Ok(())
}

fn capture_screen(
    cap: &mut TerminalCapture,
    session: &mut impl Expect,
    label: &str,
) -> Result<String> {
    cap.capture(session, label)
}
