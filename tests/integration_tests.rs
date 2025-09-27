use std::process::Command;

/// Basic smoke tests for CLI functionality
#[test]
fn test_cli_help() {
    let output = Command::new("./target/release/aoba")
        .arg("--help")
        .output()
        .expect("Failed to execute aoba binary");
    
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage: aoba"));
}

#[test]
fn test_cli_list_ports() {
    let output = Command::new("./target/release/aoba")
        .arg("--list-ports")
        .output()
        .expect("Failed to execute aoba binary");
    
    assert!(output.status.success());
}

#[test]
fn test_cli_list_ports_json() {
    let output = Command::new("./target/release/aoba")
        .arg("--list-ports")
        .arg("--json")
        .output()
        .expect("Failed to execute aoba binary");
    
    assert!(output.status.success());
}