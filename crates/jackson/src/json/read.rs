//! `ObjectMapper.readTree(String)`: a port of `ReaderBasedJsonParser` (default features) feeding
//! `JsonNodeDeserializer`, with Jackson 2.22's exact exception messages and locations; and
//! `ObjectMapper.readTree(byte[])` ([`read_tree_bytes`]): `ByteSourceJsonBootstrapper`'s encoding detection, then
//! UTF-8 through the same parser reading bytes as `UTF8StreamJsonParser` does, UTF-16 and UTF-32 as chars.

use crate::JacksonError;
use crate::node::{JsonNode, ObjectNode};
use num_bigint::BigInt;
use std::sync::Arc;

const SOURCE: &str = "[Source: REDACTED (`StreamReadFeature.INCLUDE_SOURCE_IN_LOCATION` disabled); ";
const VALID_VALUES: &str = "(JSON String, Number, Array, Object or token 'null', 'true' or 'false')";
/// `_validJsonValueList()` with `ALLOW_NON_NUMERIC_NUMBERS`.
const VALID_VALUES_NON_NUMERIC: &str = "(JSON String, Number (or 'NaN'/'+INF'/'-INF'), Array, Object or token 'null', 'true' or 'false')";
const MAX_NESTING_DEPTH: usize = 1000;
const MAX_ERROR_TOKEN_LENGTH: usize = 256;

/// Parser features the swagger mappers toggle.
#[derive(Clone, Copy, Debug, Default)]
pub struct ReadFeatures {
    /// `StreamReadFeature.STRICT_DUPLICATE_DETECTION`
    pub strict_duplicate_detection: bool,
    /// `DeserializationFeature.USE_BIG_DECIMAL_FOR_FLOATS`
    pub use_big_decimal_for_floats: bool,
    /// `JsonParser.Feature.ALLOW_UNQUOTED_FIELD_NAMES`
    pub allow_unquoted_field_names: bool,
    /// `JsonParser.Feature.ALLOW_SINGLE_QUOTES` (for field names)
    pub allow_single_quotes: bool,
    /// Read member values the way `JsonNodeDeserializer` does, through `nextFieldName()`: there a `}` where a value
    /// should be is an odd value ("expected a valid value …"), not the "expected a value" of `nextToken()`.
    pub tree_member_values: bool,
    /// `JsonReadFeature.ALLOW_NON_NUMERIC_NUMBERS`: `NaN`, `Infinity`, `+INF`, `-INF` (and signed `Infinity`) read as
    /// doubles.
    pub allow_non_numeric_numbers: bool,
    /// `JsonReadFeature.ALLOW_JAVA_COMMENTS`: `/* … */` and `// …` between tokens.
    pub allow_comments: bool,
    /// `JsonReadFeature.ALLOW_YAML_COMMENTS`: `# …` to the end of the line between tokens.
    pub allow_yaml_comments: bool,
    /// `JsonReadFeature.ALLOW_UNESCAPED_CONTROL_CHARS`: control characters (up to space) inside strings and names.
    pub allow_unquoted_control_chars: bool,
    /// `JsonReadFeature.ALLOW_BACKSLASH_ESCAPING_ANY_CHARACTER`: `\x` for any `x` reads as `x`.
    pub allow_backslash_escaping_any_character: bool,
    /// `JsonReadFeature.ALLOW_LEADING_ZEROS_FOR_NUMBERS`: `007` reads as `7`.
    pub allow_numeric_leading_zeros: bool,
    /// `JsonReadFeature.ALLOW_MISSING_VALUES`: an empty array element (`[1,,2]`, `[,1]`) or member value reads as null.
    pub allow_missing_values: bool,
    /// `JsonReadFeature.ALLOW_TRAILING_COMMA`: a comma right before the closing `]` or `}`.
    pub allow_trailing_comma: bool,
}

impl ReadFeatures {
    /// The parser features MockServer's `ObjectMapperFactory` enables (member values read through `nextFieldName`,
    /// as `readTree` does).
    pub fn mockserver() -> ReadFeatures {
        ReadFeatures {
            allow_unquoted_field_names: true,
            allow_single_quotes: true,
            tree_member_values: true,
            allow_non_numeric_numbers: true,
            allow_comments: true,
            allow_yaml_comments: true,
            allow_unquoted_control_chars: true,
            allow_backslash_escaping_any_character: true,
            allow_numeric_leading_zeros: true,
            allow_missing_values: true,
            allow_trailing_comma: true,
            ..Default::default()
        }
    }
}

/// Parses `text` the way `ObjectMapper.readTree(String)` does. Empty (or whitespace-only) content yields
/// `MissingNode`. Only the first root value is read (trailing content is not checked).
pub fn read_tree(text: &str, features: ReadFeatures) -> Result<JsonNode, JacksonError> {
    let buf: Vec<u16> = text.encode_utf16().collect();
    Parser::new(buf, false, features).read()
}

/// Parses `bytes` the way `ObjectMapper.readTree(byte[])` does: the encoding is detected as
/// `ByteSourceJsonBootstrapper.detectEncoding` does (a BOM, else the zero bytes of the first four); UTF-8 is read
/// byte by byte as `UTF8StreamJsonParser` reads it (columns count bytes, the BOM's included, and invalid UTF-8 is
/// reported where that parser finds it), UTF-16 is decoded as `new String(bytes, "UTF-16BE"/"UTF-16LE")` does
/// (malformed units become U+FFFD) and UTF-32 as `UTF32Reader` does (an invalid character fails the 4000-char read
/// that would hold it), both then read as chars. Only the default features are byte-exact here (comments,
/// apostrophes and unquoted names are read as the char parser reads them).
pub fn read_tree_bytes(bytes: &[u8], features: ReadFeatures) -> Result<JsonNode, JacksonError> {
    match detect_encoding(bytes)? {
        Encoding::Utf8 { skip } => {
            let mut p = Parser::new(bytes.iter().map(|&b| b as u16).collect(), true, features);
            p.ptr = skip;
            p.read()
        }
        Encoding::Utf16 { big_endian, skip } => Parser::new(java_utf16(&bytes[skip..], big_endian), false, features).read(),
        Encoding::Utf32 { big_endian, skip } => {
            let (buf, failure) = utf32_reader(bytes, skip, big_endian);
            match failure {
                None => Parser::new(buf, false, features).read(),
                Some((cut, e)) => {
                    // the parser sees the chars of the reads before the failing one, and fails when it asks for more
                    let mut p = Parser::new(buf[..cut].to_vec(), false, features);
                    let result = p.read();
                    if p.hit_end.get() { Err(e) } else { result }
                }
            }
        }
    }
}

enum Encoding {
    Utf8 { skip: usize },
    Utf16 { big_endian: bool, skip: usize },
    Utf32 { big_endian: bool, skip: usize },
}

fn char_conversion(message: String) -> JacksonError {
    JacksonError { class: "java.io.CharConversionException".into(), message }
}

fn weird_ucs4(kind: &str) -> JacksonError {
    char_conversion(format!("Unsupported UCS-4 endianness ({kind}) detected"))
}

/// `ByteSourceJsonBootstrapper.detectEncoding`: `handleBOM`, then `checkUTF32` and `checkUTF16` on the first bytes.
fn detect_encoding(b: &[u8]) -> Result<Encoding, JacksonError> {
    let check_utf16 = |i16: u32| -> Option<Encoding> {
        if i16 & 0xFF00 == 0 {
            Some(Encoding::Utf16 { big_endian: true, skip: 0 })
        } else if i16 & 0x00FF == 0 {
            Some(Encoding::Utf16 { big_endian: false, skip: 0 })
        } else {
            None
        }
    };
    if b.len() >= 4 {
        let quad = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
        match quad {
            0x0000_FEFF => return Ok(Encoding::Utf32 { big_endian: true, skip: 4 }),
            0xFFFE_0000 => return Ok(Encoding::Utf32 { big_endian: false, skip: 4 }),
            0x0000_FFFE => return Err(weird_ucs4("2143")),
            0xFEFF_0000 => return Err(weird_ucs4("3412")),
            _ => {}
        }
        match quad >> 16 {
            0xFEFF => return Ok(Encoding::Utf16 { big_endian: true, skip: 2 }),
            0xFFFE => return Ok(Encoding::Utf16 { big_endian: false, skip: 2 }),
            _ => {}
        }
        if quad >> 8 == 0xEF_BBBF {
            return Ok(Encoding::Utf8 { skip: 3 });
        }
        if quad >> 8 == 0 {
            return Ok(Encoding::Utf32 { big_endian: true, skip: 0 });
        }
        if quad & 0x00FF_FFFF == 0 {
            return Ok(Encoding::Utf32 { big_endian: false, skip: 0 });
        }
        if quad & !0x00FF_0000 == 0 {
            return Err(weird_ucs4("3412"));
        }
        if quad & !0x0000_FF00 == 0 {
            return Err(weird_ucs4("2143"));
        }
        return Ok(check_utf16(quad >> 16).unwrap_or(Encoding::Utf8 { skip: 0 }));
    }
    if b.len() >= 2 {
        return Ok(check_utf16(((b[0] as u32) << 8) | b[1] as u32).unwrap_or(Encoding::Utf8 { skip: 0 }));
    }
    Ok(Encoding::Utf8 { skip: 0 })
}

/// The JDK's `UTF-16BE`/`UTF-16LE` decoder with `REPLACE` (`sun.nio.cs.UnicodeDecoder`): an unpaired low surrogate
/// is one U+FFFD; a high surrogate not followed by a low one is one U+FFFD for both units;
/// bytes left at the end (an odd byte, or a high surrogate with less than a unit after it) are one U+FFFD.
fn java_utf16(b: &[u8], big_endian: bool) -> Vec<u16> {
    let unit = |i: usize| if big_endian { u16::from_be_bytes([b[i], b[i + 1]]) } else { u16::from_le_bytes([b[i], b[i + 1]]) };
    let mut out = Vec::with_capacity(b.len() / 2 + 1);
    let mut i = 0;
    while i + 2 <= b.len() {
        let c = unit(i);
        if (0xDC00..=0xDFFF).contains(&c) {
            out.push(0xFFFD);
            i += 2;
        } else if (0xD800..=0xDBFF).contains(&c) {
            if i + 4 > b.len() {
                break;
            }
            let c2 = unit(i + 2);
            if (0xDC00..=0xDFFF).contains(&c2) {
                out.push(c);
                out.push(c2);
            } else {
                out.push(0xFFFD);
            }
            i += 4;
        } else {
            out.push(c);
            i += 2;
        }
    }
    if i < b.len() {
        out.push(0xFFFD);
    }
    out
}

/// `ReaderBasedJsonParser`'s input buffer (`IOContext.allocTokenBuffer()`, `BufferRecycler.CHAR_TOKEN_BUFFER`).
const READER_CHUNK: usize = 4000;

/// `UTF32Reader` over the whole input: the chars, and when it fails, how many chars the reads before the failing
/// one delivered, with its `CharConversionException`.
fn utf32_reader(b: &[u8], start: usize, big_endian: bool) -> (Vec<u16>, Option<(usize, JacksonError)>) {
    let mut out: Vec<u16> = Vec::new();
    let mut ptr = start;
    while ptr + 4 <= b.len() {
        let (hi, lo) = if big_endian {
            (u16::from_be_bytes([b[ptr], b[ptr + 1]]) as u32, u16::from_be_bytes([b[ptr + 2], b[ptr + 3]]) as u32)
        } else {
            (u16::from_le_bytes([b[ptr + 2], b[ptr + 3]]) as u32, u16::from_le_bytes([b[ptr], b[ptr + 1]]) as u32)
        };
        ptr += 4;
        if hi != 0 {
            let ch = (hi.wrapping_sub(1) << 16) | lo;
            if hi > 0x10 {
                let e = char_conversion(format!(
                    "Invalid UTF-32 character 0x{ch:x} (above 0x0010ffff) at char #{}, byte #{})",
                    out.len(),
                    ptr - 1
                ));
                let cut = out.len() - out.len() % READER_CHUNK;
                return (out, Some((cut, e)));
            }
            out.push((0xD800 + (ch >> 10)) as u16);
            out.push((0xDC00 | (ch & 0x3FF)) as u16);
        } else {
            out.push(lo as u16);
        }
    }
    let left = b.len() - ptr;
    if left > 0 {
        let e = char_conversion(format!(
            "Unexpected EOF in the middle of a 4-byte UTF-32 char: got {left}, needed 4, at char #{}, byte #{left})",
            out.len()
        ));
        let cut = out.len();
        return (out, Some((cut, e)));
    }
    (out, None)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CtxType {
    Root,
    Array,
    Object,
}

struct Ctx {
    kind: CtxType,
    index: i32,
    start_row: usize,
    start_col: usize,
    names: Option<std::collections::HashSet<String>>,
}

impl Ctx {
    fn root() -> Ctx {
        Ctx { kind: CtxType::Root, index: -1, start_row: 1, start_col: 0, names: None }
    }
    fn type_desc(&self) -> &'static str {
        match self.kind {
            CtxType::Root => "root",
            CtxType::Array => "Array",
            CtxType::Object => "Object",
        }
    }
    fn expect_comma(&mut self) -> bool {
        self.index += 1;
        self.kind != CtxType::Root && self.index > 0
    }
}

#[derive(Debug)]
enum Token {
    StartObject,
    EndObject,
    StartArray,
    EndArray,
    Field(String),
    Str(String),
    Int { text: String, int_len: usize, neg: bool },
    Float(String),
    True,
    False,
    Null,
}

struct Parser {
    /// UTF-16 units, or (when `utf8`) the input bytes
    buf: Vec<u16>,
    utf8: bool,
    /// set once the parser looked past the end of `buf` (a reader would have been asked for more)
    hit_end: std::cell::Cell<bool>,
    /// `_currToken` while the next token is read, for `_reportInvalidEOF()`
    last_token: &'static str,
    ptr: usize,
    row: usize,
    row_start: usize,
    ctx: Vec<Ctx>,
    features: ReadFeatures,
    token_row: usize,
    token_col: usize,
    pending: Option<i32>,
}

fn char_desc(ch: i32) -> String {
    // `_getCharDesc` shows `(char) ch` (a supplementary code point cut to its low 16 bits) and the whole code
    let c = ch as u16 as u32;
    let is_control = c <= 0x1f || (0x7f..=0x9f).contains(&c);
    if is_control {
        format!("(CTRL-CHAR, code {ch})")
    } else {
        // a lone surrogate is `?`, as Java's UTF-8 encoder writes it
        let s = char::from_u32(c).map_or_else(|| "?".to_string(), |c| c.to_string());
        if ch > 255 { format!("'{s}' (code {ch} / 0x{:x})", ch) } else { format!("'{s}' (code {ch})") }
    }
}

/// `Character.isJavaIdentifierPart` for a UTF-16 unit (approximation adequate for token error reporting): identifier
/// starts, digits, and the identifier-ignorable controls and format characters.
pub fn is_java_identifier_part(c: u16) -> bool {
    match char::from_u32(c as u32) {
        Some(ch) => {
            ch.is_alphanumeric()
                || is_currency_or_connector(c)
                || (c <= 8)
                || (0x0e..=0x1b).contains(&c)
                || (0x7f..=0x9f).contains(&c)
                || matches!(c, 0xad | 0x200b..=0x200f | 0x202a..=0x202e | 0x2060..=0x2064 | 0xfeff)
        }
        None => false,
    }
}

/// Unicode `Sc` (currency symbols) and `Pc` (connector punctuation) in the BMP, as JDK 17's tables have them: Java
/// identifiers may contain both.
fn is_currency_or_connector(c: u16) -> bool {
    matches!(c,
        0x24 | 0x5F | 0xA2..=0xA5 | 0x58F | 0x60B | 0x7FE..=0x7FF | 0x9F2..=0x9F3 | 0x9FB | 0xAF1 | 0xBF9 | 0xE3F
        | 0x17DB | 0x203F..=0x2040 | 0x2054 | 0x20A0..=0x20BF | 0xA838 | 0xFDFC | 0xFE33..=0xFE34 | 0xFE4D..=0xFE4F
        | 0xFE69 | 0xFF04 | 0xFF3F | 0xFFE0..=0xFFE1 | 0xFFE5..=0xFFE6)
}

/// `Character.isJavaIdentifierStart`: letters, letter numbers, currency symbols and connector punctuation.
pub fn is_java_identifier_start(c: i32) -> bool {
    match char::from_u32(c as u32) {
        Some(ch) => ch.is_alphabetic() || (c <= 0xffff && is_currency_or_connector(c as u16)),
        None => false,
    }
}

impl Parser {
    fn new(buf: Vec<u16>, utf8: bool, features: ReadFeatures) -> Parser {
        Parser {
            buf,
            utf8,
            hit_end: std::cell::Cell::new(false),
            last_token: "null",
            ptr: 0,
            row: 1,
            row_start: 0,
            ctx: vec![Ctx::root()],
            features,
            token_row: 1,
            token_col: 0,
            pending: None,
        }
    }

    fn read(&mut self) -> Result<JsonNode, JacksonError> {
        match self.next_token()? {
            None => Ok(JsonNode::Missing),
            Some(t) => {
                self.last_token = t.name();
                self.build(t)
            }
        }
    }

    fn loc(&self, row: usize, col: usize) -> String {
        format!("{SOURCE}line: {row}, column: {col}]")
    }

    fn current_location(&self) -> String {
        self.loc(self.row, self.ptr - self.row_start + 1)
    }

    fn location_minus_one(&self) -> String {
        self.loc(self.row, self.ptr - 1 - self.row_start + 1)
    }

    fn err_at(&self, class: &str, msg: String, loc: String) -> JacksonError {
        JacksonError { class: class.to_string(), message: format!("{msg}\n at {loc}") }
    }

    fn parse_error(&self, msg: String) -> JacksonError {
        self.err_at("com.fasterxml.jackson.core.JsonParseException", msg, self.current_location())
    }

    fn parse_error_minus_one(&self, msg: String) -> JacksonError {
        self.err_at("com.fasterxml.jackson.core.JsonParseException", msg, self.location_minus_one())
    }

    fn eof_error(&self, msg: &str) -> JacksonError {
        self.err_at("com.fasterxml.jackson.core.io.JsonEOFException", format!("Unexpected end-of-input{msg}"), self.current_location())
    }

    fn unexpected_char(&self, ch: i32, comment: &str) -> JacksonError {
        if ch < 0 {
            return self.eof_error(" in null");
        }
        self.parse_error_minus_one(format!("Unexpected character ({}): {comment}", char_desc(ch)))
    }

    fn unexpected_number_char(&self, ch: i32, comment: &str) -> JacksonError {
        self.parse_error_minus_one(format!("Unexpected character ({}) in numeric value: {comment}", char_desc(ch)))
    }

    fn invalid_space(&self, i: i32) -> JacksonError {
        let mut msg = format!("Illegal character ({}): only regular white space (\\r, \\n, \\t) is allowed between tokens", char_desc(i));
        if i == 30 {
            msg.push_str(" (consider enabling `JsonReadFeature.ALLOW_RS_CONTROL_CHAR` to allow use of Record Separators (\\u001E))");
        }
        self.parse_error(msg)
    }

    fn at_end(&self) -> bool {
        let end = self.ptr >= self.buf.len();
        if end {
            self.hit_end.set(true);
        }
        end
    }

    fn top(&mut self) -> &mut Ctx {
        self.ctx.last_mut().unwrap()
    }

    fn handle_eof(&self) -> Result<(), JacksonError> {
        let c = self.ctx.last().unwrap();
        if c.kind != CtxType::Root {
            let marker = if c.kind == CtxType::Array { "Array" } else { "Object" };
            let start = self.loc(c.start_row, c.start_col);
            return Err(self.eof_error(&format!(": expected close marker for {marker} (start marker at {start})")));
        }
        Ok(())
    }

    fn skip_cr(&mut self) {
        if !self.at_end() && self.buf[self.ptr] == b'\n' as u16 {
            self.ptr += 1;
        }
        self.row += 1;
        self.row_start = self.ptr;
    }

    /// `_skipComment` (after `/`) and `_skipYAMLComment` (after `#`): `true` when `i` opened a comment, now skipped.
    fn skip_comment_if(&mut self, i: i32) -> Result<bool, JacksonError> {
        if i == 47 {
            self.skip_comment()?;
            return Ok(true);
        }
        if i == 35 && self.features.allow_yaml_comments {
            self.skip_line()?;
            return Ok(true);
        }
        Ok(false)
    }

    fn skip_comment(&mut self) -> Result<(), JacksonError> {
        if !self.features.allow_comments {
            return Err(self.unexpected_char(47, "maybe a (non-standard) comment? (not recognized as one since Feature 'ALLOW_COMMENTS' not enabled for parser)"));
        }
        if self.at_end() {
            return Err(self.eof_error(" in a comment"));
        }
        let c = self.buf[self.ptr] as i32;
        self.ptr += 1;
        match c {
            47 => self.skip_line(),
            42 => self.skip_c_comment(),
            _ => Err(self.unexpected_char(c, "was expecting either '*' or '/' for a comment")),
        }
    }

    fn skip_c_comment(&mut self) -> Result<(), JacksonError> {
        while !self.at_end() {
            let i = self.buf[self.ptr] as i32;
            self.ptr += 1;
            if i == 42 {
                if self.at_end() {
                    break;
                }
                if self.buf[self.ptr] == 47 {
                    self.ptr += 1;
                    return Ok(());
                }
            } else if i < 32 {
                self.line_space(i)?;
            }
        }
        Err(self.eof_error(" in a comment"))
    }

    fn skip_line(&mut self) -> Result<(), JacksonError> {
        while !self.at_end() {
            let i = self.buf[self.ptr] as i32;
            self.ptr += 1;
            if i < 32 {
                match i {
                    10 => {
                        self.row += 1;
                        self.row_start = self.ptr;
                        return Ok(());
                    }
                    13 => {
                        self.skip_cr();
                        return Ok(());
                    }
                    9 => {}
                    _ => return Err(self.invalid_space(i)),
                }
            }
        }
        Ok(())
    }

    /// A control character between tokens: a line break, a tab, or an error.
    fn line_space(&mut self, i: i32) -> Result<(), JacksonError> {
        match i {
            10 => {
                self.row += 1;
                self.row_start = self.ptr;
            }
            13 => self.skip_cr(),
            9 => {}
            _ => return Err(self.invalid_space(i)),
        }
        Ok(())
    }

    /// `_skipWSOrEnd`: next significant char, or -1 at EOF (after checking open scopes).
    fn skip_ws_or_end(&mut self) -> Result<i32, JacksonError> {
        while !self.at_end() {
            let i = self.buf[self.ptr] as i32;
            self.ptr += 1;
            if i > 32 {
                if self.skip_comment_if(i)? {
                    continue;
                }
                return Ok(i);
            }
            if i != 32 {
                if i == 10 {
                    self.row += 1;
                    self.row_start = self.ptr;
                } else if i == 13 {
                    self.skip_cr();
                } else if i != 9 {
                    return Err(self.invalid_space(i));
                }
            }
        }
        self.handle_eof()?;
        Ok(-1)
    }

    fn skip_comma(&mut self, i: i32) -> Result<i32, JacksonError> {
        if i != 44 {
            let desc = self.ctx.last().unwrap().type_desc();
            return Err(self.unexpected_char(i, &format!("was expecting comma to separate {desc} entries")));
        }
        while !self.at_end() {
            let i = self.buf[self.ptr] as i32;
            self.ptr += 1;
            if i > 32 {
                if self.skip_comment_if(i)? {
                    continue;
                }
                return Ok(i);
            }
            if i < 32 {
                if i == 10 {
                    self.row += 1;
                    self.row_start = self.ptr;
                } else if i == 13 {
                    self.skip_cr();
                } else if i != 9 {
                    return Err(self.invalid_space(i));
                }
            }
        }
        let desc = self.ctx.last().unwrap().type_desc();
        Err(self.parse_error(format!("Unexpected end-of-input within/between {desc} entries")))
    }

    fn skip_colon(&mut self) -> Result<i32, JacksonError> {
        let mut got_colon = false;
        while !self.at_end() {
            let i = self.buf[self.ptr] as i32;
            self.ptr += 1;
            if i > 32 {
                if self.skip_comment_if(i)? {
                    continue;
                }
                if got_colon {
                    return Ok(i);
                }
                if i != 58 {
                    return Err(self.unexpected_char(i, "was expecting a colon to separate field name and value"));
                }
                got_colon = true;
            } else if i < 32 {
                if i == 10 {
                    self.row += 1;
                    self.row_start = self.ptr;
                } else if i == 13 {
                    self.skip_cr();
                } else if i != 9 {
                    return Err(self.invalid_space(i));
                }
            }
        }
        let desc = self.ctx.last().unwrap().type_desc();
        Err(self.eof_error(&format!(" within/between {desc} entries")))
    }

    fn push_ctx(&mut self, kind: CtxType) -> Result<(), JacksonError> {
        if self.ctx.len() > MAX_NESTING_DEPTH {
            return Err(JacksonError {
                class: "com.fasterxml.jackson.core.exc.StreamConstraintsException".into(),
                message: format!(
                    "Document nesting depth ({}) exceeds the maximum allowed ({MAX_NESTING_DEPTH}, from `StreamReadConstraints.getMaxNestingDepth()`)",
                    self.ctx.len()
                ),
            });
        }
        let names = if kind == CtxType::Object && self.features.strict_duplicate_detection { Some(Default::default()) } else { None };
        self.ctx.push(Ctx { kind, index: -1, start_row: self.token_row, start_col: self.token_col, names });
        Ok(())
    }

    fn close_scope(&mut self, i: i32) -> Result<Token, JacksonError> {
        let c = self.ctx.last().unwrap();
        let (want, exp) = if i == 93 { (CtxType::Array, '}') } else { (CtxType::Object, ']') };
        if c.kind != want {
            if c.kind == CtxType::Root {
                let scope = if i == 125 { "Object" } else { "Array" };
                return Err(self.parse_error_minus_one(format!("Unexpected close marker '{}': no open {scope} to close", i as u8 as char)));
            }
            let msg = format!(
                "Unexpected close marker '{}': expected '{exp}' (for {} starting at {})",
                i as u8 as char,
                c.type_desc(),
                self.loc(c.start_row, c.start_col)
            );
            return Err(self.parse_error_minus_one(msg));
        }
        self.ctx.pop();
        Ok(if i == 93 { Token::EndArray } else { Token::EndObject })
    }

    fn next_token(&mut self) -> Result<Option<Token>, JacksonError> {
        let mut i = self.skip_ws_or_end()?;
        if i < 0 {
            return Ok(None);
        }
        if (i | 32) == 125 {
            return self.close_scope(i).map(Some);
        }
        if self.top().expect_comma() {
            i = self.skip_comma(i)?;
            if self.features.allow_trailing_comma && (i | 32) == 125 {
                return self.close_scope(i).map(Some);
            }
        }
        let in_object = self.ctx.last().unwrap().kind == CtxType::Object;
        if in_object {
            let name = if i == 34 && self.utf8 {
                self.parse_name_utf8()?
            } else if i == 34 {
                self.parse_string(" in field name", "name")?
            } else {
                self.odd_name(i)?
            };
            if let Some(names) = &mut self.ctx.last_mut().unwrap().names {
                if !names.insert(name.clone()) {
                    return Err(self.parse_error(format!("Duplicate field '{name}'")));
                }
            }
            let after = self.skip_colon()?;
            self.pending = Some(after);
            return Ok(Some(Token::Field(name)));
        }
        self.value_token(i).map(Some)
    }

    /// `_handleOddName`: an apostrophe-quoted or unquoted field name, when the features allow them.
    fn odd_name(&mut self, i: i32) -> Result<String, JacksonError> {
        if i == 39 && self.features.allow_single_quotes {
            let mut out: Vec<u16> = Vec::new();
            loop {
                if self.at_end() {
                    return Err(self.eof_error(" in field name"));
                }
                let c = self.buf[self.ptr];
                self.ptr += 1;
                match c {
                    39 => break,
                    92 => out.push(self.decode_escaped()?),
                    _ if c < 32 => {
                        self.unquoted_control_char(c, "name")?;
                        out.push(c);
                    }
                    _ => out.push(c),
                }
            }
            return Ok(String::from_utf16_lossy(&out));
        }
        if !self.features.allow_unquoted_field_names {
            // UTF8StreamJsonParser reports the char the bytes decode to, cut to a `char`
            let i = if self.utf8 { self.decode_char_for_error(i as u16)? as u16 as i32 } else { i };
            return Err(self.unexpected_char(i, "was expecting double-quote to start field name"));
        }
        // `CharTypes.getInputCodeLatin1JsNames`: below 256, Java identifier parts from `!` up and `@`, `#`, `*`, `-`, `+`;
        // past it, Java identifier parts.
        let name_char = |c: u16| if c < 256 { (c >= 33 && is_java_identifier_part(c)) || matches!(c, 0x40 | 0x23 | 0x2a | 0x2d | 0x2b) } else { is_java_identifier_part(c) };
        if !name_char(i as u16) {
            return Err(self.unexpected_char(i, "was expecting either valid name character (for unquoted name) or double-quote (for quoted) to start field name"));
        }
        let start = self.ptr - 1;
        while !self.at_end() && name_char(self.buf[self.ptr]) {
            self.ptr += 1;
        }
        Ok(String::from_utf16_lossy(&self.buf[start..self.ptr]))
    }

    fn value_token(&mut self, i: i32) -> Result<Token, JacksonError> {
        self.token_row = self.row;
        self.token_col = self.ptr - self.row_start;
        Ok(match i {
            34 if self.utf8 => Token::Str(self.parse_string_utf8()?),
            34 => Token::Str(self.parse_string(": was expecting closing quote for a string value", "string value")?),
            45 => self.parse_signed_number(true)?,
            43 => {
                if self.at_end() {
                    return Err(self.eof_error(" in a Number value"));
                }
                let c = self.buf[self.ptr] as i32;
                self.ptr += 1;
                return self.invalid_number_start(c, false, true);
            }
            48..=57 => self.parse_unsigned_number(i)?,
            91 => {
                self.push_ctx(CtxType::Array)?;
                Token::StartArray
            }
            123 => {
                self.push_ctx(CtxType::Object)?;
                Token::StartObject
            }
            102 => {
                self.match_token("false", 1)?;
                Token::False
            }
            110 => {
                self.match_token("null", 1)?;
                Token::Null
            }
            116 => {
                self.match_token("true", 1)?;
                Token::True
            }
            125 if self.features.tree_member_values && self.ctx.last().is_some_and(|c| c.kind == CtxType::Object) => return self.handle_odd_value(i),
            125 => return Err(self.unexpected_char(i, "expected a value")),
            _ => return self.handle_odd_value(i),
        })
    }

    fn valid_values(&self) -> &'static str {
        if self.features.allow_non_numeric_numbers { VALID_VALUES_NON_NUMERIC } else { VALID_VALUES }
    }

    fn handle_odd_value(&mut self, i: i32) -> Result<Token, JacksonError> {
        // UTF8StreamJsonParser._handleUnexpectedValue: `}`, `,` and (in an array) `]` where a value should be
        if self.utf8
            && !self.features.allow_missing_values
            && (i == 125 || i == 44 || (i == 93 && self.ctx.last().is_some_and(|c| c.kind == CtxType::Array)))
        {
            return Err(self.unexpected_char(i, "expected a value"));
        }
        match i {
            39 if self.features.allow_single_quotes => return self.parse_apos_string().map(Token::Str),
            // `]` is a missing element only inside an array; `,` anywhere but the root.
            93 | 44 if self.features.allow_missing_values
                && !self.in_root()
                && (i == 44 || self.ctx.last().is_some_and(|c| c.kind == CtxType::Array)) =>
            {
                self.ptr -= 1;
                return Ok(Token::Null);
            }
            73 => {
                self.match_token("Infinity", 1)?;
                if self.features.allow_non_numeric_numbers {
                    return Ok(Token::Float("Infinity".into()));
                }
                return Err(self.parse_error("Non-standard token 'Infinity': enable `JsonReadFeature.ALLOW_NON_NUMERIC_NUMBERS` to allow".into()));
            }
            78 => {
                self.match_token("NaN", 1)?;
                if self.features.allow_non_numeric_numbers {
                    return Ok(Token::Float("NaN".into()));
                }
                return Err(self.parse_error("Non-standard token 'NaN': enable `JsonReadFeature.ALLOW_NON_NUMERIC_NUMBERS` to allow".into()));
            }
            _ => {}
        }
        // [core#77] (2.22.3): a multi-byte UTF-8 character is decoded first, its continuation bytes consumed
        let i = if self.utf8 && i > 0x7F { self.decode_char_for_error(i as u16)? as i32 } else { i };
        if is_java_identifier_start(i) {
            let s = String::from_utf16_lossy(&[i as u16]);
            return Err(self.invalid_token(&s));
        }
        let valid = self.valid_values();
        Err(self.unexpected_char(i, &format!("expected a valid value {valid}")))
    }

    fn invalid_token(&mut self, matched: &str) -> JacksonError {
        let token_start = self.ptr - matched.encode_utf16().count();
        let col = token_start - self.row_start + 1;
        let loc = self.loc(self.row, col);
        let mut sb: Vec<u16> = matched.encode_utf16().collect();
        if self.utf8 {
            // the token's bytes, decoded one char at a time (`_decodeCharForError`), each cut to a `char`
            while !self.at_end() {
                let i = self.buf[self.ptr];
                self.ptr += 1;
                let c = match self.decode_char_for_error(i) {
                    Ok(c) => c as u16,
                    Err(e) => return e,
                };
                if !is_java_identifier_part(c) {
                    break;
                }
                sb.push(c);
                if sb.len() >= MAX_ERROR_TOKEN_LENGTH {
                    sb.extend("...".encode_utf16());
                    break;
                }
            }
            let s = String::from_utf16_lossy(&sb);
            let valid = self.valid_values();
            return self.err_at("com.fasterxml.jackson.core.JsonParseException", format!("Unrecognized token '{s}': was expecting {valid}"), loc);
        }
        while !self.at_end() {
            let c = self.buf[self.ptr];
            if !is_java_identifier_part(c) {
                break;
            }
            self.ptr += 1;
            sb.push(c);
            if sb.len() >= MAX_ERROR_TOKEN_LENGTH {
                sb.extend("...".encode_utf16());
                break;
            }
        }
        let s = String::from_utf16_lossy(&sb);
        let valid = self.valid_values();
        self.err_at("com.fasterxml.jackson.core.JsonParseException", format!("Unrecognized token '{s}': was expecting {valid}"), loc)
    }

    fn match_token(&mut self, m: &str, mut i: usize) -> Result<(), JacksonError> {
        let mc: Vec<u16> = m.encode_utf16().collect();
        while i < mc.len() {
            if self.at_end() || self.buf[self.ptr] != mc[i] {
                let part = String::from_utf16_lossy(&mc[..i]);
                return Err(self.invalid_token(&part));
            }
            self.ptr += 1;
            i += 1;
        }
        if self.utf8 && !self.at_end() && self.buf[self.ptr] >= 0x80 {
            // `_checkMatchEnd` (2.22.3): a multi-byte character after the token is decoded (consumed), and ends the
            // token's report when it is an identifier part; otherwise the parser rewinds to it
            let ptr = self.ptr;
            self.ptr += 1;
            let c = self.decode_char_for_error(self.buf[ptr])? as u16;
            if is_java_identifier_part(c) {
                let mut part = mc[..i].to_vec();
                part.push(c);
                return Err(self.invalid_token(&String::from_utf16_lossy(&part)));
            }
            self.ptr = ptr;
            return Ok(());
        }
        if !self.at_end() {
            let ch = self.buf[self.ptr];
            if ch >= 48 && ch != 93 && ch != 125 && is_java_identifier_part(ch) {
                let part = String::from_utf16_lossy(&mc[..i]);
                return Err(self.invalid_token(&part));
            }
        }
        Ok(())
    }

    fn parse_string(&mut self, eof_msg: &str, ctxt_desc: &str) -> Result<String, JacksonError> {
        let mut out: Vec<u16> = Vec::new();
        loop {
            if self.at_end() {
                return Err(self.eof_error(eof_msg));
            }
            let c = self.buf[self.ptr];
            self.ptr += 1;
            if c == 34 {
                break;
            }
            if c == 92 {
                let d = self.decode_escaped()?;
                out.push(d);
                continue;
            }
            if c < 32 {
                self.unquoted_control_char(c, ctxt_desc)?;
            }
            out.push(c);
        }
        Ok(String::from_utf16_lossy(&out))
    }

    /// `_handleApos`: a string value in apostrophes.
    fn parse_apos_string(&mut self) -> Result<String, JacksonError> {
        let mut out: Vec<u16> = Vec::new();
        loop {
            if self.at_end() {
                return Err(self.eof_error(": was expecting closing quote for a string value"));
            }
            let c = self.buf[self.ptr];
            self.ptr += 1;
            match c {
                39 => return Ok(String::from_utf16_lossy(&out)),
                92 => out.push(self.decode_escaped()?),
                _ if c < 32 => {
                    self.unquoted_control_char(c, "string value")?;
                    out.push(c);
                }
                _ => out.push(c),
            }
        }
    }

    /// `_throwUnquotedSpace`: a control character inside a string or name, fine under `ALLOW_UNQUOTED_CONTROL_CHARS`.
    fn unquoted_control_char(&self, c: u16, ctxt_desc: &str) -> Result<(), JacksonError> {
        if self.features.allow_unquoted_control_chars {
            return Ok(());
        }
        Err(self.parse_error_minus_one(format!(
            "Illegal unquoted character ({}): has to be escaped using backslash to be included in {ctxt_desc}",
            char_desc(c as i32)
        )))
    }

    fn decode_escaped(&mut self) -> Result<u16, JacksonError> {
        if self.at_end() {
            return Err(self.eof_error(" in character escape sequence"));
        }
        let c = self.buf[self.ptr];
        self.ptr += 1;
        Ok(match c {
            34 | 47 | 92 => c,
            98 => 8,
            102 => 12,
            110 => 10,
            114 => 13,
            116 => 9,
            117 => {
                let mut value: u16 = 0;
                for _ in 0..4 {
                    if self.at_end() {
                        return Err(self.eof_error(" in character escape sequence"));
                    }
                    let ch = self.buf[self.ptr];
                    self.ptr += 1;
                    let digit = match char::from_u32(ch as u32).and_then(|c| c.to_digit(16)) {
                        Some(d) if ch < 128 => d as u16,
                        _ => return Err(self.unexpected_char(ch as i32, "expected a hex-digit for character escape sequence")),
                    };
                    value = (value << 4) | digit;
                }
                value
            }
            // `_handleUnrecognizedCharacterEscape`, of the char the bytes decode to
            _ if self.utf8 && c >= 0x80 => {
                let d = self.decode_char_for_error(c)? as u16;
                if self.features.allow_backslash_escaping_any_character {
                    return Ok(d);
                }
                return Err(self.parse_error_minus_one(format!("Unrecognized character escape {}", char_desc(d as i32))));
            }
            _ if self.features.allow_backslash_escaping_any_character => c,
            39 if self.features.allow_single_quotes => c,
            _ => return Err(self.parse_error_minus_one(format!("Unrecognized character escape {}", char_desc(c as i32)))),
        })
    }

    fn peek(&self) -> Option<u16> {
        if self.at_end() { None } else { Some(self.buf[self.ptr]) }
    }

    fn verify_root_space(&mut self, ch: i32) -> Result<(), JacksonError> {
        self.ptr += 1;
        match ch {
            9 | 32 => Ok(()),
            10 => {
                self.row += 1;
                self.row_start = self.ptr;
                Ok(())
            }
            13 => {
                self.ptr -= 1;
                Ok(())
            }
            _ => Err(self.unexpected_char(ch, "Expected space separating root-level values")),
        }
    }

    fn in_root(&self) -> bool {
        self.ctx.last().unwrap().kind == CtxType::Root
    }

    fn parse_unsigned_number(&mut self, ch: i32) -> Result<Token, JacksonError> {
        let start = self.ptr - 1;
        self.parse_number(false, start, ch)
    }

    fn parse_signed_number(&mut self, negative: bool) -> Result<Token, JacksonError> {
        let start = self.ptr - 1;
        match self.peek() {
            // `_parseNumber2`'s `getNextChar`; the byte parser's `_loadMoreGuaranteed` names the current token
            None => Err(if self.utf8 { self.eof_in_token() } else { self.eof_error("No digit following sign") }),
            Some(c) if (48..=57).contains(&c) => {
                self.ptr += 1;
                self.parse_number(negative, start, c as i32)
            }
            Some(c) => {
                self.ptr += 1;
                if c == 46 {
                    return self.handle_odd_value(46);
                }
                self.invalid_number_start(c as i32, negative, true)
            }
        }
    }

    fn invalid_number_start(&mut self, ch: i32, negative: bool, has_sign: bool) -> Result<Token, JacksonError> {
        if ch == 73 {
            if self.at_end() {
                return Err(self.eof_error(" in a Number value"));
            }
            let c = self.buf[self.ptr];
            self.ptr += 1;
            let m = if c == 78 {
                Some(if negative { "-INF" } else { "+INF" })
            } else if c == 110 {
                Some(if negative { "-Infinity" } else { "+Infinity" })
            } else {
                None
            };
            if let Some(m) = m {
                self.match_token(m, 3)?;
                if self.features.allow_non_numeric_numbers {
                    return Ok(Token::Float(if negative { "-Infinity" } else { "Infinity" }.into()));
                }
                return Err(self.parse_error(format!("Non-standard token '{m}': enable `JsonReadFeature.ALLOW_NON_NUMERIC_NUMBERS` to allow")));
            }
        }
        if has_sign && !negative {
            return Err(self.unexpected_number_char(43, "JSON spec does not allow numbers to have plus signs: enable `JsonReadFeature.ALLOW_LEADING_PLUS_SIGN_FOR_NUMBERS` to allow"));
        }
        let msg = if negative { "expected digit (0-9) to follow minus sign, for valid numeric value" } else { "expected digit (0-9) for valid numeric value" };
        Err(self.unexpected_number_char(ch, msg))
    }

    /// Number with the first digit (`first`, at `ptr-1`) already consumed; `start` is where the text begins.
    fn parse_number(&mut self, neg: bool, start: usize, first: i32) -> Result<Token, JacksonError> {
        let mut c = first;
        let mut int_len = 0usize;
        // `_verifyNoLeadingZeroes`: allowed, the zeros are dropped from the number's text.
        let mut zeros = 0usize;
        if c == 48 {
            if let Some(n) = self.peek() {
                if (48..=57).contains(&n) {
                    if !self.features.allow_numeric_leading_zeros {
                        return Err(self.parse_error("Invalid numeric value: Leading zeroes not allowed".into()));
                    }
                    while self.peek() == Some(48) {
                        self.ptr += 1;
                        zeros += 1;
                    }
                    // Every zero goes when a significant digit follows; otherwise the last one stays.
                    if self.peek().is_some_and(|d| (49..=57).contains(&d)) {
                        zeros += 1;
                        c = self.buf[self.ptr] as i32;
                        self.ptr += 1;
                    }
                }
            }
        }
        let mut eof = false;
        loop {
            if !(48..=57).contains(&c) {
                break;
            }
            int_len += 1;
            match self.peek() {
                None => {
                    eof = true;
                    c = 0;
                    break;
                }
                Some(n) => {
                    self.ptr += 1;
                    c = n as i32;
                }
            }
        }
        // Jackson's fast path (numbers not starting with '0') keeps its scan pointer local, so errors in the
        // fraction/exponent are located at the first '.'/'e' after the integer part.
        let fast_mark = if !self.utf8 && first != 48 && !eof && (c == 46 || (c | 32) == 101) { Some(self.ptr) } else { None };
        let num_err = |p: &mut Parser, ch: i32, msg: &str| -> JacksonError {
            if let Some(m) = fast_mark {
                let saved = p.ptr;
                p.ptr = m;
                let e = p.unexpected_number_char(ch, msg);
                p.ptr = saved;
                e
            } else {
                p.unexpected_number_char(ch, msg)
            }
        };
        let mut fract_len: i32 = -1;
        if !eof && c == 46 {
            fract_len = 0;
            loop {
                match self.peek() {
                    None => {
                        eof = true;
                        break;
                    }
                    Some(n) => {
                        self.ptr += 1;
                        c = n as i32;
                        if !(48..=57).contains(&c) {
                            break;
                        }
                        fract_len += 1;
                    }
                }
            }
            if fract_len == 0 {
                // a number that reaches the end is parsed by `_parseNumber2`, which reports the '.' where it stands
                if eof {
                    return Err(self.unexpected_number_char(46, "Decimal point not followed by a digit"));
                }
                return Err(num_err(self, c, "Decimal point not followed by a digit"));
            }
        }
        let mut exp_len: i32 = -1;
        if !eof && (c | 32) == 101 {
            exp_len = 0;
            let next = |p: &mut Parser| -> Result<i32, JacksonError> {
                match p.peek() {
                    None => Err(if p.utf8 { p.eof_in_token() } else { p.eof_error("expected a digit for number exponent") }),
                    Some(n) => {
                        p.ptr += 1;
                        Ok(n as i32)
                    }
                }
            };
            c = next(self)?;
            if c == 45 || c == 43 {
                c = next(self)?;
            }
            while (48..=57).contains(&c) {
                exp_len += 1;
                match self.peek() {
                    None => {
                        eof = true;
                        break;
                    }
                    Some(n) => {
                        self.ptr += 1;
                        c = n as i32;
                    }
                }
            }
            if exp_len == 0 {
                return Err(num_err(self, c, "Exponent indicator not followed by a digit"));
            }
        }
        let end;
        if !eof {
            self.ptr -= 1;
            end = self.ptr;
            if self.in_root() {
                self.verify_root_space(c)?;
            }
        } else {
            end = self.ptr;
        }
        let mut text = String::from_utf16_lossy(&self.buf[start..end]);
        if zeros > 0 {
            let digits = if neg { start + 1 } else { start };
            let sign = if neg { "-" } else { "" };
            text = format!("{sign}{}", String::from_utf16_lossy(&self.buf[digits + zeros..end]));
        }
        if fract_len < 0 && exp_len < 0 {
            Ok(Token::Int { text, int_len, neg })
        } else {
            Ok(Token::Float(text))
        }
    }

    /// `JsonNodeDeserializer`: builds the tree for the value starting with `t` (iteratively, so deep documents
    /// hit the nesting-depth limit rather than the native stack).
    fn build(&mut self, t: Token) -> Result<JsonNode, JacksonError> {
        enum Open {
            Obj(ObjectNode, Option<String>),
            Arr(Vec<JsonNode>),
        }
        let mut stack: Vec<Open> = Vec::new();
        let mut tok = t;
        loop {
            // Turn `tok` into either a completed value or a newly opened container.
            let mut value = match tok {
                Token::StartObject => {
                    stack.push(Open::Obj(ObjectNode::new(), None));
                    None
                }
                Token::StartArray => {
                    stack.push(Open::Arr(Vec::new()));
                    None
                }
                Token::EndObject => match stack.pop() {
                    Some(Open::Obj(m, _)) => Some(JsonNode::Object(Arc::new(m))),
                    _ => unreachable!(),
                },
                Token::EndArray => match stack.pop() {
                    Some(Open::Arr(a)) => Some(JsonNode::Array(Arc::new(a))),
                    _ => unreachable!(),
                },
                Token::Field(name) => {
                    if let Some(Open::Obj(_, n)) = stack.last_mut() {
                        *n = Some(name);
                    }
                    let first = self.pending.take().unwrap();
                    self.last_token = "FIELD_NAME";
                    tok = self.value_token(first)?;
                    self.last_token = tok.name();
                    continue;
                }
                Token::Str(s) => Some(JsonNode::Text(s.into())),
                Token::True => Some(JsonNode::Boolean(true)),
                Token::False => Some(JsonNode::Boolean(false)),
                Token::Null => Some(JsonNode::Null),
                Token::Int { text, int_len, neg } => Some(int_node(&text, int_len, neg)),
                Token::Float(text) => Some(if self.features.use_big_decimal_for_floats && !text.ends_with(['N', 'y']) {
                    JsonNode::Decimal(Arc::new(crate::BigDecimal::parse(&text).expect("valid JSON number")))
                } else {
                    JsonNode::Double(text.parse::<f64>().unwrap())
                }),
            };
            if let Some(v) = value.take() {
                match stack.last_mut() {
                    None => return Ok(v),
                    Some(Open::Arr(a)) => a.push(v),
                    Some(Open::Obj(m, n)) => {
                        // duplicate keys (non-strict): last value wins, first position kept
                        m.insert(n.take().unwrap(), v);
                    }
                }
            }
            tok = match self.next_token()? {
                Some(t) => {
                    self.last_token = t.name();
                    t
                }
                None => unreachable!("EOF inside a scope is reported by the tokenizer"),
            };
        }
    }

    fn utf8_error(&self, msg: String) -> JacksonError {
        self.parse_error(msg)
    }

    /// `_reportInvalidInitial`
    fn invalid_initial(&self, mask: u32) -> JacksonError {
        self.utf8_error(format!("Invalid UTF-8 start byte 0x{mask:x}"))
    }

    /// `_reportInvalidOther`
    fn invalid_other(&self, mask: u32) -> JacksonError {
        self.utf8_error(format!("Invalid UTF-8 middle byte 0x{mask:x}"))
    }

    /// `_reportInvalidEOF()`: end of input inside a multi-byte char, named after the current token.
    fn eof_in_token(&self) -> JacksonError {
        self.eof_error(&format!(" in {}", self.last_token))
    }

    /// `nextByte()`
    fn next_byte(&mut self) -> Result<u32, JacksonError> {
        if self.at_end() {
            return Err(self.eof_in_token());
        }
        let b = self.buf[self.ptr] as u32;
        self.ptr += 1;
        Ok(b)
    }

    /// `_decodeCharForError(firstByte)`: the code point a lead byte (already consumed) and its continuation bytes
    /// spell.
    fn decode_char_for_error(&mut self, first: u16) -> Result<u32, JacksonError> {
        let mut c = first as u32 & 0xFF;
        if c > 0x7F {
            let needed = if c & 0xE0 == 0xC0 {
                c &= 0x1F;
                1
            } else if c & 0xF0 == 0xE0 {
                c &= 0x0F;
                2
            } else if c & 0xF8 == 0xF0 {
                c &= 0x07;
                3
            } else {
                return Err(self.invalid_initial(c));
            };
            for _ in 0..needed {
                let d = self.next_byte()?;
                if d & 0xC0 != 0x80 {
                    return Err(self.invalid_other(d));
                }
                c = (c << 6) | (d & 0x3F);
            }
        }
        Ok(c)
    }

    /// One continuation byte inside a string value (`_decodeUtf8_2/3/4`).
    fn string_continuation(&mut self) -> Result<u32, JacksonError> {
        if self.at_end() {
            return Err(self.eof_in_token());
        }
        let d = self.buf[self.ptr] as u32;
        self.ptr += 1;
        if d & 0xC0 != 0x80 {
            return Err(self.invalid_other(d));
        }
        Ok(d & 0x3F)
    }

    /// `UTF8StreamJsonParser._finishString2`: a string value's bytes up to the closing quote, UTF-8 decoded.
    fn parse_string_utf8(&mut self) -> Result<String, JacksonError> {
        self.last_token = "VALUE_STRING";
        let mut out: Vec<u16> = Vec::new();
        loop {
            if self.at_end() {
                return Err(self.eof_in_token());
            }
            let c = self.buf[self.ptr];
            self.ptr += 1;
            match c {
                34 => break,
                92 => out.push(self.decode_escaped()?),
                _ if c < 32 => {
                    self.unquoted_control_char(c, "string value")?;
                    out.push(c);
                }
                _ if c < 0x80 => out.push(c),
                _ if c & 0xE0 == 0xC0 => {
                    let d = self.string_continuation()?;
                    out.push((((c as u32 & 0x1F) << 6) | d) as u16);
                }
                _ if c & 0xF0 == 0xE0 => {
                    let d = self.string_continuation()?;
                    let e = self.string_continuation()?;
                    let ch = ((c as u32 & 0x0F) << 12) | (d << 6) | e;
                    if (0xD800..=0xDFFF).contains(&ch) {
                        return Err(self.utf8_error(format!("Invalid UTF-8: Illegal surrogate character 0x{ch:x}")));
                    }
                    out.push(ch as u16);
                }
                _ if c & 0xF8 == 0xF0 => {
                    let d = self.string_continuation()?;
                    let e = self.string_continuation()?;
                    let f = self.string_continuation()?;
                    let ch = (((c as u32 & 0x07) << 18) | (d << 12) | (e << 6) | f).wrapping_sub(0x10000);
                    out.push((0xD800 | (ch >> 10)) as u16);
                    out.push((0xDC00 | (ch & 0x3FF)) as u16);
                }
                _ => return Err(self.invalid_initial(c as u32)),
            }
        }
        Ok(String::from_utf16_lossy(&out))
    }

    /// `UTF8StreamJsonParser._parseName` and `parseEscapedName`: the name's bytes up to the closing quote (escapes
    /// re-encoded as UTF-8), then decoded as `addName` decodes them, so its errors are located after the quote.
    fn parse_name_utf8(&mut self) -> Result<String, JacksonError> {
        let mut bytes: Vec<u8> = Vec::new();
        if self.at_end() {
            return Err(self.eof_error(": was expecting closing '\"' for name"));
        }
        loop {
            if self.at_end() {
                return Err(self.eof_error(" in field name"));
            }
            let c = self.buf[self.ptr];
            self.ptr += 1;
            match c {
                34 => break,
                92 => {
                    let mut ch = self.decode_escaped()? as u32;
                    if (0xD800..=0xDBFF).contains(&ch) {
                        if self.at_end() {
                            return Err(self.eof_error(" in field name"));
                        }
                        let next = self.buf[self.ptr];
                        if next != 92 {
                            return Err(self.parse_error(format!(
                                "Broken surrogate pair in field name: expected '\\' to start low surrogate, got 0x{next:x}"
                            )));
                        }
                        self.ptr += 1;
                        let lo = self.decode_escaped()? as u32;
                        if !(0xDC00..=0xDFFF).contains(&lo) {
                            return Err(self.parse_error(format!("Broken surrogate pair in field name: expected low surrogate, got 0x{lo:04X}")));
                        }
                        ch = 0x10000 + ((ch - 0xD800) << 10) + (lo - 0xDC00);
                    } else if (0xDC00..=0xDFFF).contains(&ch) {
                        return Err(self.parse_error(format!("Unexpected low surrogate in field name: 0x{ch:x}")));
                    }
                    match char::from_u32(ch) {
                        Some(ch) => bytes.extend_from_slice(ch.encode_utf8(&mut [0; 4]).as_bytes()),
                        None => unreachable!("surrogates are paired or rejected above"),
                    }
                }
                _ if c < 32 => {
                    self.unquoted_control_char(c, "name")?;
                    bytes.push(c as u8);
                }
                _ => bytes.push(c as u8),
            }
        }
        self.add_name(&bytes)
    }

    /// `addName(quads, qlen, lastQuadBytes)`: the name's bytes, packed four to a big-endian `int` quad (the last one
    /// left-aligned), decoded as UTF-8. A continuation byte that is not one is reported as the quad shifted down to
    /// it (`ch2 = quad >> shift`, not masked), as there.
    fn add_name(&self, bytes: &[u8]) -> Result<String, JacksonError> {
        let quad_byte = |ix: usize| -> i32 {
            let q = ix >> 2;
            let mut quad: u32 = 0;
            for k in 0..4 {
                quad = (quad << 8) | *bytes.get(q * 4 + k).unwrap_or(&0) as u32;
            }
            (quad as i32) >> ((3 - (ix & 3)) << 3)
        };
        let len = bytes.len();
        let mut out: Vec<u16> = Vec::new();
        let mut ix = 0;
        while ix < len {
            let mut ch = bytes[ix] as u32;
            ix += 1;
            if ch > 127 {
                let needed = if ch & 0xE0 == 0xC0 {
                    ch &= 0x1F;
                    1
                } else if ch & 0xF0 == 0xE0 {
                    ch &= 0x0F;
                    2
                } else if ch & 0xF8 == 0xF0 {
                    ch &= 0x07;
                    3
                } else {
                    return Err(self.invalid_initial(ch));
                };
                if ix + needed > len {
                    return Err(self.eof_error(" in field name"));
                }
                let ch2 = quad_byte(ix);
                ix += 1;
                if ch2 & 0xC0 != 0x80 {
                    return Err(self.invalid_other(ch2 as u32));
                }
                ch = (ch << 6) | (ch2 as u32 & 0x3F);
                if needed > 1 {
                    let ch2 = quad_byte(ix);
                    ix += 1;
                    if ch2 & 0xC0 != 0x80 {
                        return Err(self.invalid_other(ch2 as u32));
                    }
                    ch = (ch << 6) | (ch2 as u32 & 0x3F);
                    if needed == 2 {
                        if (0xD800..=0xDFFF).contains(&ch) {
                            return Err(self.utf8_error(format!("Invalid UTF-8: Illegal surrogate character 0x{ch:x}")));
                        }
                    } else {
                        let ch2 = quad_byte(ix);
                        ix += 1;
                        if ch2 & 0xC0 != 0x80 {
                            return Err(self.invalid_other(ch2 as u32 & 0xFF));
                        }
                        ch = (ch << 6) | (ch2 as u32 & 0x3F);
                    }
                }
                if needed > 2 {
                    let c = ch.wrapping_sub(0x10000);
                    out.push((0xD800 + (c >> 10)) as u16);
                    out.push((0xDC00 | (c & 0x3FF)) as u16);
                    continue;
                }
            }
            out.push(ch as u16);
        }
        Ok(String::from_utf16_lossy(&out))
    }
}

impl Token {
    /// The `JsonToken` constant's name.
    fn name(&self) -> &'static str {
        match self {
            Token::StartObject => "START_OBJECT",
            Token::EndObject => "END_OBJECT",
            Token::StartArray => "START_ARRAY",
            Token::EndArray => "END_ARRAY",
            Token::Field(_) => "FIELD_NAME",
            Token::Str(_) => "VALUE_STRING",
            Token::Int { .. } => "VALUE_NUMBER_INT",
            Token::Float(_) => "VALUE_NUMBER_FLOAT",
            Token::True => "VALUE_TRUE",
            Token::False => "VALUE_FALSE",
            Token::Null => "VALUE_NULL",
        }
    }
}

fn int_node(text: &str, int_len: usize, _neg: bool) -> JsonNode {
    if int_len <= 9 {
        return JsonNode::Int(text.parse().unwrap());
    }
    if int_len <= 18 {
        let l: i64 = text.parse().unwrap();
        if int_len == 10 && l >= i32::MIN as i64 && l <= i32::MAX as i64 {
            return JsonNode::Int(l as i32);
        }
        return JsonNode::Long(l);
    }
    match text.parse::<i64>() {
        Ok(l) => JsonNode::Long(l),
        Err(_) => JsonNode::BigInteger(Arc::new(text.parse::<BigInt>().unwrap())),
    }
}
