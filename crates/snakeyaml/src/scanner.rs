//! `ScannerImpl` and the token classes. Comment tokens are scanned but never emitted
//! (`LoaderOptions.processComments` is off in every consumer).

use crate::error::{Mark, Result, YamlError};
use crate::options::{LoaderOptions, ScalarStyle};
use crate::reader::StreamReader;
use std::collections::VecDeque;

/// `Token.ID`: the kind of a scanner token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenId {
    /// `AliasToken`: `*name`.
    Alias,
    /// `AnchorToken`: `&name`.
    Anchor,
    /// `BlockEndToken`: the end of a block collection (dedent).
    BlockEnd,
    /// `BlockEntryToken`: a `-` block sequence entry.
    BlockEntry,
    /// `BlockMappingStartToken`.
    BlockMappingStart,
    /// `BlockSequenceStartToken`.
    BlockSequenceStart,
    /// `DirectiveToken`: `%YAML` or `%TAG`.
    Directive,
    /// `DocumentEndToken`: `...`.
    DocumentEnd,
    /// `DocumentStartToken`: `---`.
    DocumentStart,
    /// `FlowEntryToken`: `,`.
    FlowEntry,
    /// `FlowMappingEndToken`: `}`.
    FlowMappingEnd,
    /// `FlowMappingStartToken`: `{`.
    FlowMappingStart,
    /// `FlowSequenceEndToken`: `]`.
    FlowSequenceEnd,
    /// `FlowSequenceStartToken`: `[`.
    FlowSequenceStart,
    /// `KeyToken`: a mapping key follows.
    Key,
    /// `ScalarToken`.
    Scalar,
    /// `StreamEndToken`.
    StreamEnd,
    /// `StreamStartToken`.
    StreamStart,
    /// `TagToken`: `!handle!suffix` or `!<uri>`.
    Tag,
    /// `ValueToken`: a mapping value follows.
    Value,
}

impl std::fmt::Display for TokenId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            TokenId::Alias => "<alias>",
            TokenId::Anchor => "<anchor>",
            TokenId::BlockEnd => "<block end>",
            TokenId::BlockEntry => "-",
            TokenId::BlockMappingStart => "<block mapping start>",
            TokenId::BlockSequenceStart => "<block sequence start>",
            TokenId::Directive => "<directive>",
            TokenId::DocumentEnd => "<document end>",
            TokenId::DocumentStart => "<document start>",
            TokenId::FlowEntry => ",",
            TokenId::FlowMappingEnd => "}",
            TokenId::FlowMappingStart => "{",
            TokenId::FlowSequenceEnd => "]",
            TokenId::FlowSequenceStart => "[",
            TokenId::Key => "?",
            TokenId::Scalar => "<scalar>",
            TokenId::StreamEnd => "<stream end>",
            TokenId::StreamStart => "<stream start>",
            TokenId::Tag => "<tag>",
            TokenId::Value => ":",
        })
    }
}

/// The value of a `DirectiveToken`.
#[derive(Clone, Debug)]
pub enum DirectiveValue {
    /// An unknown directive, whose parameters are ignored.
    None,
    /// `%YAML major.minor`.
    Yaml(i32, i32),
    /// `%TAG handle prefix`.
    Tag(String, String),
}

/// The payload of a [`Token`], by token kind.
#[derive(Clone, Debug)]
pub enum TokenData {
    /// A token that carries no data.
    None,
    /// alias / anchor name
    Name(String),
    /// A `DirectiveToken`.
    Directive {
        /// The directive name without `%`, e.g. `YAML` or `TAG`.
        name: String,
        /// The parsed parameters.
        value: DirectiveValue,
    },
    /// (handle, suffix)
    Tag(Option<String>, String),
    /// A `ScalarToken`.
    Scalar {
        /// The scalar text with escapes and folding applied.
        value: String,
        /// `true` for a plain (unquoted, non-block) scalar.
        plain: bool,
        /// The scalar style it was written in.
        style: ScalarStyle,
    },
}

/// `org.yaml.snakeyaml.tokens.Token`: one scanner token with its position.
#[derive(Clone, Debug)]
pub struct Token {
    /// `getTokenId()`: the kind of token.
    pub id: TokenId,
    /// The token's payload.
    pub data: TokenData,
    /// `getStartMark()`: where the token starts in the input.
    pub start_mark: Mark,
    /// `getEndMark()`: where the token ends in the input.
    pub end_mark: Mark,
}

impl Token {
    fn simple(id: TokenId, start: Mark, end: Mark) -> Token {
        Token { id, data: TokenData::None, start_mark: start, end_mark: end }
    }
}

struct SimpleKey {
    token_number: usize,
    required: bool,
    index: usize,
    line: usize,
    column: usize,
    mark: Mark,
}

const NULL_BL_T_LINEBR: &[i32] = &[9, 32, 0, 13, 10, 0x85, 0x2028, 0x2029];
const NULL_BL_LINEBR: &[i32] = &[32, 0, 13, 10, 0x85, 0x2028, 0x2029];
const NULL_OR_LINEBR: &[i32] = &[0, 13, 10, 0x85, 0x2028, 0x2029];
const LINEBR: &[i32] = &[10, 0x85, 0x2028, 0x2029];

fn has(set: &[i32], c: i32) -> bool {
    set.contains(&c)
}

fn has_extra(set: &[i32], c: i32, extra: &str) -> bool {
    has(set, c) || extra.chars().any(|e| e as i32 == c)
}

fn in_str(s: &str, c: i32) -> bool {
    s.chars().any(|e| e as i32 == c)
}

fn is_alpha(c: i32) -> bool {
    (c >= 'a' as i32 && c <= 'z' as i32) || (c >= 'A' as i32 && c <= 'Z' as i32) || (c >= '0' as i32 && c <= '9' as i32) || c == '-' as i32 || c == '_' as i32
}

fn is_uri_char(c: i32) -> bool {
    is_alpha(c) || in_str("-;/?:@&=+$,_.!~*'()[]%", c)
}

/// `Character.isDigit` (Unicode decimal digits)
fn is_digit(c: i32) -> bool {
    (48..=57).contains(&c)
}

fn cp_str(c: i32) -> String {
    let mut s = String::new();
    crate::error::push_code_point(&mut s, c);
    s
}

/// `ESCAPE_REPLACEMENTS`
fn escape_replacement(c: i32) -> Option<&'static str> {
    Some(match char::from_u32(c as u32)? {
        '0' => "\u{0}",
        'a' => "\u{7}",
        'b' => "\u{8}",
        't' => "\t",
        'n' => "\n",
        'v' => "\u{b}",
        'f' => "\u{c}",
        'r' => "\r",
        'e' => "\u{1b}",
        ' ' => " ",
        '"' => "\"",
        '\\' => "\\",
        'N' => "\u{85}",
        '_' => "\u{a0}",
        'L' => "\u{2028}",
        'P' => "\u{2029}",
        _ => return None,
    })
}

/// `org.yaml.snakeyaml.scanner.ScannerImpl`: turns the character stream into [`Token`]s on demand.
pub struct Scanner {
    reader: StreamReader,
    done: bool,
    flow_level: usize,
    tokens: VecDeque<Token>,
    last_token_id: Option<TokenId>,
    tokens_taken: usize,
    indent: i64,
    indents: Vec<i64>,
    options: LoaderOptions,
    allow_simple_key: bool,
    /// keyed by flow level, in insertion order (a LinkedHashMap in Java)
    possible_simple_keys: Vec<(usize, SimpleKey)>,
}

impl Scanner {
    /// A scanner over `reader`; the `StreamStart` token is already queued.
    pub fn new(reader: StreamReader, options: LoaderOptions) -> Scanner {
        let mut s = Scanner {
            reader,
            done: false,
            flow_level: 0,
            tokens: VecDeque::new(),
            last_token_id: None,
            tokens_taken: 0,
            indent: -1,
            indents: Vec::new(),
            options,
            allow_simple_key: true,
            possible_simple_keys: Vec::new(),
        };
        let mark = s.reader.get_mark();
        s.add_token(Token::simple(TokenId::StreamStart, mark.clone(), mark));
        s
    }

    fn fill(&mut self) -> Result<()> {
        while self.need_more_tokens()? {
            self.fetch_more_tokens()?;
        }
        Ok(())
    }

    /// `checkToken(choices)`: `true` when the next token is one of `choices` (any token when empty), `false` at the end.
    pub fn check_token(&mut self, choices: &[TokenId]) -> Result<bool> {
        self.fill()?;
        Ok(match self.tokens.front() {
            Some(t) => choices.is_empty() || choices.contains(&t.id),
            None => false,
        })
    }

    /// `peekToken()`: the next token, left in the queue.
    pub fn peek_token(&mut self) -> Result<&Token> {
        self.fill()?;
        Ok(self.tokens.front().expect("token available"))
    }

    /// `getToken()`: removes and returns the next token.
    pub fn get_token(&mut self) -> Result<Token> {
        self.fill()?;
        self.tokens_taken += 1;
        Ok(self.tokens.pop_front().expect("token available"))
    }

    /// `resetDocumentIndex()`: restarts the per-document code-point count used for the document size limit.
    pub fn reset_document_index(&mut self) {
        self.reader.reset_document_index();
    }

    fn add_token(&mut self, t: Token) {
        self.last_token_id = Some(t.id);
        self.tokens.push_back(t);
    }

    fn insert_token(&mut self, index: usize, t: Token) {
        if index == self.tokens.len() {
            self.last_token_id = Some(t.id);
        }
        self.tokens.insert(index, t);
    }

    fn need_more_tokens(&mut self) -> Result<bool> {
        if self.done {
            return Ok(false);
        }
        if self.tokens.is_empty() {
            return Ok(true);
        }
        self.stale_possible_simple_keys()?;
        Ok(self.next_possible_simple_key() == Some(self.tokens_taken))
    }

    fn next_possible_simple_key(&self) -> Option<usize> {
        self.possible_simple_keys.first().map(|(_, k)| k.token_number)
    }

    fn stale_possible_simple_keys(&mut self) -> Result<()> {
        let line = self.reader.get_line();
        let index = self.reader.get_index();
        let mut i = 0;
        while i < self.possible_simple_keys.len() {
            let key = &self.possible_simple_keys[i].1;
            if key.line != line || index - key.index > 1024 {
                if key.required {
                    return Err(YamlError::scanner(
                        Some("while scanning a simple key"),
                        Some(key.mark.clone()),
                        "could not find expected ':'",
                        self.reader.get_mark(),
                    ));
                }
                self.possible_simple_keys.remove(i);
            } else {
                i += 1;
            }
        }
        Ok(())
    }

    fn save_possible_simple_key(&mut self) -> Result<()> {
        let required = self.flow_level == 0 && self.indent == self.reader.get_column() as i64;
        if !self.allow_simple_key && required {
            return Err(YamlError::plain("A simple key is required only if it is the first token in the current line"));
        }
        if self.allow_simple_key {
            self.remove_possible_simple_key()?;
            let token_number = self.tokens_taken + self.tokens.len();
            let key = SimpleKey {
                token_number,
                required,
                index: self.reader.get_index(),
                line: self.reader.get_line(),
                column: self.reader.get_column(),
                mark: self.reader.get_mark(),
            };
            // LinkedHashMap.put on an absent key appends
            self.possible_simple_keys.push((self.flow_level, key));
        }
        Ok(())
    }

    fn take_simple_key(&mut self) -> Option<SimpleKey> {
        let pos = self.possible_simple_keys.iter().position(|(l, _)| *l == self.flow_level)?;
        Some(self.possible_simple_keys.remove(pos).1)
    }

    fn remove_possible_simple_key(&mut self) -> Result<()> {
        if let Some(key) = self.take_simple_key() {
            if key.required {
                return Err(YamlError::scanner(Some("while scanning a simple key"), Some(key.mark), "could not find expected ':'", self.reader.get_mark()));
            }
        }
        Ok(())
    }

    fn unwind_indent(&mut self, col: i64) {
        if self.flow_level == 0 {
            while self.indent > col {
                let mark = self.reader.get_mark();
                self.indent = self.indents.pop().unwrap_or(-1);
                self.add_token(Token::simple(TokenId::BlockEnd, mark.clone(), mark));
            }
        }
    }

    fn add_indent(&mut self, column: i64) -> bool {
        if self.indent < column {
            self.indents.push(self.indent);
            self.indent = column;
            true
        } else {
            false
        }
    }

    fn fetch_more_tokens(&mut self) -> Result<()> {
        if self.reader.get_document_index() > self.options.code_point_limit {
            return Err(YamlError::plain(format!(
                "The incoming YAML document exceeds the limit: {} code points.",
                self.options.code_point_limit
            )));
        }
        self.scan_to_next_token()?;
        self.stale_possible_simple_keys()?;
        self.unwind_indent(self.reader.get_column() as i64);
        let c = self.reader.peek()?;
        match c {
            0 => return self.fetch_stream_end(),
            33 => return self.fetch_tag(),
            34 => return self.fetch_flow_scalar('"'),
            37 => {
                if self.reader.get_column() == 0 {
                    return self.fetch_directive();
                }
            }
            38 => return self.fetch_anchor_or_alias(true),
            39 => return self.fetch_flow_scalar('\''),
            42 => return self.fetch_anchor_or_alias(false),
            44 => return self.fetch_flow_entry(),
            45 => {
                if self.check_document_indicator("---")? {
                    return self.fetch_document_indicator(true);
                }
                if has(NULL_BL_T_LINEBR, self.reader.peek_at(1)?) {
                    return self.fetch_block_entry();
                }
            }
            46 => {
                if self.check_document_indicator("...")? {
                    return self.fetch_document_indicator(false);
                }
            }
            58 => {
                if self.flow_level != 0 || has(NULL_BL_T_LINEBR, self.reader.peek_at(1)?) {
                    return self.fetch_value();
                }
            }
            62 => {
                if self.flow_level == 0 {
                    return self.fetch_block_scalar('>');
                }
            }
            63 => {
                if self.flow_level != 0 || has(NULL_BL_T_LINEBR, self.reader.peek_at(1)?) {
                    return self.fetch_key();
                }
            }
            91 => return self.fetch_flow_collection_start(false),
            93 => return self.fetch_flow_collection_end(false),
            123 => return self.fetch_flow_collection_start(true),
            124 => {
                if self.flow_level == 0 {
                    return self.fetch_block_scalar('|');
                }
            }
            125 => return self.fetch_flow_collection_end(true),
            _ => {}
        }
        if self.check_plain()? {
            return self.fetch_plain();
        }
        let mut rep = cp_str(c);
        // escapeChar: reverse lookup in ESCAPE_REPLACEMENTS
        for (k, v) in [('0', "\u{0}"), ('a', "\u{7}"), ('b', "\u{8}"), ('t', "\t"), ('n', "\n"), ('v', "\u{b}"), ('f', "\u{c}"), ('r', "\r"), ('e', "\u{1b}"), (' ', " "), ('"', "\""), ('\\', "\\"), ('_', "\u{a0}"), ('N', "\u{85}"), ('L', "\u{2028}"), ('P', "\u{2029}")] {
            if v == rep {
                rep = format!("\\{k}");
                break;
            }
        }
        if c == 9 {
            rep.push_str("(TAB)");
        }
        let text = format!("found character '{rep}' that cannot start any token. (Do not use {rep} for indentation)");
        Err(YamlError::scanner(Some("while scanning for the next token"), None, &text, self.reader.get_mark()))
    }

    fn check_document_indicator(&mut self, s: &str) -> Result<bool> {
        if self.reader.get_column() != 0 {
            return Ok(false);
        }
        Ok(self.reader.prefix(3)? == s && has(NULL_BL_T_LINEBR, self.reader.peek_at(3)?))
    }

    fn check_plain(&mut self) -> Result<bool> {
        let c = self.reader.peek()?;
        Ok(!has_extra(NULL_BL_T_LINEBR, c, "-?:,[]{}#&*!|>'\"%@`")
            || (!has(NULL_BL_T_LINEBR, self.reader.peek_at(1)?) && (c == 45 || (self.flow_level == 0 && in_str("?:", c)))))
    }

    fn fetch_stream_end(&mut self) -> Result<()> {
        self.unwind_indent(-1);
        self.remove_possible_simple_key()?;
        self.allow_simple_key = false;
        self.possible_simple_keys.clear();
        let mark = self.reader.get_mark();
        self.add_token(Token::simple(TokenId::StreamEnd, mark.clone(), mark));
        self.done = true;
        Ok(())
    }

    fn fetch_directive(&mut self) -> Result<()> {
        self.unwind_indent(-1);
        self.remove_possible_simple_key()?;
        self.allow_simple_key = false;
        let t = self.scan_directive()?;
        self.add_token(t);
        Ok(())
    }

    fn fetch_document_indicator(&mut self, start: bool) -> Result<()> {
        self.unwind_indent(-1);
        self.remove_possible_simple_key()?;
        self.allow_simple_key = false;
        let s = self.reader.get_mark();
        self.reader.forward_n(3)?;
        let e = self.reader.get_mark();
        self.add_token(Token::simple(if start { TokenId::DocumentStart } else { TokenId::DocumentEnd }, s, e));
        Ok(())
    }

    fn fetch_flow_collection_start(&mut self, mapping: bool) -> Result<()> {
        self.save_possible_simple_key()?;
        self.flow_level += 1;
        self.allow_simple_key = true;
        let s = self.reader.get_mark();
        self.reader.forward_n(1)?;
        let e = self.reader.get_mark();
        self.add_token(Token::simple(if mapping { TokenId::FlowMappingStart } else { TokenId::FlowSequenceStart }, s, e));
        Ok(())
    }

    fn fetch_flow_collection_end(&mut self, mapping: bool) -> Result<()> {
        self.remove_possible_simple_key()?;
        // Java decrements an int; a stray closing bracket makes it -1 (we saturate, the parser fails anyway)
        self.flow_level = self.flow_level.saturating_sub(1);
        self.allow_simple_key = false;
        let s = self.reader.get_mark();
        self.reader.forward()?;
        let e = self.reader.get_mark();
        self.add_token(Token::simple(if mapping { TokenId::FlowMappingEnd } else { TokenId::FlowSequenceEnd }, s, e));
        Ok(())
    }

    fn fetch_flow_entry(&mut self) -> Result<()> {
        self.allow_simple_key = true;
        self.remove_possible_simple_key()?;
        let s = self.reader.get_mark();
        self.reader.forward()?;
        let e = self.reader.get_mark();
        self.add_token(Token::simple(TokenId::FlowEntry, s, e));
        Ok(())
    }

    fn fetch_block_entry(&mut self) -> Result<()> {
        if self.flow_level == 0 {
            if !self.allow_simple_key {
                return Err(YamlError::scanner(None, None, "sequence entries are not allowed here", self.reader.get_mark()));
            }
            if self.add_indent(self.reader.get_column() as i64) {
                let m = self.reader.get_mark();
                self.add_token(Token::simple(TokenId::BlockSequenceStart, m.clone(), m));
            }
        }
        self.allow_simple_key = true;
        self.remove_possible_simple_key()?;
        let s = self.reader.get_mark();
        self.reader.forward()?;
        let e = self.reader.get_mark();
        self.add_token(Token::simple(TokenId::BlockEntry, s, e));
        Ok(())
    }

    fn fetch_key(&mut self) -> Result<()> {
        if self.flow_level == 0 {
            if !self.allow_simple_key {
                return Err(YamlError::scanner(None, None, "mapping keys are not allowed here", self.reader.get_mark()));
            }
            if self.add_indent(self.reader.get_column() as i64) {
                let m = self.reader.get_mark();
                self.add_token(Token::simple(TokenId::BlockMappingStart, m.clone(), m));
            }
        }
        self.allow_simple_key = self.flow_level == 0;
        self.remove_possible_simple_key()?;
        let s = self.reader.get_mark();
        self.reader.forward()?;
        let e = self.reader.get_mark();
        self.add_token(Token::simple(TokenId::Key, s, e));
        Ok(())
    }

    fn fetch_value(&mut self) -> Result<()> {
        if let Some(key) = self.take_simple_key() {
            let idx = key.token_number - self.tokens_taken;
            self.insert_token(idx, Token::simple(TokenId::Key, key.mark.clone(), key.mark.clone()));
            if self.flow_level == 0 && self.add_indent(key.column as i64) {
                self.insert_token(idx, Token::simple(TokenId::BlockMappingStart, key.mark.clone(), key.mark.clone()));
            }
            self.allow_simple_key = false;
        } else {
            if self.flow_level == 0 && !self.allow_simple_key {
                return Err(YamlError::scanner(None, None, "mapping values are not allowed here", self.reader.get_mark()));
            }
            if self.flow_level == 0 && self.add_indent(self.reader.get_column() as i64) {
                let m = self.reader.get_mark();
                self.add_token(Token::simple(TokenId::BlockMappingStart, m.clone(), m));
            }
            self.allow_simple_key = self.flow_level == 0;
            self.remove_possible_simple_key()?;
        }
        let s = self.reader.get_mark();
        self.reader.forward()?;
        let e = self.reader.get_mark();
        self.add_token(Token::simple(TokenId::Value, s, e));
        Ok(())
    }

    fn fetch_anchor_or_alias(&mut self, anchor: bool) -> Result<()> {
        self.save_possible_simple_key()?;
        self.allow_simple_key = false;
        let t = self.scan_anchor(anchor)?;
        self.add_token(t);
        Ok(())
    }

    fn fetch_tag(&mut self) -> Result<()> {
        self.save_possible_simple_key()?;
        self.allow_simple_key = false;
        let t = self.scan_tag()?;
        self.add_token(t);
        Ok(())
    }

    fn fetch_block_scalar(&mut self, style: char) -> Result<()> {
        self.allow_simple_key = true;
        self.remove_possible_simple_key()?;
        let t = self.scan_block_scalar(style)?;
        self.add_token(t);
        Ok(())
    }

    fn fetch_flow_scalar(&mut self, style: char) -> Result<()> {
        self.save_possible_simple_key()?;
        self.allow_simple_key = false;
        let t = self.scan_flow_scalar(style)?;
        self.add_token(t);
        Ok(())
    }

    fn fetch_plain(&mut self) -> Result<()> {
        self.save_possible_simple_key()?;
        self.allow_simple_key = false;
        let t = self.scan_plain()?;
        self.add_token(t);
        Ok(())
    }

    fn scan_to_next_token(&mut self) -> Result<()> {
        if self.reader.get_index() == 0 && self.reader.peek()? == 65279 {
            self.reader.forward()?;
        }
        loop {
            let mut ff = 0;
            while self.reader.peek_at(ff)? == 32 {
                ff += 1;
            }
            while self.reader.peek_at(ff)? == 9 && self.flow_level > 0 {
                ff += 1;
            }
            if ff > 0 {
                self.reader.forward_n(ff)?;
            }
            if self.reader.peek()? == 35 {
                self.scan_comment()?;
            }
            let breaks = self.scan_line_break()?;
            if !breaks.is_empty() {
                if self.flow_level == 0 {
                    self.allow_simple_key = true;
                }
            } else {
                return Ok(());
            }
        }
    }

    fn scan_comment(&mut self) -> Result<String> {
        self.reader.forward()?;
        let mut length = 0;
        while !has(NULL_OR_LINEBR, self.reader.peek_at(length)?) {
            length += 1;
        }
        self.reader.prefix_forward(length)
    }

    fn scan_directive(&mut self) -> Result<Token> {
        let start = self.reader.get_mark();
        self.reader.forward()?;
        let name = self.scan_directive_name(&start)?;
        let value;
        let end;
        if name == "YAML" {
            value = self.scan_yaml_directive_value(&start)?;
            end = self.reader.get_mark();
        } else if name == "TAG" {
            value = self.scan_tag_directive_value(&start)?;
            end = self.reader.get_mark();
        } else {
            end = self.reader.get_mark();
            let mut ff = 0;
            while !has(NULL_OR_LINEBR, self.reader.peek_at(ff)?) {
                ff += 1;
            }
            if ff > 0 {
                self.reader.forward_n(ff)?;
            }
            value = DirectiveValue::None;
        }
        self.scan_directive_ignored_line(&start)?;
        Ok(Token { id: TokenId::Directive, data: TokenData::Directive { name, value }, start_mark: start, end_mark: end })
    }

    fn directive_err(&self, start: &Mark, problem: String) -> YamlError {
        YamlError::scanner(Some("while scanning a directive"), Some(start.clone()), &problem, self.reader.get_mark())
    }

    fn scan_directive_name(&mut self, start: &Mark) -> Result<String> {
        let mut length = 0;
        let mut c = self.reader.peek_at(length)?;
        while is_alpha(c) {
            length += 1;
            c = self.reader.peek_at(length)?;
        }
        if length == 0 {
            return Err(self.directive_err(start, format!("expected alphabetic or numeric character, but found {}({c})", cp_str(c))));
        }
        let value = self.reader.prefix_forward(length)?;
        let c = self.reader.peek()?;
        if !has(NULL_BL_LINEBR, c) {
            return Err(self.directive_err(start, format!("expected alphabetic or numeric character, but found {}({c})", cp_str(c))));
        }
        Ok(value)
    }

    fn scan_yaml_directive_value(&mut self, start: &Mark) -> Result<DirectiveValue> {
        while self.reader.peek()? == 32 {
            self.reader.forward()?;
        }
        let major = self.scan_yaml_directive_number(start)?;
        let c = self.reader.peek()?;
        if c != 46 {
            return Err(self.directive_err(start, format!("expected a digit or '.', but found {}({c})", cp_str(c))));
        }
        self.reader.forward()?;
        let minor = self.scan_yaml_directive_number(start)?;
        let c = self.reader.peek()?;
        if !has(NULL_BL_LINEBR, c) {
            return Err(self.directive_err(start, format!("expected a digit or ' ', but found {}({c})", cp_str(c))));
        }
        Ok(DirectiveValue::Yaml(major, minor))
    }

    fn scan_yaml_directive_number(&mut self, start: &Mark) -> Result<i32> {
        let c = self.reader.peek()?;
        if !is_digit(c) {
            return Err(self.directive_err(start, format!("expected a digit, but found {}({c})", cp_str(c))));
        }
        let mut length = 0;
        while is_digit(self.reader.peek_at(length)?) {
            length += 1;
        }
        let number = self.reader.prefix_forward(length)?;
        if length > 3 {
            return Err(YamlError::scanner(
                Some("while scanning a YAML directive"),
                Some(start.clone()),
                &format!("found a number which cannot represent a valid version: {number}"),
                self.reader.get_mark(),
            ));
        }
        let ascii: String = number.chars().map(|c| char::from_digit(c.to_digit(10).unwrap_or(0), 10).unwrap()).collect();
        Ok(ascii.parse().unwrap_or(0))
    }

    fn scan_tag_directive_value(&mut self, start: &Mark) -> Result<DirectiveValue> {
        while self.reader.peek()? == 32 {
            self.reader.forward()?;
        }
        let handle = self.scan_tag_handle("directive", start)?;
        let c = self.reader.peek()?;
        if c != 32 {
            return Err(self.directive_err(start, format!("expected ' ', but found {}({c})", cp_str(c))));
        }
        while self.reader.peek()? == 32 {
            self.reader.forward()?;
        }
        let prefix = self.scan_tag_uri("directive", start)?;
        let c = self.reader.peek()?;
        if !has(NULL_BL_LINEBR, c) {
            return Err(self.directive_err(start, format!("expected ' ', but found {}({c})", cp_str(c))));
        }
        Ok(DirectiveValue::Tag(handle, prefix))
    }

    fn scan_directive_ignored_line(&mut self, start: &Mark) -> Result<()> {
        while self.reader.peek()? == 32 {
            self.reader.forward()?;
        }
        if self.reader.peek()? == 35 {
            self.scan_comment()?;
        }
        let c = self.reader.peek()?;
        let lb = self.scan_line_break()?;
        if lb.is_empty() && c != 0 {
            return Err(self.directive_err(start, format!("expected a comment or a line break, but found {}({c})", cp_str(c))));
        }
        Ok(())
    }

    fn scan_anchor(&mut self, is_anchor: bool) -> Result<Token> {
        let start = self.reader.get_mark();
        let indicator = self.reader.peek()?;
        let name = if indicator == 42 { "alias" } else { "anchor" };
        self.reader.forward()?;
        let mut length = 0;
        let mut c = self.reader.peek_at(length)?;
        while !has_extra(NULL_BL_T_LINEBR, c, ":,[]{}/.*&") {
            length += 1;
            c = self.reader.peek_at(length)?;
        }
        let ctx = format!("while scanning an {name}");
        if length == 0 {
            return Err(YamlError::scanner(Some(&ctx), Some(start), &format!("unexpected character found {}({c})", cp_str(c)), self.reader.get_mark()));
        }
        let value = self.reader.prefix_forward(length)?;
        let c = self.reader.peek()?;
        if !has_extra(NULL_BL_T_LINEBR, c, "?:,]}%@`") {
            return Err(YamlError::scanner(Some(&ctx), Some(start), &format!("unexpected character found {}({c})", cp_str(c)), self.reader.get_mark()));
        }
        let end = self.reader.get_mark();
        Ok(Token { id: if is_anchor { TokenId::Anchor } else { TokenId::Alias }, data: TokenData::Name(value), start_mark: start, end_mark: end })
    }

    fn scan_tag(&mut self) -> Result<Token> {
        let start = self.reader.get_mark();
        let mut c = self.reader.peek_at(1)?;
        let mut handle: Option<String> = None;
        let suffix;
        if c == 60 {
            self.reader.forward_n(2)?;
            suffix = self.scan_tag_uri("tag", &start)?;
            c = self.reader.peek()?;
            if c != 62 {
                return Err(YamlError::scanner(Some("while scanning a tag"), Some(start), &format!("expected '>', but found '{}' ({c})", cp_str(c)), self.reader.get_mark()));
            }
            self.reader.forward()?;
        } else if has(NULL_BL_T_LINEBR, c) {
            suffix = "!".to_string();
            self.reader.forward()?;
        } else {
            let mut length = 1;
            let mut use_handle = false;
            while !has(NULL_BL_LINEBR, c) {
                if c == 33 {
                    use_handle = true;
                    break;
                }
                length += 1;
                c = self.reader.peek_at(length)?;
            }
            if use_handle {
                handle = Some(self.scan_tag_handle("tag", &start)?);
            } else {
                handle = Some("!".to_string());
                self.reader.forward()?;
            }
            suffix = self.scan_tag_uri("tag", &start)?;
        }
        let c = self.reader.peek()?;
        if !has(NULL_BL_LINEBR, c) {
            return Err(YamlError::scanner(Some("while scanning a tag"), Some(start), &format!("expected ' ', but found '{}' ({c})", cp_str(c)), self.reader.get_mark()));
        }
        let end = self.reader.get_mark();
        Ok(Token { id: TokenId::Tag, data: TokenData::Tag(handle, suffix), start_mark: start, end_mark: end })
    }

    fn scan_block_scalar(&mut self, style: char) -> Result<Token> {
        let folded = style == '>';
        let mut chunks = String::new();
        let start = self.reader.get_mark();
        self.reader.forward()?;
        let (chomping, increment) = self.scan_block_scalar_indicators(&start)?;
        self.scan_block_scalar_ignored_line(&start)?;
        let min_indent = (self.indent + 1).max(1);
        let mut breaks;
        let indent;
        let mut end;
        if increment == -1 {
            let (b, max_indent, e) = self.scan_block_scalar_indentation()?;
            breaks = b;
            end = e;
            indent = min_indent.max(max_indent as i64);
        } else {
            indent = min_indent + increment - 1;
            let (b, e) = self.scan_block_scalar_breaks(indent)?;
            breaks = b;
            end = e;
        }
        let mut line_break = String::new();
        while self.reader.get_column() as i64 == indent && self.reader.peek()? != 0 {
            chunks.push_str(&breaks);
            let leading_non_space = !in_str(" \t", self.reader.peek()?);
            let mut length = 0;
            while !has(NULL_OR_LINEBR, self.reader.peek_at(length)?) {
                length += 1;
            }
            chunks.push_str(&self.reader.prefix_forward(length)?);
            line_break = self.scan_line_break()?;
            let (b, e) = self.scan_block_scalar_breaks(indent)?;
            breaks = b;
            end = e;
            if self.reader.get_column() as i64 != indent || self.reader.peek()? == 0 {
                break;
            }
            if folded && line_break == "\n" && leading_non_space && !in_str(" \t", self.reader.peek()?) {
                if breaks.is_empty() {
                    chunks.push(' ');
                }
            } else {
                chunks.push_str(&line_break);
            }
        }
        if chomping != Some(false) {
            chunks.push_str(&line_break);
        }
        if chomping == Some(true) {
            chunks.push_str(&breaks);
        }
        let style = if folded { ScalarStyle::Folded } else { ScalarStyle::Literal };
        Ok(Token { id: TokenId::Scalar, data: TokenData::Scalar { value: chunks, plain: false, style }, start_mark: start, end_mark: end })
    }

    fn scan_block_scalar_indicators(&mut self, start: &Mark) -> Result<(Option<bool>, i64)> {
        let mut chomping = None;
        let mut increment: i64 = -1;
        let mut c = self.reader.peek()?;
        let zero_err = |s: &Scanner| {
            YamlError::scanner(Some("while scanning a block scalar"), Some(start.clone()), "expected indentation indicator in the range 1-9, but found 0", s.reader.get_mark())
        };
        if c == 45 || c == 43 {
            chomping = Some(c == 43);
            self.reader.forward()?;
            c = self.reader.peek()?;
            if is_digit(c) {
                increment = char::from_u32(c as u32).and_then(|ch| ch.to_digit(10)).unwrap_or(0) as i64;
                if increment == 0 {
                    return Err(zero_err(self));
                }
                self.reader.forward()?;
            }
        } else if is_digit(c) {
            increment = char::from_u32(c as u32).and_then(|ch| ch.to_digit(10)).unwrap_or(0) as i64;
            if increment == 0 {
                return Err(zero_err(self));
            }
            self.reader.forward()?;
            c = self.reader.peek()?;
            if c == 45 || c == 43 {
                chomping = Some(c == 43);
                self.reader.forward()?;
            }
        }
        let c = self.reader.peek()?;
        if !has(NULL_BL_LINEBR, c) {
            return Err(YamlError::scanner(
                Some("while scanning a block scalar"),
                Some(start.clone()),
                &format!("expected chomping or indentation indicators, but found {}({c})", cp_str(c)),
                self.reader.get_mark(),
            ));
        }
        Ok((chomping, increment))
    }

    fn scan_block_scalar_ignored_line(&mut self, start: &Mark) -> Result<()> {
        while self.reader.peek()? == 32 {
            self.reader.forward()?;
        }
        if self.reader.peek()? == 35 {
            self.scan_comment()?;
        }
        let c = self.reader.peek()?;
        let lb = self.scan_line_break()?;
        if lb.is_empty() && c != 0 {
            return Err(YamlError::scanner(
                Some("while scanning a block scalar"),
                Some(start.clone()),
                &format!("expected a comment or a line break, but found {}({c})", cp_str(c)),
                self.reader.get_mark(),
            ));
        }
        Ok(())
    }

    fn scan_block_scalar_indentation(&mut self) -> Result<(String, usize, Mark)> {
        let mut chunks = String::new();
        let mut max_indent = 0;
        let mut end = self.reader.get_mark();
        loop {
            let c = self.reader.peek()?;
            if !has_extra(LINEBR, c, " \r") {
                break;
            }
            if c != 32 {
                chunks.push_str(&self.scan_line_break()?);
                end = self.reader.get_mark();
            } else {
                self.reader.forward()?;
                if self.reader.get_column() > max_indent {
                    max_indent = self.reader.get_column();
                }
            }
        }
        Ok((chunks, max_indent, end))
    }

    fn scan_block_scalar_breaks(&mut self, indent: i64) -> Result<(String, Mark)> {
        let mut chunks = String::new();
        let mut end = self.reader.get_mark();
        let mut col = self.reader.get_column() as i64;
        while col < indent && self.reader.peek()? == 32 {
            self.reader.forward()?;
            col += 1;
        }
        loop {
            let lb = self.scan_line_break()?;
            if lb.is_empty() {
                break;
            }
            chunks.push_str(&lb);
            end = self.reader.get_mark();
            let mut col = self.reader.get_column() as i64;
            while col < indent && self.reader.peek()? == 32 {
                self.reader.forward()?;
                col += 1;
            }
        }
        Ok((chunks, end))
    }

    fn scan_flow_scalar(&mut self, style: char) -> Result<Token> {
        let double = style == '"';
        let mut chunks = String::new();
        let start = self.reader.get_mark();
        let quote = self.reader.peek()?;
        self.reader.forward()?;
        self.scan_flow_scalar_non_spaces(double, &start, &mut chunks)?;
        while self.reader.peek()? != quote {
            self.scan_flow_scalar_spaces(&start, &mut chunks)?;
            self.scan_flow_scalar_non_spaces(double, &start, &mut chunks)?;
        }
        self.reader.forward()?;
        let end = self.reader.get_mark();
        let style = if double { ScalarStyle::DoubleQuoted } else { ScalarStyle::SingleQuoted };
        Ok(Token { id: TokenId::Scalar, data: TokenData::Scalar { value: chunks, plain: false, style }, start_mark: start, end_mark: end })
    }

    fn scan_flow_scalar_non_spaces(&mut self, double: bool, start: &Mark, chunks: &mut String) -> Result<()> {
        loop {
            let mut length = 0;
            while !has_extra(NULL_BL_T_LINEBR, self.reader.peek_at(length)?, "'\"\\") {
                length += 1;
            }
            if length != 0 {
                chunks.push_str(&self.reader.prefix_forward(length)?);
            }
            let c = self.reader.peek()?;
            if !double && c == 39 && self.reader.peek_at(1)? == 39 {
                chunks.push('\'');
                self.reader.forward_n(2)?;
            } else if (double && c == 39) || (!double && in_str("\"\\", c)) {
                crate::error::push_code_point(chunks, c);
                self.reader.forward()?;
            } else if double && c == 92 {
                self.reader.forward()?;
                let c = self.reader.peek()?;
                if c < 0x10000 {
                    if let Some(r) = escape_replacement(c) {
                        chunks.push_str(r);
                        self.reader.forward()?;
                        continue;
                    }
                    let code_len = match c {
                        120 => Some(2),
                        117 => Some(4),
                        85 => Some(8),
                        _ => None,
                    };
                    if let Some(len) = code_len {
                        self.reader.forward()?;
                        let hex = self.reader.prefix(len)?;
                        if hex.chars().any(|h| !h.is_ascii_hexdigit()) {
                            return Err(YamlError::scanner(
                                Some("while scanning a double-quoted scalar"),
                                Some(start.clone()),
                                &format!("expected escape sequence of {len} hexadecimal numbers, but found: {hex}"),
                                self.reader.get_mark(),
                            ));
                        }
                        // Integer.parseInt of a short prefix (fewer than `len` chars at EOF) still parses
                        let decimal = i64::from_str_radix(&hex, 16).unwrap_or(-1);
                        match u32::try_from(decimal).ok().and_then(char::from_u32) {
                            Some(ch) if decimal <= 0x10FFFF => {
                                chunks.push(ch);
                                self.reader.forward_n(len)?;
                                continue;
                            }
                            _ if (0xD800..0xE000).contains(&decimal) => {
                                // appendCodePoint accepts lone surrogates; Rust strings cannot hold them
                                chunks.push('\u{FFFD}');
                                self.reader.forward_n(len)?;
                                continue;
                            }
                            _ => {
                                return Err(YamlError::scanner(
                                    Some("while scanning a double-quoted scalar"),
                                    Some(start.clone()),
                                    &format!("found unknown escape character {hex}"),
                                    self.reader.get_mark(),
                                ));
                            }
                        }
                    }
                }
                if !self.scan_line_break()?.is_empty() {
                    let b = self.scan_flow_scalar_breaks(start)?;
                    chunks.push_str(&b);
                    continue;
                }
                return Err(YamlError::scanner(
                    Some("while scanning a double-quoted scalar"),
                    Some(start.clone()),
                    &format!("found unknown escape character {}({c})", cp_str(c)),
                    self.reader.get_mark(),
                ));
            } else {
                return Ok(());
            }
        }
    }

    fn scan_flow_scalar_spaces(&mut self, start: &Mark, chunks: &mut String) -> Result<()> {
        let mut length = 0;
        while in_str(" \t", self.reader.peek_at(length)?) {
            length += 1;
        }
        let whitespaces = self.reader.prefix_forward(length)?;
        let c = self.reader.peek()?;
        if c == 0 {
            return Err(YamlError::scanner(Some("while scanning a quoted scalar"), Some(start.clone()), "found unexpected end of stream", self.reader.get_mark()));
        }
        let lb = self.scan_line_break()?;
        if !lb.is_empty() {
            let breaks = self.scan_flow_scalar_breaks(start)?;
            if lb != "\n" {
                chunks.push_str(&lb);
            } else if breaks.is_empty() {
                chunks.push(' ');
            }
            chunks.push_str(&breaks);
        } else {
            chunks.push_str(&whitespaces);
        }
        Ok(())
    }

    fn scan_flow_scalar_breaks(&mut self, start: &Mark) -> Result<String> {
        let mut chunks = String::new();
        loop {
            let prefix = self.reader.prefix(3)?;
            if (prefix == "---" || prefix == "...") && has(NULL_BL_T_LINEBR, self.reader.peek_at(3)?) {
                return Err(YamlError::scanner(Some("while scanning a quoted scalar"), Some(start.clone()), "found unexpected document separator", self.reader.get_mark()));
            }
            while in_str(" \t", self.reader.peek()?) {
                self.reader.forward()?;
            }
            let lb = self.scan_line_break()?;
            if lb.is_empty() {
                return Ok(chunks);
            }
            chunks.push_str(&lb);
        }
    }

    fn scan_plain(&mut self) -> Result<Token> {
        let mut chunks = String::new();
        let start = self.reader.get_mark();
        let mut end = start.clone();
        let indent = self.indent + 1;
        let mut spaces = String::new();
        let make = |chunks: String, start: Mark, end: Mark| Token {
            id: TokenId::Scalar,
            data: TokenData::Scalar { value: chunks, plain: true, style: ScalarStyle::Plain },
            start_mark: start,
            end_mark: end,
        };
        loop {
            let mut length = 0;
            if self.reader.peek()? == 35 {
                return Ok(make(chunks, start, end));
            }
            loop {
                let c = self.reader.peek_at(length)?;
                let stop = has(NULL_BL_T_LINEBR, c)
                    || (c == 58 && has_extra(NULL_BL_T_LINEBR, self.reader.peek_at(length + 1)?, if self.flow_level != 0 { ",[]{}" } else { "" }))
                    || (self.flow_level != 0 && in_str(",?[]{}", c));
                if stop {
                    if length == 0 {
                        return Ok(make(chunks, start, end));
                    }
                    self.allow_simple_key = false;
                    chunks.push_str(&spaces);
                    chunks.push_str(&self.reader.prefix_forward(length)?);
                    end = self.reader.get_mark();
                    spaces = self.scan_plain_spaces()?;
                    if spaces.is_empty() || self.reader.peek()? == 35 || (self.flow_level == 0 && (self.reader.get_column() as i64) < indent) {
                        return Ok(make(chunks, start, end));
                    }
                    break;
                }
                length += 1;
            }
        }
    }

    fn scan_plain_spaces(&mut self) -> Result<String> {
        let mut length = 0;
        while matches!(self.reader.peek_at(length)?, 32 | 9) {
            length += 1;
        }
        let whitespaces = self.reader.prefix_forward(length)?;
        let line_break = self.scan_line_break()?;
        if line_break.is_empty() {
            return Ok(whitespaces);
        }
        self.allow_simple_key = true;
        let mut prefix = self.reader.prefix(3)?;
        if prefix == "---" || (prefix == "..." && has(NULL_BL_T_LINEBR, self.reader.peek_at(3)?)) {
            return Ok(String::new());
        }
        let mut breaks = String::new();
        loop {
            while self.reader.peek()? == 32 {
                self.reader.forward()?;
            }
            let lb = self.scan_line_break()?;
            if lb.is_empty() {
                if line_break != "\n" {
                    return Ok(line_break + &breaks);
                }
                if breaks.is_empty() {
                    return Ok(" ".to_string());
                }
                return Ok(breaks);
            }
            breaks.push_str(&lb);
            prefix = self.reader.prefix(3)?;
            if prefix == "---" || (prefix == "..." && has(NULL_BL_T_LINEBR, self.reader.peek_at(3)?)) {
                return Ok(String::new());
            }
        }
    }

    fn scan_tag_handle(&mut self, name: &str, start: &Mark) -> Result<String> {
        let mut c = self.reader.peek()?;
        let ctx = format!("while scanning a {name}");
        if c != 33 {
            return Err(YamlError::scanner(Some(&ctx), Some(start.clone()), &format!("expected '!', but found {}({c})", cp_str(c)), self.reader.get_mark()));
        }
        let mut length = 1;
        c = self.reader.peek_at(length)?;
        if c != 32 {
            while is_alpha(c) {
                length += 1;
                c = self.reader.peek_at(length)?;
            }
            if c != 33 {
                self.reader.forward_n(length)?;
                return Err(YamlError::scanner(Some(&ctx), Some(start.clone()), &format!("expected '!', but found {}({c})", cp_str(c)), self.reader.get_mark()));
            }
            length += 1;
        }
        self.reader.prefix_forward(length)
    }

    fn scan_tag_uri(&mut self, name: &str, start: &Mark) -> Result<String> {
        let mut chunks = String::new();
        let mut length = 0;
        let mut c = self.reader.peek_at(length)?;
        while is_uri_char(c) {
            if c == 37 {
                chunks.push_str(&self.reader.prefix_forward(length)?);
                length = 0;
                chunks.push_str(&self.scan_uri_escapes(name, start)?);
            } else {
                length += 1;
            }
            c = self.reader.peek_at(length)?;
        }
        if length != 0 {
            chunks.push_str(&self.reader.prefix_forward(length)?);
        }
        if chunks.is_empty() {
            return Err(YamlError::scanner(
                Some(&format!("while scanning a {name}")),
                Some(start.clone()),
                &format!("expected URI, but found {}({c})", cp_str(c)),
                self.reader.get_mark(),
            ));
        }
        Ok(chunks)
    }

    fn scan_uri_escapes(&mut self, name: &str, start: &Mark) -> Result<String> {
        let beginning = self.reader.get_mark();
        let mut bytes = Vec::new();
        while self.reader.peek()? == 37 {
            self.reader.forward()?;
            let p = self.reader.prefix(2)?;
            match u8::from_str_radix(&p, 16) {
                Ok(b) if p.len() == 2 || p.len() == 1 => bytes.push(b),
                _ => {
                    let c1 = self.reader.peek()?;
                    let c2 = self.reader.peek_at(1)?;
                    return Err(YamlError::scanner(
                        Some(&format!("while scanning a {name}")),
                        Some(start.clone()),
                        &format!("expected URI escape sequence of 2 hexadecimal numbers, but found {}({c1}) and {}({c2})", cp_str(c1), cp_str(c2)),
                        self.reader.get_mark(),
                    ));
                }
            }
            self.reader.forward_n(2)?;
        }
        match String::from_utf8(bytes) {
            Ok(s) => Ok(s),
            Err(_) => Err(YamlError::scanner(Some(&format!("while scanning a {name}")), Some(start.clone()), "expected URI in UTF-8: Input length = 1", beginning)),
        }
    }

    fn scan_line_break(&mut self) -> Result<String> {
        let c = self.reader.peek()?;
        if c == 13 || c == 10 || c == 133 {
            if c == 13 && self.reader.peek_at(1)? == 10 {
                self.reader.forward_n(2)?;
            } else {
                self.reader.forward()?;
            }
            Ok("\n".to_string())
        } else if c == 8232 || c == 8233 {
            self.reader.forward()?;
            Ok(cp_str(c))
        } else {
            Ok(String::new())
        }
    }
}
