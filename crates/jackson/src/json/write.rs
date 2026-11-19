//! `WriterBasedJsonGenerator` with optional `DefaultPrettyPrinter`.

use crate::JacksonError;
use crate::bigdecimal::BigDecimal;
use crate::generator::JsonGenerator;
use num_bigint::BigInt;

/// Generator features relevant to the swagger mappers.
#[derive(Clone, Copy, Debug)]
pub struct WriteFeatures {
    /// `writerWithDefaultPrettyPrinter()`
    pub pretty: bool,
    /// `SerializationFeature.WRITE_BIGDECIMAL_AS_PLAIN` / `StreamWriteFeature.WRITE_BIGDECIMAL_AS_PLAIN`
    pub big_decimal_as_plain: bool,
    /// `JsonWriteFeature.QUOTE_NON_NUMERIC_NUMBERS` (default on)
    pub quote_non_numeric_numbers: bool,
}

impl Default for WriteFeatures {
    fn default() -> Self {
        WriteFeatures { pretty: false, big_decimal_as_plain: false, quote_non_numeric_numbers: true }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Scope {
    Object,
    Array,
}

struct Level {
    scope: Scope,
    entries: usize,
    /// object: a field name was written and its value is pending
    expecting_value: bool,
}

/// Writes JSON text into a `String`.
pub struct JsonWriter {
    out: String,
    features: WriteFeatures,
    stack: Vec<Level>,
    nesting: usize,
    root_values: usize,
}

const HEX: &[u8; 16] = b"0123456789ABCDEF";

/// Appends `s` as a quoted JSON string with Jackson's default escaping.
pub fn quote_into(out: &mut String, s: &str) {
    out.push('"');
    escape_into(out, s);
    out.push('"');
}

/// Jackson's `sOutputEscapes128`: `"` `\\` and control chars; `\b \t \f \n \r` get short escapes, the rest `\u00XX`.
pub fn escape_into(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 32 => {
                let v = c as u32;
                out.push_str("\\u00");
                out.push(HEX[(v >> 4) as usize] as char);
                out.push(HEX[(v & 15) as usize] as char);
            }
            c => out.push(c),
        }
    }
}

impl JsonWriter {
    /// A writer with an empty output buffer and the given features.
    pub fn new(features: WriteFeatures) -> JsonWriter {
        JsonWriter { out: String::new(), features, stack: Vec::new(), nesting: 0, root_values: 0 }
    }

    /// The JSON text written so far.
    pub fn into_string(self) -> String {
        self.out
    }

    /// The JSON text written so far, borrowed.
    pub fn as_str(&self) -> &str {
        &self.out
    }

    fn indent(&mut self) {
        self.out.push('\n');
        for _ in 0..self.nesting {
            self.out.push_str("  ");
        }
    }

    /// Separator handling before any value (scalar or container start).
    fn before_value(&mut self) {
        match self.stack.last_mut() {
            None => {
                if self.root_values > 0 {
                    self.out.push(' ');
                }
                self.root_values += 1;
            }
            Some(level) => match level.scope {
                Scope::Object => {
                    level.expecting_value = false;
                }
                Scope::Array => {
                    let first = level.entries == 0;
                    level.entries += 1;
                    if self.features.pretty {
                        if first {
                            self.out.push(' ');
                        } else {
                            self.out.push_str(", ");
                        }
                    } else if !first {
                        self.out.push(',');
                    }
                }
            },
        }
    }

    fn raw_number(&mut self, s: &str) -> Result<(), JacksonError> {
        self.before_value();
        self.out.push_str(s);
        Ok(())
    }

    fn non_numeric(&mut self, s: &str) -> Result<(), JacksonError> {
        if self.features.quote_non_numeric_numbers {
            self.before_value();
            self.out.push('"');
            self.out.push_str(s);
            self.out.push('"');
            Ok(())
        } else {
            self.raw_number(s)
        }
    }
}

impl JsonGenerator for JsonWriter {
    fn write_start_object(&mut self) -> Result<(), JacksonError> {
        self.before_value();
        self.out.push('{');
        if self.features.pretty {
            self.nesting += 1;
        }
        self.stack.push(Level { scope: Scope::Object, entries: 0, expecting_value: false });
        Ok(())
    }

    fn write_end_object(&mut self) -> Result<(), JacksonError> {
        let level = self.stack.pop().ok_or_else(|| gen_err("Current context not Object but root"))?;
        if level.scope != Scope::Object {
            return Err(gen_err("Current context not Object but Array"));
        }
        if self.features.pretty {
            self.nesting -= 1;
            if level.entries > 0 {
                self.indent();
            } else {
                self.out.push(' ');
            }
        }
        self.out.push('}');
        Ok(())
    }

    fn write_start_array(&mut self) -> Result<(), JacksonError> {
        self.before_value();
        self.out.push('[');
        self.stack.push(Level { scope: Scope::Array, entries: 0, expecting_value: false });
        Ok(())
    }

    fn write_end_array(&mut self) -> Result<(), JacksonError> {
        let level = self.stack.pop().ok_or_else(|| gen_err("Current context not Array but root"))?;
        if level.scope != Scope::Array {
            return Err(gen_err("Current context not Array but Object"));
        }
        if self.features.pretty {
            self.out.push(' ');
        }
        self.out.push(']');
        Ok(())
    }

    fn write_field_name(&mut self, name: &str) -> Result<(), JacksonError> {
        let pretty = self.features.pretty;
        let level = match self.stack.last_mut() {
            Some(l) if l.scope == Scope::Object && !l.expecting_value => l,
            _ => return Err(gen_err("Can not write a field name, expecting a value")),
        };
        let first = level.entries == 0;
        level.entries += 1;
        level.expecting_value = true;
        if !first {
            self.out.push(',');
        }
        if pretty {
            self.indent();
        }
        quote_into(&mut self.out, name);
        if pretty {
            self.out.push_str(" : ");
        } else {
            self.out.push(':');
        }
        Ok(())
    }

    fn write_string(&mut self, value: &str) -> Result<(), JacksonError> {
        self.before_value();
        quote_into(&mut self.out, value);
        Ok(())
    }

    fn write_boolean(&mut self, value: bool) -> Result<(), JacksonError> {
        self.raw_number(if value { "true" } else { "false" })
    }

    fn write_null(&mut self) -> Result<(), JacksonError> {
        self.raw_number("null")
    }

    fn write_number_i16(&mut self, value: i16) -> Result<(), JacksonError> {
        self.raw_number(&value.to_string())
    }

    fn write_number_i32(&mut self, value: i32) -> Result<(), JacksonError> {
        self.raw_number(&value.to_string())
    }

    fn write_number_i64(&mut self, value: i64) -> Result<(), JacksonError> {
        self.raw_number(&value.to_string())
    }

    fn write_number_big_integer(&mut self, value: &BigInt) -> Result<(), JacksonError> {
        self.raw_number(&value.to_string())
    }

    fn write_number_f32(&mut self, value: f32) -> Result<(), JacksonError> {
        if !value.is_finite() {
            return self.non_numeric(&java_compat::float_to_string(value));
        }
        self.raw_number(&java_compat::float_to_string(value))
    }

    fn write_number_f64(&mut self, value: f64) -> Result<(), JacksonError> {
        if !value.is_finite() {
            return self.non_numeric(&java_compat::double_to_string(value));
        }
        self.raw_number(&java_compat::double_to_string(value))
    }

    fn write_number_big_decimal(&mut self, value: &BigDecimal) -> Result<(), JacksonError> {
        let s = if self.features.big_decimal_as_plain { value.to_plain_string() } else { value.to_java_string() };
        self.raw_number(&s)
    }

    fn write_binary(&mut self, value: &[u8]) -> Result<(), JacksonError> {
        self.before_value();
        self.out.push('"');
        self.out.push_str(&base64_mime_no_linefeeds(value));
        self.out.push('"');
        Ok(())
    }
}

fn gen_err(msg: &str) -> JacksonError {
    JacksonError { class: "com.fasterxml.jackson.core.JsonGenerationException".into(), message: msg.into() }
}

/// `Base64Variants.MIME_NO_LINEFEEDS` encoding (standard alphabet, padded).
pub fn base64_mime_no_linefeeds(data: &[u8]) -> String {
    const ALPHA: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHA[(n >> 18) as usize & 63] as char);
        out.push(ALPHA[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { ALPHA[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { ALPHA[n as usize & 63] as char } else { '=' });
    }
    out
}
