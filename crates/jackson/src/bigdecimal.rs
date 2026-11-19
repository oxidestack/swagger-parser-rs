//! `java.math.BigDecimal`: an unscaled `BigInteger` and a 32-bit scale, value = unscaled × 10^-scale.

use num_bigint::{BigInt, Sign};
use num_traits::{Signed, ToPrimitive, Zero};
use std::cmp::Ordering;

/// `java.math.BigDecimal`: an arbitrary-precision decimal, `unscaled × 10^-scale`.
#[derive(Clone, Debug)]
pub struct BigDecimal {
    /// `unscaledValue()`: the digits as an integer.
    pub unscaled: BigInt,
    /// `scale()`: the number of digits after the decimal point; negative scales multiply by a power of ten.
    pub scale: i32,
}

/// `NumberFormatException` raised by `new BigDecimal(String)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NumberFormatError(pub String);

impl BigDecimal {
    /// `new BigDecimal(unscaled, scale)`.
    pub fn new(unscaled: BigInt, scale: i32) -> BigDecimal {
        BigDecimal { unscaled, scale }
    }

    /// `BigDecimal.valueOf(long)`: scale 0.
    pub fn from_i64(v: i64) -> BigDecimal {
        BigDecimal { unscaled: BigInt::from(v), scale: 0 }
    }

    /// `new BigDecimal(BigInteger)`: scale 0.
    pub fn from_bigint(v: BigInt) -> BigDecimal {
        BigDecimal { unscaled: v, scale: 0 }
    }

    /// `new BigDecimal(String)`.
    pub fn parse(s: &str) -> Result<BigDecimal, NumberFormatError> {
        let chars: Vec<char> = s.chars().collect();
        if chars.is_empty() {
            return Err(NumberFormatError("Index 0 out of bounds for length 0".to_string()));
        }
        let mut i = 0;
        let mut negative = false;
        if chars[0] == '-' || chars[0] == '+' {
            negative = chars[0] == '-';
            i = 1;
        }
        let mut digits = String::new();
        let mut scale: i64 = 0;
        let mut seen_dot = false;
        while i < chars.len() {
            let c = chars[i];
            if c.is_ascii_digit() {
                digits.push(c);
                if seen_dot {
                    scale += 1;
                }
            } else if c == '.' {
                if seen_dot {
                    return Err(NumberFormatError("Character array contains more than one decimal point.".to_string()));
                }
                seen_dot = true;
            } else if c == 'e' || c == 'E' {
                break;
            } else {
                return Err(NumberFormatError(format!(
                    "Character {c} is neither a decimal digit number, decimal point, nor \"e\" notation exponential mark."
                )));
            }
            i += 1;
        }
        if digits.is_empty() {
            return Err(NumberFormatError("No digits found.".to_string()));
        }
        if i < chars.len() {
            let body: String = chars[i + 1..].iter().collect();
            let (eneg, digits_part) = match body.strip_prefix('-') {
                Some(b) => (true, b),
                None => (false, body.strip_prefix('+').unwrap_or(&body)),
            };
            if digits_part.is_empty() || !digits_part.chars().all(|c| c.is_ascii_digit()) {
                return Err(NumberFormatError("No exponent digits.".to_string()));
            }
            let e: i64 = digits_part.parse().map_err(|_| NumberFormatError("Exponent overflow.".to_string()))?;
            scale -= if eneg { -e } else { e };
        }
        if scale > i32::MAX as i64 || scale < i32::MIN as i64 {
            return Err(NumberFormatError("Scale out of range.".to_string()));
        }
        let mut unscaled: BigInt = digits.parse().unwrap();
        if negative {
            unscaled = -unscaled;
        }
        Ok(BigDecimal { unscaled, scale: scale as i32 })
    }

    /// `BigDecimal.valueOf(double)` (= `new BigDecimal(Double.toString(d))`). Panics on NaN/Infinity
    /// (Java throws NumberFormatException) — use [`BigDecimal::try_value_of_f64`] to handle that.
    pub fn value_of_f64(d: f64) -> BigDecimal {
        BigDecimal::parse(&java_compat::double_to_string(d)).expect("finite double")
    }

    /// [`BigDecimal::value_of_f64`] returning `Err` (Java's `NumberFormatException`) for NaN and the infinities.
    pub fn try_value_of_f64(d: f64) -> Result<BigDecimal, NumberFormatError> {
        if !d.is_finite() {
            return Err(NumberFormatError("Infinite or NaN".to_string()));
        }
        BigDecimal::parse(&java_compat::double_to_string(d))
    }

    /// `new BigDecimal(double)`: the exact binary value.
    pub fn from_f64_exact(d: f64) -> BigDecimal {
        if d == 0.0 {
            return BigDecimal { unscaled: BigInt::zero(), scale: 0 };
        }
        let bits = d.to_bits();
        let negative = (bits >> 63) != 0;
        let exponent = ((bits >> 52) & 0x7ff) as i64;
        let mut significand = if exponent == 0 { (bits & ((1 << 52) - 1)) << 1 } else { (bits & ((1 << 52) - 1)) | (1 << 52) };
        let mut exp = exponent - 1075;
        while significand & 1 == 0 {
            significand >>= 1;
            exp += 1;
        }
        let mut val = BigInt::from(significand);
        if negative {
            val = -val;
        }
        if exp >= 0 {
            val <<= exp as usize;
            BigDecimal { unscaled: val, scale: 0 }
        } else {
            let n = (-exp) as u32;
            val *= BigInt::from(5u32).pow(n);
            BigDecimal { unscaled: val, scale: n as i32 }
        }
    }

    /// `signum()`: `-1`, `0` or `1`.
    pub fn signum(&self) -> i32 {
        match self.unscaled.sign() {
            Sign::Minus => -1,
            Sign::NoSign => 0,
            Sign::Plus => 1,
        }
    }

    /// `precision()`
    pub fn precision(&self) -> usize {
        self.unscaled.abs().to_string().len()
    }

    /// `toString()` — scientific notation when the adjusted exponent is below -6 or the scale negative.
    pub fn to_java_string(&self) -> String {
        let coeff = self.unscaled.abs().to_string();
        let mut out = String::new();
        if self.unscaled.is_negative() {
            out.push('-');
        }
        if self.scale == 0 {
            out.push_str(&coeff);
            return out;
        }
        let adjusted: i64 = -(self.scale as i64) + (coeff.len() as i64 - 1);
        if self.scale > 0 && adjusted >= -6 {
            let pad = self.scale as i64 - coeff.len() as i64;
            if pad >= 0 {
                out.push_str("0.");
                for _ in 0..pad {
                    out.push('0');
                }
                out.push_str(&coeff);
            } else {
                let split = (-pad) as usize;
                out.push_str(&coeff[..split]);
                out.push('.');
                out.push_str(&coeff[split..]);
            }
        } else {
            out.push_str(&coeff[..1]);
            if coeff.len() > 1 {
                out.push('.');
                out.push_str(&coeff[1..]);
            }
            if adjusted != 0 {
                out.push('E');
                if adjusted > 0 {
                    out.push('+');
                }
                out.push_str(&adjusted.to_string());
            }
        }
        out
    }

    /// `toPlainString()`
    pub fn to_plain_string(&self) -> String {
        let coeff = self.unscaled.abs().to_string();
        let mut out = String::new();
        if self.unscaled.is_negative() {
            out.push('-');
        }
        if self.scale <= 0 {
            out.push_str(&coeff);
            for _ in 0..(-(self.scale as i64)) {
                out.push('0');
            }
            return out;
        }
        let scale = self.scale as usize;
        if coeff.len() > scale {
            out.push_str(&coeff[..coeff.len() - scale]);
            out.push('.');
            out.push_str(&coeff[coeff.len() - scale..]);
        } else {
            out.push_str("0.");
            for _ in 0..(scale - coeff.len()) {
                out.push('0');
            }
            out.push_str(&coeff);
        }
        out
    }

    /// `compareTo` (numeric, scale-insensitive)
    pub fn compare_to(&self, other: &BigDecimal) -> Ordering {
        let (a, b) = align(self, other);
        a.cmp(&b)
    }

    /// `stripTrailingZeros()`
    pub fn strip_trailing_zeros(&self) -> BigDecimal {
        if self.unscaled.is_zero() {
            return BigDecimal { unscaled: BigInt::zero(), scale: 0 };
        }
        let ten = BigInt::from(10);
        let mut u = self.unscaled.clone();
        let mut s = self.scale as i64;
        while (&u % &ten).is_zero() {
            u /= &ten;
            s -= 1;
        }
        BigDecimal { unscaled: u, scale: s as i32 }
    }

    /// `doubleValue()`
    pub fn to_f64(&self) -> f64 {
        self.to_java_string().parse::<f64>().unwrap_or(f64::NAN)
    }

    /// `toBigInteger()` (truncating)
    pub fn to_bigint_truncated(&self) -> BigInt {
        if self.scale <= 0 {
            &self.unscaled * BigInt::from(10).pow((-(self.scale as i64)) as u32)
        } else {
            &self.unscaled / BigInt::from(10).pow(self.scale as u32)
        }
    }

    /// `longValue()` (truncating, wrapping like Java)
    pub fn to_i64_wrapping(&self) -> i64 {
        wrap_i64(&self.to_bigint_truncated())
    }

    /// `intValue()`
    pub fn to_i32_wrapping(&self) -> i32 {
        self.to_i64_wrapping() as i32
    }

    /// `longValueExact()` as an Option.
    pub fn to_i64_exact(&self) -> Option<i64> {
        let s = self.strip_trailing_zeros();
        if s.scale > 0 {
            return None;
        }
        s.to_bigint_truncated().to_i64()
    }

    /// `hashCode()`: 31*unscaled.hashCode() + scale
    pub fn java_hash_code(&self) -> i32 {
        big_integer_hash_code(&self.unscaled).wrapping_mul(31).wrapping_add(self.scale)
    }
}

/// Two's-complement truncation of a BigInteger to 64 bits (`BigInteger.longValue()`).
pub fn wrap_i64(v: &BigInt) -> i64 {
    let bytes = v.to_signed_bytes_le();
    let mut buf = [if v.is_negative() { 0xffu8 } else { 0 }; 8];
    for (i, b) in bytes.iter().take(8).enumerate() {
        buf[i] = *b;
    }
    i64::from_le_bytes(buf)
}

/// `BigInteger.hashCode()`
pub fn big_integer_hash_code(v: &BigInt) -> i32 {
    let (sign, digits) = v.to_u32_digits();
    let mut h: i32 = 0;
    for d in digits.iter().rev() {
        h = h.wrapping_mul(31).wrapping_add(*d as i32);
    }
    match sign {
        Sign::Minus => h.wrapping_neg(),
        _ => h,
    }
}

fn align(a: &BigDecimal, b: &BigDecimal) -> (BigInt, BigInt) {
    let ten = BigInt::from(10);
    if a.scale == b.scale {
        (a.unscaled.clone(), b.unscaled.clone())
    } else if a.scale > b.scale {
        let f = ten.pow((a.scale as i64 - b.scale as i64) as u32);
        (a.unscaled.clone(), &b.unscaled * f)
    } else {
        let f = ten.pow((b.scale as i64 - a.scale as i64) as u32);
        (&a.unscaled * f, b.unscaled.clone())
    }
}

/// `equals`: value AND scale must match (2.0 != 2.00), as in Java.
impl PartialEq for BigDecimal {
    fn eq(&self, other: &Self) -> bool {
        self.scale == other.scale && self.unscaled == other.unscaled
    }
}

impl Eq for BigDecimal {}

impl std::fmt::Display for BigDecimal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_java_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(x: &str) -> String {
        BigDecimal::parse(x).unwrap().to_java_string()
    }

    #[test]
    fn to_string_rules() {
        assert_eq!(s("1"), "1");
        assert_eq!(s("1.50"), "1.50");
        assert_eq!(s("0.000001"), "0.000001");
        assert_eq!(s("0.0000001"), "1E-7");
        assert_eq!(s("1E3"), "1E+3");
        assert_eq!(s("1.5E3"), "1.5E+3");
        assert_eq!(s("-12.345e1"), "-123.45");
        assert_eq!(s("0.00"), "0.00");
        assert_eq!(s("1.0E400"), "1.0E+400");
        assert_eq!(BigDecimal::parse("1E3").unwrap().to_plain_string(), "1000");
        assert_eq!(BigDecimal::parse("0.0000001").unwrap().to_plain_string(), "0.0000001");
        assert_eq!(BigDecimal::value_of_f64(1.0).to_java_string(), "1.0");
        assert_eq!(BigDecimal::value_of_f64(1e20).to_java_string(), "1.0E+20");
        assert_eq!(BigDecimal::from_f64_exact(0.5).to_java_string(), "0.5");
    }
}
