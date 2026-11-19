//! `java.net.IDN.toASCII` (RFC 3490 ToASCII per label, RFC 3492 Punycode).
//!
//! Nameprep (RFC 3491) is approximated by Unicode lowercasing: the NFKC step and the prohibited-character
//! tables are not reproduced, which only matters for labels that contain compatibility characters.

use crate::error::{JResult, JavaException};

const ACE_PREFIX: &str = "xn--";
const MAX_LABEL_LENGTH: usize = 63;

fn is_dot(c: char) -> bool {
    matches!(c, '.' | '\u{3002}' | '\u{FF0E}' | '\u{FF61}')
}

fn iae(msg: &str) -> JavaException {
    JavaException::new("java.lang.IllegalArgumentException", msg)
}

/// `IDN.toASCII(input, flag)` (flag `ALLOW_UNASSIGNED`; `USE_STD3_ASCII_RULES` is never used by callers here)
pub fn to_ascii(input: &str) -> JResult<String> {
    let mut chars = input.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        if is_dot(c) {
            return Ok(".".into());
        }
    }
    let mut out = String::new();
    let labels: Vec<&str> = split_labels(input);
    let n = labels.len();
    for (i, label) in labels.iter().enumerate() {
        if i == n - 1 && label.is_empty() && i > 0 {
            // trailing dot: the loop in Java stops without converting an empty last label
            break;
        }
        out.push_str(&to_ascii_label(label)?);
        if i + 1 < n {
            out.push('.');
        }
    }
    Ok(out)
}

fn split_labels(input: &str) -> Vec<&str> {
    if input.is_empty() {
        return vec![];
    }
    let mut v = Vec::new();
    let mut start = 0;
    for (i, c) in input.char_indices() {
        if is_dot(c) {
            v.push(&input[start..i]);
            start = i + c.len_utf8();
        }
    }
    v.push(&input[start..]);
    v
}

fn to_ascii_label(label: &str) -> JResult<String> {
    let is_ascii = label.is_ascii();
    let mut dest = if is_ascii { label.to_string() } else { label.to_lowercase() };
    if dest.is_empty() {
        return Err(iae("Empty label is not a legal name"));
    }
    if !is_ascii && !dest.is_ascii() {
        if dest.to_ascii_lowercase().starts_with(ACE_PREFIX) {
            return Err(iae("The input starts with the ACE Prefix"));
        }
        dest = format!("{ACE_PREFIX}{}", punycode_encode(&dest).to_ascii_lowercase());
    }
    if dest.encode_utf16().count() > MAX_LABEL_LENGTH {
        return Err(iae("The label in the input is too long"));
    }
    Ok(dest)
}

/// RFC 3492 encoding
pub fn punycode_encode(input: &str) -> String {
    const BASE: u32 = 36;
    const TMIN: u32 = 1;
    const TMAX: u32 = 26;
    const SKEW: u32 = 38;
    const DAMP: u32 = 700;
    const INITIAL_BIAS: u32 = 72;
    const INITIAL_N: u32 = 128;
    fn digit(d: u32) -> char {
        if d < 26 { (b'a' + d as u8) as char } else { (b'0' + (d - 26) as u8) as char }
    }
    fn adapt(mut delta: u32, numpoints: u32, first: bool) -> u32 {
        delta = if first { delta / DAMP } else { delta / 2 };
        delta += delta / numpoints;
        let mut k = 0;
        while delta > ((BASE - TMIN) * TMAX) / 2 {
            delta /= BASE - TMIN;
            k += BASE;
        }
        k + (BASE - TMIN + 1) * delta / (delta + SKEW)
    }
    let cps: Vec<u32> = input.chars().map(|c| c as u32).collect();
    let mut out: String = input.chars().filter(|c| c.is_ascii()).collect();
    let b = out.chars().count() as u32;
    let mut h = b;
    if b > 0 {
        out.push('-');
    }
    let mut n = INITIAL_N;
    let mut delta: u32 = 0;
    let mut bias = INITIAL_BIAS;
    while (h as usize) < cps.len() {
        let m = cps.iter().copied().filter(|&c| c >= n).min().unwrap();
        delta += (m - n) * (h + 1);
        n = m;
        for &c in &cps {
            if c < n {
                delta += 1;
            }
            if c == n {
                let mut q = delta;
                let mut k = BASE;
                loop {
                    let t = if k <= bias { TMIN } else if k >= bias + TMAX { TMAX } else { k - bias };
                    if q < t {
                        break;
                    }
                    out.push(digit(t + (q - t) % (BASE - t)));
                    q = (q - t) / (BASE - t);
                    k += BASE;
                }
                out.push(digit(q));
                bias = adapt(delta, h + 1, h == b);
                delta = 0;
                h += 1;
            }
        }
        delta += 1;
        n += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_ascii_cases() {
        assert_eq!(to_ascii("blah.😋.local").unwrap(), "blah.xn--p28h.local");
        assert_eq!(to_ascii("*.😋.local").unwrap(), "*.xn--p28h.local");
        assert_eq!(to_ascii("bücher.DE").unwrap(), "xn--bcher-kva.DE");
        assert_eq!(to_ascii("ÄÖ.de").unwrap(), "xn--4ca0b.de");
        assert_eq!(to_ascii("Example.COM").unwrap(), "Example.COM");
        assert_eq!(to_ascii("a.").unwrap(), "a.");
        assert_eq!(to_ascii("").unwrap(), "");
        assert!(to_ascii("a..b").is_err());
    }
}
