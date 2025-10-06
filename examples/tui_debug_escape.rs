/// Debug script to test the escape behavior after register editing
use anyhow::Result;
use aoba::ci::TerminalCapture;
use expectrl::{spawn, Expect};
use std::time::Duration;

fn main() -> Result<()> {
    println!("🔍 TUI Escape Behavior After Register Editing");
    println!("==============================================\n");

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

    // Set Register Length to 4
    for _ in 0..5 {
        tui_session.send("\x1b[B")?;
        std::thread::sleep(Duration::from_millis(100));
    }
    tui_session.send("\r4\r")?;
    std::thread::sleep(Duration::from_millis(1000));

    // Navigate to register values and set 4 registers
    tui_session.send("\x1b[B")?;
    std::thread::sleep(Duration::from_millis(500));

    let values = [0, 10, 20, 30];
    for (i, &val) in values.iter().enumerate() {
        let hex = format!("{:X}", val);
        tui_session.send(&format!("\r{}\r", hex))?;
        std::thread::sleep(Duration::from_millis(400));
        
        if i < values.len() - 1 {
            tui_session.send("\x1b[C")?; // Right
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    std::thread::sleep(Duration::from_millis(500));
    let screen = capture_screen(&mut cap, &mut tui_session, "all_set")?;
    println!("📸 After setting all 4 registers:\n{}\n", screen);

    println!("🧪 TEST: Press Escape ONCE and see where we go");
    println!("===============================================\n");
    
    tui_session.send("\x1b")?; // Escape
    std::thread::sleep(Duration::from_millis(1000));
    
    let screen = capture_screen(&mut cap, &mut tui_session, "after_escape_1")?;
    println!("📸 After first Escape:\n{}\n", screen);
    
    // Check where we are
    if screen.contains("COM Ports") {
        println!("❌ We're on the main port list (went too far back)");
    } else if screen.contains("Enable Port") {
        println!("✓ We're on the port details page (perfect!)");
    } else if screen.contains("ModBus Master/Slave Settings") {
        println!("~ We're still in Modbus settings page");
        
        println!("\n🧪 TEST: Press Escape AGAIN");
        tui_session.send("\x1b")?; // Escape again
        std::thread::sleep(Duration::from_millis(1000));
        
        let screen = capture_screen(&mut cap, &mut tui_session, "after_escape_2")?;
        println!("📸 After second Escape:\n{}\n", screen);
        
        if screen.contains("COM Ports") {
            println!("❌ We're on the main port list (went too far back)");
        } else if screen.contains("Enable Port") {
            println!("✓ We're on the port details page (perfect!)");
        }
    }

    println!("\n📊 ANALYSIS");
    println!("===========");
    println!("We need to know:");
    println!("1. After editing registers and NOT being in edit mode,");
    println!("   how many Escapes does it take to get back to port details?");
    println!("2. Do we accidentally go to the main port list?");

    Ok(())
}

fn capture_screen(
    cap: &mut TerminalCapture,
    session: &mut impl Expect,
    label: &str,
) -> Result<String> {
    cap.capture(session, label)
}
