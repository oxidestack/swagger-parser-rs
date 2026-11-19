//! `YAMLGenerator` (jackson-dataformat-yaml 2.22) over the SnakeYAML emitter.

use crate::bigdecimal::BigDecimal;
use crate::generator::JsonGenerator;
use crate::JacksonError;
use num_bigint::BigInt;
use snakeyaml::emitter::{DumperOptions, Emitter};
use snakeyaml::events::{Event, EventKind, ImplicitTuple};
use snakeyaml::options::{FlowStyle, ScalarStyle};

/// `YAMLGenerator.Feature` flags
#[derive(Clone, Copy, Debug)]
pub struct YamlGeneratorFeatures {
    /// `WRITE_DOC_START_MARKER`: begin the document with `---`.
    pub write_doc_start_marker: bool,
    /// `SPLIT_LINES`: let the emitter fold long plain and quoted scalars across lines.
    pub split_lines: bool,
    /// `MINIMIZE_QUOTES`: write strings plain when YAML would read them back as the same string.
    pub minimize_quotes: bool,
    /// `ALWAYS_QUOTE_NUMBERS_AS_STRINGS`: with `MINIMIZE_QUOTES`, still quote strings that look like numbers.
    pub always_quote_numbers_as_strings: bool,
    /// `LITERAL_BLOCK_STYLE`: write multi-line strings as `|` literal blocks.
    pub literal_block_style: bool,
    /// `INDENT_ARRAYS`: indent sequence items under their parent key.
    pub indent_arrays: bool,
    /// `INDENT_ARRAYS_WITH_INDICATOR`: indent sequence items with the `-` indicator counted in the indentation.
    pub indent_arrays_with_indicator: bool,
    /// `ALLOW_LONG_KEYS`: allow implicit keys up to 1024 characters instead of 128.
    pub allow_long_keys: bool,
    /// `WRITE_BIGDECIMAL_AS_PLAIN` (a generator feature set by the mapper)
    pub big_decimal_as_plain: bool,
}

impl Default for YamlGeneratorFeatures {
    fn default() -> Self {
        YamlGeneratorFeatures {
            write_doc_start_marker: true,
            split_lines: true,
            minimize_quotes: false,
            always_quote_numbers_as_strings: false,
            literal_block_style: false,
            indent_arrays: false,
            indent_arrays_with_indicator: false,
            allow_long_keys: false,
            big_decimal_as_plain: false,
        }
    }
}

impl YamlGeneratorFeatures {
    /// swagger-core `ObjectMapperFactory.createYaml()`: no document start marker, MINIMIZE_QUOTES, SPLIT_LINES,
    /// ALWAYS_QUOTE_NUMBERS_AS_STRINGS, WRITE_BIGDECIMAL_AS_PLAIN.
    pub fn swagger() -> YamlGeneratorFeatures {
        YamlGeneratorFeatures {
            write_doc_start_marker: false,
            minimize_quotes: true,
            split_lines: true,
            always_quote_numbers_as_strings: true,
            big_decimal_as_plain: true,
            ..Default::default()
        }
    }
}

const RESERVED_KEYWORDS: &[&str] = &[
    "false", "False", "FALSE", "n", "N", "no", "No", "NO", "null", "Null", "NULL", "on", "On", "ON", "off", "Off", "OFF", "true", "True", "TRUE", "y", "Y", "yes", "Yes",
    "YES",
];

/// `StringQuotingChecker.isReservedKeyword`
fn is_reserved_keyword(v: &str) -> bool {
    let Some(first) = v.chars().next() else { return true };
    match first {
        'F' | 'N' | 'O' | 'T' | 'Y' | 'f' | 'n' | 'o' | 't' | 'y' => RESERVED_KEYWORDS.contains(&v),
        '~' => true,
        _ => false,
    }
}

fn looks_like_yaml_number(v: &str) -> bool {
    matches!(v.chars().next(), Some('+' | '-' | '.' | '0'..='9'))
}

fn is_blank(c: u16) -> bool {
    c == b' ' as u16 || c == b'\t' as u16
}

/// `valueHasQuotableChar`
fn value_has_quotable_char(v: &str) -> bool {
    let s: Vec<u16> = v.encode_utf16().collect();
    for (i, &c) in s.iter().enumerate() {
        match c {
            0x23 /* # */ if i == 0 || is_blank(s[i - 1]) => return true,
            0x2c | 0x5b | 0x5d | 0x7b | 0x7d => return true,
            0x3a /* : */ if i == s.len() - 1 || is_blank(s[i + 1]) => return true,
            _ => {}
        }
    }
    false
}

fn need_to_quote_name(name: &str) -> bool {
    is_reserved_keyword(name) || looks_like_yaml_number(name) || name.encode_utf16().any(|c| c < 32)
}

fn need_to_quote_value(value: &str) -> bool {
    is_reserved_keyword(value) || value_has_quotable_char(value)
}

/// `[+-]?[0-9]*(\.[0-9]*)?` (whole string)
fn plain_number(v: &str) -> bool {
    let b = v.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    if i < b.len() && b[i] == b'.' {
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
    }
    i == b.len()
}

fn emitter_error(e: snakeyaml::emitter::EmitterError) -> JacksonError {
    JacksonError { class: "org.yaml.snakeyaml.emitter.EmitterException".into(), message: e.0 }
}

/// `YAMLGenerator`: a [`JsonGenerator`] that emits YAML text through the SnakeYAML emitter.
pub struct YamlWriter {
    emitter: Emitter,
    features: YamlGeneratorFeatures,
    closed: bool,
}

impl YamlWriter {
    /// A generator with the given features; the stream and document start are already emitted.
    pub fn new(features: YamlGeneratorFeatures) -> YamlWriter {
        let mut opts = DumperOptions { default_flow_style: FlowStyle::Block, split_lines: features.split_lines, ..Default::default() };
        if features.indent_arrays {
            opts.indicator_indent = 1;
            opts.indent = 2;
        }
        if features.indent_arrays_with_indicator {
            opts.indicator_indent = 2;
            opts.indent_with_indicator = true;
        }
        if features.allow_long_keys {
            opts.max_simple_key_length = 1024;
        }
        let mut w = YamlWriter { emitter: Emitter::new(&opts), features, closed: false };
        let _ = w.emit(EventKind::StreamStart);
        let _ = w.emit(EventKind::DocumentStart { explicit: features.write_doc_start_marker, version: None, tags: None });
        w
    }

    fn emit(&mut self, k: EventKind) -> Result<(), JacksonError> {
        self.emitter.emit(Event::unmarked(k)).map_err(emitter_error)
    }

    fn scalar(&mut self, value: &str, style: ScalarStyle) -> Result<(), JacksonError> {
        self.emit(EventKind::Scalar { anchor: None, tag: None, implicit: ImplicitTuple { plain: true, non_plain: true }, value: value.to_string(), style })
    }

    /// `close()`: ends the document and the stream; returns the YAML text.
    pub fn into_string(mut self) -> Result<String, JacksonError> {
        if !self.closed {
            self.emit(EventKind::DocumentEnd { explicit: false })?;
            self.emit(EventKind::StreamEnd)?;
            self.closed = true;
        }
        Ok(self.emitter.output())
    }
}

impl JsonGenerator for YamlWriter {
    fn write_start_object(&mut self) -> Result<(), JacksonError> {
        self.emit(EventKind::MappingStart { anchor: None, tag: None, implicit: true, flow_style: FlowStyle::Block })
    }
    fn write_end_object(&mut self) -> Result<(), JacksonError> {
        self.emit(EventKind::MappingEnd)
    }
    fn write_start_array(&mut self) -> Result<(), JacksonError> {
        self.emit(EventKind::SequenceStart { anchor: None, tag: None, implicit: true, flow_style: FlowStyle::Block })
    }
    fn write_end_array(&mut self) -> Result<(), JacksonError> {
        self.emit(EventKind::SequenceEnd)
    }
    fn write_field_name(&mut self, name: &str) -> Result<(), JacksonError> {
        let style = if need_to_quote_name(name) { ScalarStyle::DoubleQuoted } else { ScalarStyle::Plain };
        self.scalar(name, style)
    }
    fn write_string(&mut self, text: &str) -> Result<(), JacksonError> {
        if text.is_empty() {
            return self.scalar(text, ScalarStyle::DoubleQuoted);
        }
        let style = if self.features.minimize_quotes {
            if text.contains('\n') {
                ScalarStyle::Literal
            } else if !need_to_quote_value(text) && (!self.features.always_quote_numbers_as_strings || !plain_number(text)) {
                ScalarStyle::Plain
            } else {
                ScalarStyle::DoubleQuoted
            }
        } else if self.features.literal_block_style && text.contains('\n') {
            ScalarStyle::Literal
        } else {
            ScalarStyle::DoubleQuoted
        };
        self.scalar(text, style)
    }
    fn write_boolean(&mut self, value: bool) -> Result<(), JacksonError> {
        self.scalar(if value { "true" } else { "false" }, ScalarStyle::Plain)
    }
    fn write_null(&mut self) -> Result<(), JacksonError> {
        self.scalar("null", ScalarStyle::Plain)
    }
    fn write_number_i16(&mut self, value: i16) -> Result<(), JacksonError> {
        self.scalar(&value.to_string(), ScalarStyle::Plain)
    }
    fn write_number_i32(&mut self, value: i32) -> Result<(), JacksonError> {
        self.scalar(&value.to_string(), ScalarStyle::Plain)
    }
    fn write_number_i64(&mut self, value: i64) -> Result<(), JacksonError> {
        self.scalar(&value.to_string(), ScalarStyle::Plain)
    }
    fn write_number_big_integer(&mut self, value: &BigInt) -> Result<(), JacksonError> {
        self.scalar(&value.to_string(), ScalarStyle::Plain)
    }
    fn write_number_f32(&mut self, value: f32) -> Result<(), JacksonError> {
        self.scalar(&java_compat::float_to_string(value), ScalarStyle::Plain)
    }
    fn write_number_f64(&mut self, value: f64) -> Result<(), JacksonError> {
        self.scalar(&java_compat::double_to_string(value), ScalarStyle::Plain)
    }
    fn write_number_big_decimal(&mut self, value: &BigDecimal) -> Result<(), JacksonError> {
        let s = if self.features.big_decimal_as_plain { value.to_plain_string() } else { value.to_java_string() };
        self.scalar(&s, ScalarStyle::Plain)
    }
    fn write_binary(&mut self, value: &[u8]) -> Result<(), JacksonError> {
        // Base64Variants.MIME with "\n" line feeds every 76 characters
        let flat = crate::json::write::base64_mime_no_linefeeds(value);
        let mut encoded = String::new();
        for (i, ch) in flat.chars().enumerate() {
            if i > 0 && i % 76 == 0 {
                encoded.push('\n');
            }
            encoded.push(ch);
        }
        self.emit(EventKind::Scalar {
            anchor: None,
            tag: Some("tag:yaml.org,2002:binary".into()),
            implicit: ImplicitTuple { plain: false, non_plain: false },
            value: encoded,
            style: ScalarStyle::Literal,
        })
    }
}
