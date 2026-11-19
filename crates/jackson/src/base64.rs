//! `Base64Variants.MIME` decoding with Jackson's exception texts.

fn decode_char(ch: char) -> i32 {
    match ch {
        'A'..='Z' => ch as i32 - 'A' as i32,
        'a'..='z' => ch as i32 - 'a' as i32 + 26,
        '0'..='9' => ch as i32 - '0' as i32 + 52,
        '+' => 62,
        '/' => 63,
        '=' => -2,
        _ => -1,
    }
}

fn is_iso_control(c: char) -> bool {
    (c as u32) <= 0x1f || (0x7f..=0x9f).contains(&(c as u32))
}

fn invalid(ch: char, bindex: usize, msg: Option<&str>) -> String {
    let mut base = if ch <= ' ' {
        format!("Illegal white space character (code 0x{:x}) as character #{} of 4-char base64 unit: can only used between units", ch as u32, bindex + 1)
    } else if ch == '=' {
        format!("Unexpected padding character ('=') as character #{} of 4-char base64 unit: padding only legal as 3rd or 4th character", bindex + 1)
    } else if !is_iso_control(ch) {
        format!("Illegal character '{ch}' (code 0x{:x}) in base64 content", ch as u32)
    } else {
        format!("Illegal character (code 0x{:x}) in base64 content", ch as u32)
    };
    if let Some(m) = msg {
        base.push_str(": ");
        base.push_str(m);
    }
    base
}

fn missing_padding() -> String {
    "Unexpected end of base64-encoded String: base64 variant 'MIME' expects padding (one or more '=' characters) at the end. This Base64Variant might have been incorrectly configured".to_string()
}

/// `Base64Variants.MIME.decode(String)`; the error is the `IllegalArgumentException` message.
pub fn decode_mime(s: &str) -> Result<Vec<u8>, String> {
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let mut out = Vec::new();
    let mut ptr = 0;
    while ptr < len {
        let ch = chars[ptr];
        ptr += 1;
        if ch <= ' ' {
            continue;
        }
        let bits = decode_char(ch);
        if bits < 0 {
            return Err(invalid(ch, 0, None));
        }
        let mut data = bits;
        if ptr >= len {
            return Err(missing_padding());
        }
        let ch = chars[ptr];
        ptr += 1;
        let bits = decode_char(ch);
        if bits < 0 {
            return Err(invalid(ch, 1, None));
        }
        data = (data << 6) | bits;
        if ptr >= len {
            return Err(missing_padding());
        }
        let ch = chars[ptr];
        ptr += 1;
        let bits = decode_char(ch);
        if bits < 0 {
            if bits != -2 {
                return Err(invalid(ch, 2, None));
            }
            if ptr >= len {
                return Err(missing_padding());
            }
            let ch = chars[ptr];
            ptr += 1;
            if ch != '=' {
                return Err(invalid(ch, 3, Some("expected padding character '='")));
            }
            out.push((data >> 4) as u8);
            continue;
        }
        data = (data << 6) | bits;
        if ptr >= len {
            return Err(missing_padding());
        }
        let ch = chars[ptr];
        ptr += 1;
        let bits = decode_char(ch);
        if bits < 0 {
            if bits != -2 {
                return Err(invalid(ch, 3, None));
            }
            let v = data >> 2;
            out.push((v >> 8) as u8);
            out.push(v as u8);
            continue;
        }
        data = (data << 6) | bits;
        out.push((data >> 16) as u8);
        out.push((data >> 8) as u8);
        out.push(data as u8);
    }
    Ok(out)
}
