/// Debug script to test 12-register navigation
use anyhow::Result;
use aoba::ci::TerminalCapture;
use expectrl::{spawn, Expect};
use std::time::Duration;

fn main() -> Result<()> {
    println!("🔍 TUI 12-Register Navigation Test");
    println!("===================================\n");

    // Create virtual COM ports
    let _vcom_process =
        spawn("socat -d -d pty,raw,echo=0,link=/tmp/vcom1 pty,raw,echo=0,link=/tmp/vcom2")?;
    std::thread::sleep(Duration::from_secs(2));

    // Launch TUI
    let aoba_bin = std::env::current_dir()?.join("target/release/aoba");
    let mut tui_session = spawn(format!("{} --tui", aoba_bin.display()))?;
    let mut cap = TerminalCapture::new(30, 80);
    std::thread::sleep(Duration::from_secs(3));

    // Navigate to vcom1, open business config, create station
    tui_session.send("\r\x1b[B\x1b[B\r")?;
    std::thread::sleep(Duration::from_millis(1500));
    tui_session.send("\r")?; // Create station
    std::thread::sleep(Duration::from_millis(1000));

    // Set Register Length to 12
    for _ in 0..5 {
        tui_session.send("\x1b[B")?;
        std::thread::sleep(Duration::from_millis(100));
    }
    tui_session.send("\r12\r")?;
    std::thread::sleep(Duration::from_millis(1000));

    let screen = capture_screen(&mut cap, &mut tui_session, "reg_length_12")?;
    println!("📸 Register Length set to 12:\n{}\n", screen);

    // Navigate to register values line
    tui_session.send("\x1b[B")?;
    std::thread::sleep(Duration::from_millis(500));

    println!("🧪 TEST: Set 12 registers using ONLY RIGHT arrow navigation");
    println!("=============================================================\n");

    // Set registers 0-11 with values 0, 10, 20, ..., 110
    let values: Vec<u16> = (0..12).map(|i| i * 10).collect();
    for (i, &val) in values.iter().enumerate() {
        let hex_val = format!("{:X}", val);
        println!("⌨️  Step {}: Enter -> '{}' ({}=0x{:04X}) -> Enter -> Right", i + 1, hex_val, val, val);
        
        // Enter edit mode, type value, confirm, move right
        tui_session.send(&format!("\r{}\r", hex_val))?;
        std::thread::sleep(Duration::from_millis(400));
        
        if i < values.len() - 1 {
            tui_session.send("\x1b[C")?; // Right arrow
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    std::thread::sleep(Duration::from_millis(1000));
    let screen = capture_screen(&mut cap, &mut tui_session, "all_12_set")?;
    println!("\n📸 After setting all 12 registers:\n{}\n", screen);

    println!("📊 ANALYSIS");
    println!("===========");
    println!("Expected values:");
    println!("  Row 1: 0x0000 0x000A 0x0014 0x001E");
    println!("  Row 2: 0x0028 0x0032 0x003C 0x0046");
    println!("  Row 3: 0x0050 0x005A 0x0064 0x006E");
    println!("\nDoes simple RIGHT navigation work for all 12 registers?");

    Ok(())
}

fn capture_screen(
    cap: &mut TerminalCapture,
    session: &mut impl Expect,
    label: &str,
) -> Result<String> {
    cap.capture(session, label)
}
