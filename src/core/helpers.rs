/// Parse hex payload string into bytes
pub fn parse_hex_payload(data: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut current = String::new();

    for ch in data.chars() {
        if ch.is_ascii_hexdigit() {
            current.push(ch);
            if current.len() == 2 {
                if let Ok(value) = u8::from_str_radix(&current, 16) {
                    bytes.push(value);
                }
                current.clear();
            }
        } else {
            current.clear();
        }
    }

    bytes
}
