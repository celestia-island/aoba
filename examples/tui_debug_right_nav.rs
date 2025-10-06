/// Debug script to verify register editing flow with Right arrow navigation
use anyhow::Result;
use aoba::ci::TerminalCapture;
use expectrl::{spawn, Expect};
use std::time::Duration;

fn main() -> Result<()> {
    println!("🔍 TUI Register Editing with RIGHT navigation");
    println!("===============================================\n");

    // Create virtual COM ports
    println!("📡 Creating virtual COM ports...");
    let _vcom_process =
        spawn("socat -d -d pty,raw,echo=0,link=/tmp/vcom1 pty,raw,echo=0,link=/tmp/vcom2")?;
    std::thread::sleep(Duration::from_secs(2));
    println!("✓ Created\n");

    // Launch TUI
    println!("🚀 Launching TUI...");
    let aoba_bin = std::env::current_dir()?.join("target/release/aoba");
    let mut tui_session = spawn(format!("{} --tui", aoba_bin.display()))?;
    let mut cap = TerminalCapture::new(30, 80);
    std::thread::sleep(Duration::from_secs(3));

    // Navigate to vcom1, open business config, create station
    tui_session.send("\r")?; // Open vcom1
    std::thread::sleep(Duration::from_millis(500));
    tui_session.send("\x1b[B\x1b[B\r")?; // Down 2, Enter for Business Config
    std::thread::sleep(Duration::from_millis(1000));
    tui_session.send("\r")?; // Create station
    std::thread::sleep(Duration::from_millis(1000));

    // Set Register Length to 4
    for _ in 0..5 {
        tui_session.send("\x1b[B")?; // Down 5 times to Register Length
        std::thread::sleep(Duration::from_millis(100));
    }
    tui_session.send("\r4\r")?; // Edit, type 4, confirm
    std::thread::sleep(Duration::from_millis(1000));

    let screen = capture_screen(&mut cap, &mut tui_session, "reg_length_4")?;
    println!("📸 Register Length set to 4:\n{}\n", screen);

    // Navigate to register values line
    tui_session.send("\x1b[B")?; // Down to register line
    std::thread::sleep(Duration::from_millis(500));

    let screen = capture_screen(&mut cap, &mut tui_session, "on_reg_line")?;
    println!("📸 On register values line:\n{}\n", screen);

    // **KEY TEST**: Edit registers using Enter -> Value -> Enter -> Right pattern
    println!("🧪 TEST: Edit 4 registers using Enter-Value-Enter-Right pattern");
    println!("================================================================\n");

    // Register 0 = 0
    println!("⌨️  Step 1: Enter -> '0' -> Enter");
    tui_session.send("\r0\r")?;
    std::thread::sleep(Duration::from_millis(500));
    let screen = capture_screen(&mut cap, &mut tui_session, "reg_0_set")?;
    println!("📸 After setting reg 0:\n{}\n", screen);

    // RIGHT to move to register 1
    println!("⌨️  Step 2: Right arrow to move to register 1");
    tui_session.send("\x1b[C")?; // Right
    std::thread::sleep(Duration::from_millis(300));
    let screen = capture_screen(&mut cap, &mut tui_session, "moved_to_reg_1")?;
    println!("📸 After pressing Right (should be on reg 1):\n{}\n", screen);

    // Register 1 = 10 (0x0A)
    println!("⌨️  Step 3: Enter -> 'A' -> Enter");
    tui_session.send("\rA\r")?;
    std::thread::sleep(Duration::from_millis(500));
    let screen = capture_screen(&mut cap, &mut tui_session, "reg_1_set")?;
    println!("📸 After setting reg 1 to 0x000A:\n{}\n", screen);

    // RIGHT to move to register 2
    println!("⌨️  Step 4: Right arrow to move to register 2");
    tui_session.send("\x1b[C")?; // Right
    std::thread::sleep(Duration::from_millis(300));
    let screen = capture_screen(&mut cap, &mut tui_session, "moved_to_reg_2")?;
    println!("📸 After pressing Right (should be on reg 2):\n{}\n", screen);

    // Register 2 = 20 (0x14)
    println!("⌨️  Step 5: Enter -> '14' -> Enter");
    tui_session.send("\r14\r")?;
    std::thread::sleep(Duration::from_millis(500));
    let screen = capture_screen(&mut cap, &mut tui_session, "reg_2_set")?;
    println!("📸 After setting reg 2 to 0x0014:\n{}\n", screen);

    // RIGHT to move to register 3
    println!("⌨️  Step 6: Right arrow to move to register 3");
    tui_session.send("\x1b[C")?; // Right
    std::thread::sleep(Duration::from_millis(300));
    let screen = capture_screen(&mut cap, &mut tui_session, "moved_to_reg_3")?;
    println!("📸 After pressing Right (should be on reg 3):\n{}\n", screen);

    // Register 3 = 30 (0x1E)
    println!("⌨️  Step 7: Enter -> '1E' -> Enter");
    tui_session.send("\r1E\r")?;
    std::thread::sleep(Duration::from_millis(500));
    let screen = capture_screen(&mut cap, &mut tui_session, "reg_3_set")?;
    println!("📸 After setting reg 3 to 0x001E:\n{}\n", screen);

    println!("\n📊 ANALYSIS");
    println!("===========");
    println!("Expected final values: 0x0000 0x000A 0x0014 0x001E");
    println!("Check if the pattern Enter->Value->Enter->Right works correctly!");

    Ok(())
}

fn capture_screen(
    cap: &mut TerminalCapture,
    session: &mut impl Expect,
    label: &str,
) -> Result<String> {
    cap.capture(session, label)
}
