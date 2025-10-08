use anyhow::Result;
use std::fs::File;
use std::io::{BufRead, BufReader};

/// Read the last N lines from a log file efficiently
/// This is useful for analyzing long debug logs where we only care about recent entries
pub fn tail_log_file(path: &str, num_lines: usize) -> Result<Vec<String>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let mut lines: Vec<String> = reader.lines().map_while(Result::ok).collect();

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

    analysis.push_str("\n=== Summary ===\n");
    analysis.push_str(&format!("Errors: {error_count}\n"));
    analysis.push_str(&format!("Warnings: {warn_count}\n"));
    analysis.push_str(&format!("Timeouts: {timeout_count}\n"));
    analysis.push_str(&format!("TX (transmitted): {tx_count}\n"));
    analysis.push_str(&format!("RX (received): {rx_count}\n"));

    // Append the actual lines
    analysis.push_str("\n=== Log entries ===\n");
    for line in lines {
        analysis.push_str(&line);
        analysis.push('\n');
    }

    Ok(analysis)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_tail_log_file() {
        // Create a temporary log file
        let temp_path = "/tmp/test_log.txt";
        let mut file = File::create(temp_path).unwrap();

        // Write 100 lines
        for i in 1..=100 {
            writeln!(file, "Line {i}").unwrap();
        }
        drop(file);

        // Read last 10 lines
        let lines = tail_log_file(temp_path, 10).unwrap();
        assert_eq!(lines.len(), 10);
        assert_eq!(lines[0], "Line 91");
        assert_eq!(lines[9], "Line 100");

        // Clean up
        std::fs::remove_file(temp_path).ok();
    }

    #[test]
    fn test_analyze_log_tail() {
        // Create a temporary log file with various log levels
        let temp_path = "/tmp/test_log_analysis.txt";
        let mut file = File::create(temp_path).unwrap();

        writeln!(file, "2025-01-01 [INFO] - Starting application").unwrap();
        writeln!(
            file,
            "2025-01-01 [DEBUG] - Master TX (request): 01 03 00 00"
        )
        .unwrap();
        writeln!(
            file,
            "2025-01-01 [DEBUG] - Master RX (response): 01 03 02 12 34"
        )
        .unwrap();
        writeln!(file, "2025-01-01 [WARN] - Timeout detected").unwrap();
        writeln!(file, "2025-01-01 [ERROR] - Connection failed").unwrap();
        drop(file);

        // Analyze the log
        let analysis = analyze_log_tail(temp_path, 100).unwrap();

        assert!(analysis.contains("Errors: 1"));
        assert!(analysis.contains("Warnings: 1"));
        assert!(analysis.contains("Timeouts: 1"));
        assert!(analysis.contains("TX (transmitted): 1"));
        assert!(analysis.contains("RX (received): 1"));

        // Clean up
        std::fs::remove_file(temp_path).ok();
    }
}
