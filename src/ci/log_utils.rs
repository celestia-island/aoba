use anyhow::Result;
use std::fs::File;
use std::io::{BufRead, BufReader};

/// Read the last N lines from a log file efficiently
/// This is useful for analyzing long debug logs where we only care about recent entries
pub fn tail_log_file(path: &str, num_lines: usize) -> Result<Vec<String>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    
    let mut lines: Vec<String> = reader.lines()
        .filter_map(|line| line.ok())
        .collect();
    
    // Keep only the last num_lines
    if lines.len() > num_lines {
        lines = lines[lines.len() - num_lines..].to_vec();
    }
    
    Ok(lines)
}

/// Analyze log file for common issues (timeouts, errors, etc.)
pub fn analyze_log_tail(path: &str, num_lines: usize) -> Result<String> {
    let lines = tail_log_file(path, num_lines)?;
    
    let mut analysis = String::new();
    analysis.push_str(&format!("=== Last {} lines of log ===\n", lines.len()));
    
    let mut error_count = 0;
    let mut warn_count = 0;
    let mut timeout_count = 0;
    let mut tx_count = 0;
    let mut rx_count = 0;
    
    for line in &lines {
        if line.contains("[ERROR]") {
            error_count += 1;
        }
        if line.contains("[WARN]") {
            warn_count += 1;
        }
        if line.contains("Timeout") || line.contains("timeout") {
            timeout_count += 1;
        }
        if line.contains("TX") {
            tx_count += 1;
        }
        if line.contains("RX") {
            rx_count += 1;
        }
    }
    
    analysis.push_str(&format!("\n=== Summary ===\n"));
    analysis.push_str(&format!("Errors: {}\n", error_count));
    analysis.push_str(&format!("Warnings: {}\n", warn_count));
    analysis.push_str(&format!("Timeouts: {}\n", timeout_count));
    analysis.push_str(&format!("TX (transmitted): {}\n", tx_count));
    analysis.push_str(&format!("RX (received): {}\n", rx_count));
    
    // Append the actual lines
    analysis.push_str("\n=== Log entries ===\n");
    for line in lines {
        analysis.push_str(&line);
        analysis.push('\n');
    }
    
    Ok(analysis)
}
