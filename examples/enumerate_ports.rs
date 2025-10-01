//! Utility to enumerate all serial ports detected by the system.
//! This helps debug port detection issues in CI environments.

use aoba::protocol::tty;

fn main() {
    println!("=== Enumerating all detected serial ports ===\n");
    
    // Get raw ports from serialport crate (uses libudev on Linux)
    let raw_ports = serialport::available_ports().unwrap_or_default();
    println!("Raw ports from serialport::available_ports():");
    if raw_ports.is_empty() {
        println!("  (none)");
    } else {
        for port in &raw_ports {
            println!("  - {} ({:?})", port.port_name, port.port_type);
        }
    }
    println!();
    
    // Get enriched ports from aoba (includes virtual port detection)
    let enriched_ports = tty::available_ports_enriched();
    println!("Enriched ports from aoba::protocol::tty::available_ports_enriched():");
    if enriched_ports.is_empty() {
        println!("  (none)");
    } else {
        for (port, extra) in &enriched_ports {
            println!("  - {} ({:?})", port.port_name, port.port_type);
            if extra.vid.is_some() || extra.pid.is_some() {
                println!("    VID: {:04x?}, PID: {:04x?}", extra.vid, extra.pid);
            }
            if let Some(ref serial) = extra.serial {
                println!("    Serial: {}", serial);
            }
            if let Some(ref manu) = extra.manufacturer {
                println!("    Manufacturer: {}", manu);
            }
            if let Some(ref prod) = extra.product {
                println!("    Product: {}", prod);
            }
        }
    }
    println!();
    
    // Check specifically for vcom ports
    let has_vcom1 = enriched_ports.iter().any(|(p, _)| p.port_name.contains("vcom1"));
    let has_vcom2 = enriched_ports.iter().any(|(p, _)| p.port_name.contains("vcom2"));
    
    println!("=== Summary ===");
    println!("Total raw ports: {}", raw_ports.len());
    println!("Total enriched ports: {}", enriched_ports.len());
    println!("vcom1 detected: {}", has_vcom1);
    println!("vcom2 detected: {}", has_vcom2);
    
    // Check for ttyS* and tty* ports
    let ttys_count = enriched_ports.iter().filter(|(p, _)| {
        p.port_name.contains("/ttyS") && !p.port_name.contains("vcom")
    }).count();
    let tty_other_count = enriched_ports.iter().filter(|(p, _)| {
        p.port_name.contains("/tty") && !p.port_name.contains("USB") 
            && !p.port_name.contains("ACM") && !p.port_name.contains("vcom")
            && !p.port_name.contains("ttyS")
    }).count();
    
    println!("/dev/ttyS* ports: {}", ttys_count);
    println!("Other /dev/tty* ports: {}", tty_other_count);
    
    if ttys_count > 0 || tty_other_count > 0 {
        println!("\n⚠️  Warning: System serial ports detected that may interfere with testing");
    }
    
    if has_vcom1 && has_vcom2 {
        println!("\n✅ Both vcom1 and vcom2 are detected!");
        
        // Find their positions in the list
        let vcom1_pos = enriched_ports.iter().position(|(p, _)| p.port_name.contains("vcom1"));
        let vcom2_pos = enriched_ports.iter().position(|(p, _)| p.port_name.contains("vcom2"));
        
        if let (Some(pos1), Some(pos2)) = (vcom1_pos, vcom2_pos) {
            println!("vcom1 position: {} (1-indexed: {})", pos1, pos1 + 1);
            println!("vcom2 position: {} (1-indexed: {})", pos2, pos2 + 1);
            
            if pos1 == 0 && pos2 == 1 {
                println!("✅ Perfect! vcom1 and vcom2 are the first two ports.");
            } else {
                println!("⚠️  vcom ports are NOT the first two in the list.");
            }
        }
    } else {
        println!("\n❌ vcom ports are NOT detected!");
    }
}
