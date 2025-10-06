/// Debug script to trace navigation issues in TUI
/// This helps understand cursor movement and field navigation
use anyhow::Result;
use aoba::ci::TerminalCapture;
use expectrl::{spawn, Expect};
use std::time::Duration;

fn main() -> Result<()> {
    println!("🔍 TUI Navigation Debug - Tracing cursor movements");
    println!("===================================================\n");

    // Create virtual COM ports first
    println!("📡 Creating virtual COM ports...");
    let _vcom_process =
        spawn("socat -d -d pty,raw,echo=0,link=/tmp/vcom1 pty,raw,echo=0,link=/tmp/vcom2")?;
    std::thread::sleep(Duration::from_secs(2));
    println!("✓ Virtual COM ports created: /tmp/vcom1, /tmp/vcom2\n");

    // Launch TUI
    println!("🚀 Launching TUI application...");
    let aoba_bin = std::env::current_dir()?.join("target/release/aoba");
    let mut tui_session = spawn(format!("{} --tui", aoba_bin.display()))?;
    let mut cap = TerminalCapture::new(30, 80);
    std::thread::sleep(Duration::from_secs(3));
    println!("✓ TUI launched\n");

    // Capture initial screen
    println!("📸 SCREEN 1: Initial port list");
    println!("================================");
    let screen1 = capture_screen(&mut cap, &mut tui_session, "initial")?;
    println!("{screen1}");
    println!("\n");

    // Press Enter to open first port (/tmp/vcom1)
    println!("⌨️  ACTION: Press Enter on /tmp/vcom1");
    tui_session.send("\r")?;
    std::thread::sleep(Duration::from_millis(1000));

    println!("📸 SCREEN 2: Port details (should show Enable Port, Protocol Mode, etc.)");
    println!("========================================================================");
    let screen2 = capture_screen(&mut cap, &mut tui_session, "port_details")?;
    println!("{screen2}");
    println!("\n");

    // Now navigate to "Enter Business Configuration"
    println!("⌨️  ACTION: Press Down twice to reach 'Enter Business Configuration'");
    tui_session.send("\x1b[B")?; // Down
    std::thread::sleep(Duration::from_millis(300));
    tui_session.send("\x1b[B")?; // Down again
    std::thread::sleep(Duration::from_millis(500));

    println!("📸 SCREEN 3: After navigating DOWN 2 times");
    println!("==========================================");
    let screen3 = capture_screen(&mut cap, &mut tui_session, "after_down_2")?;
    println!("{screen3}");
    println!("\n");

    // Press Enter on "Enter Business Configuration"
    println!("⌨️  ACTION: Press Enter on 'Enter Business Configuration'");
    tui_session.send("\r")?;
    std::thread::sleep(Duration::from_millis(1500));

    println!("📸 SCREEN 4: Inside Modbus Settings");
    println!("====================================");
    let screen4 = capture_screen(&mut cap, &mut tui_session, "modbus_settings")?;
    println!("{screen4}");
    println!("\n");

    // Create a station (should be on "Create Station" by default)
    println!("⌨️  ACTION: Press Enter to create station");
    tui_session.send("\r")?;
    std::thread::sleep(Duration::from_millis(1000));

    println!("📸 SCREEN 5: After creating station");
    println!("====================================");
    let screen5 = capture_screen(&mut cap, &mut tui_session, "station_created")?;
    println!("{screen5}");
    println!("\n");

    // Navigate to register values line
    println!("⌨️  ACTION: Navigate DOWN 5 times to reach register values");
    for i in 0..5 {
        tui_session.send("\x1b[B")?;
        std::thread::sleep(Duration::from_millis(200));
        println!("  Step {}: Pressed Down", i + 1);
    }
    std::thread::sleep(Duration::from_millis(500));

    println!("📸 SCREEN 6: On register values line");
    println!("=====================================");
    let screen6 = capture_screen(&mut cap, &mut tui_session, "on_registers")?;
    println!("{screen6}");
    println!("\n");

    // Try to edit first register
    println!("⌨️  ACTION: Press Enter to edit first register");
    tui_session.send("\r")?;
    std::thread::sleep(Duration::from_millis(500));

    println!("📸 SCREEN 7: In register edit mode");
    println!("===================================");
    let screen7 = capture_screen(&mut cap, &mut tui_session, "register_edit")?;
    println!("{screen7}");
    println!("\n");

    // Type a value and confirm
    println!("⌨️  ACTION: Type '5' and press Enter");
    tui_session.send("5\r")?;
    std::thread::sleep(Duration::from_millis(500));

    println!("📸 SCREEN 8: After setting first register");
    println!("=========================================");
    let screen8 = capture_screen(&mut cap, &mut tui_session, "reg_value_set")?;
    println!("{screen8}");
    println!("\n");

    // Try navigating RIGHT to next register
    println!("⌨️  ACTION: Press RIGHT to move to next register in same row");
    tui_session.send("\x1b[C")?; // Right arrow
    std::thread::sleep(Duration::from_millis(500));

    println!("📸 SCREEN 9: After pressing RIGHT");
    println!("==================================");
    let screen9 = capture_screen(&mut cap, &mut tui_session, "after_right")?;
    println!("{screen9}");
    println!("\n");

    // Try pressing Enter on second register
    println!("⌨️  ACTION: Press Enter to edit second register");
    tui_session.send("\r")?;
    std::thread::sleep(Duration::from_millis(500));

    println!("📸 SCREEN 10: Editing second register");
    println!("=====================================");
    let screen10 = capture_screen(&mut cap, &mut tui_session, "edit_reg_2")?;
    println!("{screen10}");
    println!("\n");

    // Analysis
    println!("📊 ANALYSIS");
    println!("===========");
    println!("Key observations:");
    println!("1. Port details screen layout:");
    println!("   - Line 0: 'Enable Port'");
    println!("   - Line 1: 'Protocol Mode'");
    println!("   - Line 2: 'Enter Business Configuration' <- CORRECT OPTION");
    println!("   - Line 3: 'Enter Log Page'");
    println!("\n2. Register editing:");
    println!("   - Registers are displayed in rows of 4");
    println!("   - Need to verify RIGHT arrow moves to next register in row");
    println!("   - Need to verify DOWN arrow moves to next row");
    println!("\n3. Navigation patterns:");
    println!("   - Must count DOWN presses from cursor start position");
    println!("   - Verify cursor indicator ('>') appears on correct line");

    Ok(())
}

fn capture_screen(
    cap: &mut TerminalCapture,
    session: &mut impl Expect,
    label: &str,
) -> Result<String> {
    cap.capture(session, label)
}
