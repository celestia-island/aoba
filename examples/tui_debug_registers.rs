/// Debug script specifically for register editing navigation
use anyhow::Result;
use aoba::ci::TerminalCapture;
use expectrl::{spawn, Expect};
use std::time::Duration;

fn main() -> Result<()> {
    println!("🔍 TUI Register Editing Debug");
    println!("==============================\n");

    // Create virtual COM ports
    println!("📡 Creating virtual COM ports...");
    let _vcom_process =
        spawn("socat -d -d pty,raw,echo=0,link=/tmp/vcom1 pty,raw,echo=0,link=/tmp/vcom2")?;
    std::thread::sleep(Duration::from_secs(2));
    println!("✓ Virtual COM ports created\n");

    // Launch TUI
    println!("🚀 Launching TUI...");
    let aoba_bin = std::env::current_dir()?.join("target/release/aoba");
    let mut tui_session = spawn(format!("{} --tui", aoba_bin.display()))?;
    let mut cap = TerminalCapture::new(30, 80);
    std::thread::sleep(Duration::from_secs(3));
    println!("✓ TUI launched\n");

    // Navigate to vcom1 and open it
    println!("⌨️  Press Enter to open /tmp/vcom1");
    tui_session.send("\r")?;
    std::thread::sleep(Duration::from_millis(1000));

    // Navigate to Business Configuration
    println!("⌨️  Press Down 2 times to 'Enter Business Configuration'");
    tui_session.send("\x1b[B\x1b[B")?;
    std::thread::sleep(Duration::from_millis(500));

    println!("⌨️  Press Enter to open Business Configuration");
    tui_session.send("\r")?;
    std::thread::sleep(Duration::from_millis(1500));

    // Create station
    println!("⌨️  Press Enter to create station");
    tui_session.send("\r")?;
    std::thread::sleep(Duration::from_millis(1000));

    let screen = capture_screen(&mut cap, &mut tui_session, "station_created")?;
    println!("📸 Station created:\n{screen}\n");

    // Navigate to Register Length
    println!("⌨️  Press Down 5 times to reach Register Length");
    for _ in 0..5 {
        tui_session.send("\x1b[B")?;
        std::thread::sleep(Duration::from_millis(150));
    }
    std::thread::sleep(Duration::from_millis(300));

    let screen = capture_screen(&mut cap, &mut tui_session, "on_reg_length")?;
    println!("📸 On Register Length:\n{screen}\n");

    // Set Register Length to 12
    println!("⌨️  Press Enter, type '12', press Enter");
    tui_session.send("\r")?;
    std::thread::sleep(Duration::from_millis(300));
    tui_session.send("12\r")?;
    std::thread::sleep(Duration::from_millis(1000));

    let screen = capture_screen(&mut cap, &mut tui_session, "reg_length_set")?;
    println!("📸 Register Length set to 12:\n{screen}\n");

    // Now navigate DOWN to register values line
    println!("⌨️  Press Down 1 time to reach register values line");
    tui_session.send("\x1b[B")?;
    std::thread::sleep(Duration::from_millis(500));

    let screen = capture_screen(&mut cap, &mut tui_session, "on_reg_values")?;
    println!("📸 On register values line:\n{screen}\n");

    // Enter register editing mode
    println!("⌨️  Press Enter to start editing registers");
    tui_session.send("\r")?;
    std::thread::sleep(Duration::from_millis(500));

    let screen = capture_screen(&mut cap, &mut tui_session, "editing_reg_0")?;
    println!("📸 Editing register 0:\n{screen}\n");

    // Set first register to 0
    println!("⌨️  Type '0' and press Enter");
    tui_session.send("0\r")?;
    std::thread::sleep(Duration::from_millis(500));

    let screen = capture_screen(&mut cap, &mut tui_session, "reg_0_set")?;
    println!("📸 After setting register 0:\n{screen}\n");

    // Try to move to next register - what happens?
    println!("⌨️  Press Tab (if supported) or Enter to move to next register");
    tui_session.send("\t")?; // Try Tab first
    std::thread::sleep(Duration::from_millis(500));

    let screen = capture_screen(&mut cap, &mut tui_session, "after_tab")?;
    println!("📸 After Tab:\n{screen}\n");

    // If Tab didn't work, check if we're on the next register
    // Let's try setting it
    println!("⌨️  Type '10' and press Enter");
    tui_session.send("10\r")?;
    std::thread::sleep(Duration::from_millis(500));

    let screen = capture_screen(&mut cap, &mut tui_session, "reg_1_set")?;
    println!("📸 After setting register 1:\n{screen}\n");

    // Continue for a couple more registers
    println!("⌨️  Type '20' and press Enter");
    tui_session.send("20\r")?;
    std::thread::sleep(Duration::from_millis(500));

    let screen = capture_screen(&mut cap, &mut tui_session, "reg_2_set")?;
    println!("📸 After setting register 2:\n{screen}\n");

    println!("⌨️  Type '30' and press Enter");
    tui_session.send("30\r")?;
    std::thread::sleep(Duration::from_millis(500));

    let screen = capture_screen(&mut cap, &mut tui_session, "reg_3_set")?;
    println!("📸 After setting register 3 (end of first row):\n{screen}\n");

    // What happens at the end of a row?
    println!("⌨️  Type '40' and press Enter");
    tui_session.send("40\r")?;
    std::thread::sleep(Duration::from_millis(500));

    let screen = capture_screen(&mut cap, &mut tui_session, "reg_4_set")?;
    println!("📸 After setting register 4 (first of second row):\n{screen}\n");

    println!("📊 ANALYSIS");
    println!("===========");
    println!("Key findings:");
    println!("1. How to navigate to register values line from Register Length");
    println!("2. How entering values works (Tab vs Enter)");
    println!("3. How cursor moves between registers (within row and between rows)");
    println!("4. Whether the UI automatically wraps to next row after 4 values");

    Ok(())
}

fn capture_screen(
    cap: &mut TerminalCapture,
    session: &mut impl Expect,
    label: &str,
) -> Result<String> {
    cap.capture(session, label)
}
