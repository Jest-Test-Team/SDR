pub fn parse_mac(s: &str) -> [u8; 6] {
    let mut mac = [0u8; 6];
    let mut idx = 0usize;
    let mut hi: Option<u8> = None;

    for byte in s.bytes() {
        let v = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            b':' | b'-' => continue,
            _ => continue,
        };
        if let Some(high) = hi {
            if idx < 6 {
                mac[idx] = (high << 4) | v;
                idx += 1;
            }
            hi = None;
        } else {
            hi = Some(v);
        }
    }

    mac
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_colon_separated_mac() {
        assert_eq!(
            parse_mac("14:C1:9F:CB:51:B4"),
            [0x14, 0xC1, 0x9F, 0xCB, 0x51, 0xB4]
        );
    }

    #[test]
    fn parses_lowercase_mac() {
        assert_eq!(
            parse_mac("cc:7b:5c:25:9e:20"),
            [0xCC, 0x7B, 0x5C, 0x25, 0x9E, 0x20]
        );
    }
}
