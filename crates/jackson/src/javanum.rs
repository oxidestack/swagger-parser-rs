//! Java number parsing helpers.

/// `Float.parseFloat(s)` (`None` where Java throws `NumberFormatException`); decimal input is rounded
/// directly to `float`, as Java does.
pub fn parse_float(s: &str) -> Option<f32> {
    let t = s.trim_matches(|c: char| c <= ' ');
    let (neg, body) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    if body.is_empty() || body == "NaN" || body == "Infinity" || body.starts_with("0x") || body.starts_with("0X") {
        return parse_double(s).map(|d| d as f32);
    }
    parse_double(s)?;
    let body = body.strip_suffix(['f', 'F', 'd', 'D']).unwrap_or(body);
    let mut norm = body.to_string();
    if norm.starts_with('.') {
        norm.insert(0, '0');
    }
    let v = norm.parse::<f32>().ok()?;
    Some(if neg { -v } else { v })
}

/// `Double.parseDouble(s)` (`None` where Java throws `NumberFormatException`).
pub fn parse_double(s: &str) -> Option<f64> {
    let t = s.trim_matches(|c: char| c <= ' ');
    if t.is_empty() {
        return None;
    }
    let (neg, body) = match t.as_bytes()[0] {
        b'-' => (true, &t[1..]),
        b'+' => (false, &t[1..]),
        _ => (false, t),
    };
    let v = if body == "NaN" {
        f64::NAN
    } else if body == "Infinity" {
        f64::INFINITY
    } else if body.starts_with("0x") || body.starts_with("0X") {
        parse_hex_float(&body[2..])?
    } else {
        let body = body.strip_suffix(['f', 'F', 'd', 'D']).unwrap_or(body);
        // digits [. digits] [e[+-]digits], at least one digit in mantissa
        let bytes = body.as_bytes();
        let mut i = 0;
        let mut mant_digits = 0;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
            mant_digits += 1;
        }
        if i < bytes.len() && bytes[i] == b'.' {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
                mant_digits += 1;
            }
        }
        if mant_digits == 0 {
            return None;
        }
        if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
            i += 1;
            if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
                i += 1;
            }
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if i == start {
                return None;
            }
        }
        if i != bytes.len() {
            return None;
        }
        let mut norm = body.to_string();
        if norm.starts_with('.') {
            norm.insert(0, '0');
        }
        norm.parse::<f64>().ok()?
    };
    Some(if neg { -v } else { v })
}

fn parse_hex_float(s: &str) -> Option<f64> {
    let s = s.strip_suffix(['f', 'F', 'd', 'D']).unwrap_or(s);
    let (mant, exp) = s.split_once(['p', 'P'])?;
    let exp: i32 = exp.parse().ok()?;
    let (ip, fp) = mant.split_once('.').unwrap_or((mant, ""));
    if ip.is_empty() && fp.is_empty() {
        return None;
    }
    let mut v = 0f64;
    for c in ip.chars() {
        v = v * 16.0 + c.to_digit(16)? as f64;
    }
    let mut scale = 1.0 / 16.0;
    for c in fp.chars() {
        v += c.to_digit(16)? as f64 * scale;
        scale /= 16.0;
    }
    Some(v * 2f64.powi(exp))
}
