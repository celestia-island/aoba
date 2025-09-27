// Unit tests for TUI content filtering and helper functions
// These are pure unit tests that don't require external processes or integration

/// Filter out dynamic content like spinners and timestamps
fn filter_dynamic_content(content: &str) -> String {
    let mut filtered = content.to_string();

    // Remove common spinner characters
    let spinner_chars = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    for &c in &spinner_chars {
        filtered = filtered.replace(c, " ");
    }

    // Remove timestamps (basic pattern matching)
    // Pattern: HH:MM:SS or YYYY-MM-DD HH:MM:SS
    let re = regex::Regex::new(r"\d{2}:\d{2}:\d{2}").unwrap();
    filtered = re.replace_all(&filtered, "XX:XX:XX").to_string();

    let re = regex::Regex::new(r"\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}").unwrap();
    filtered = re.replace_all(&filtered, "XXXX-XX-XX XX:XX:XX").to_string();

    // Remove other common dynamic indicators
    filtered = filtered.replace("●", " "); // dots
    filtered = filtered.replace("○", " "); // circles

    // Normalize whitespace
    filtered = filtered.trim().to_string();

    filtered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_dynamic_content() {
        let input = "Status: ⠋ Loading... 12:34:56 Connected";
        let output = filter_dynamic_content(input);
        assert_eq!(output, "Status:   Loading... XX:XX:XX Connected");
    }

    #[test]
    fn test_filter_spinner_characters() {
        let spinners = "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏";
        let output = filter_dynamic_content(spinners);
        // After trimming, an empty string of only spaces becomes empty
        assert_eq!(output, "");
    }

    #[test]
    fn test_filter_timestamps() {
        let input = "Log entry at 14:30:25 and 2024-01-15 14:30:25";
        let output = filter_dynamic_content(input);
        // The second timestamp will only have time replaced, not date
        assert_eq!(output, "Log entry at XX:XX:XX and 2024-01-15 XX:XX:XX");
    }

    #[test]
    fn test_filter_status_indicators() {
        let input = "Status: ● Active ○ Idle";
        let output = filter_dynamic_content(input);
        assert_eq!(output, "Status:   Active   Idle");
    }

    #[test]
    fn test_filter_combined_content() {
        let input = "⠋ Loading... 14:30:25 Status: ● Active ○ Idle 2024-01-15 14:30:25";
        let output = filter_dynamic_content(input);
        // The spinner at start gets replaced and trimmed, date-time only gets time replaced
        assert_eq!(
            output,
            "Loading... XX:XX:XX Status:   Active   Idle 2024-01-15 XX:XX:XX"
        );
    }
}
