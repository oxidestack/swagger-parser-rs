//! `StreamReader`: code-point reader with SnakeYAML's 1024-char windowing (which shapes error snippets)
//! and its printable-character check.

use crate::error::{Mark, Result, YamlError, YamlErrorKind, READER_EXCEPTION};
use std::sync::Arc;

/// `org.yaml.snakeyaml.reader.StreamReader`: reads the input one code point at a time, tracking line, column and index for marks. Reads return `Err` (`ReaderException`) on a non-printable character or an unpaired high surrogate.
pub struct StreamReader {
    name: Arc<str>,
    src: Vec<u16>,
    src_pos: usize,
    window: Arc<Vec<i32>>,
    pointer: usize,
    eof: bool,
    index: usize,
    document_index: usize,
    line: usize,
    column: usize,
}

/// `StreamReader.isPrintable(int)`
pub fn is_printable(c: i32) -> bool {
    (32..=126).contains(&c)
        || c == 9
        || c == 10
        || c == 13
        || c == 133
        || (160..=55295).contains(&c)
        || (57344..=65533).contains(&c)
        || (65536..=1114111).contains(&c)
}

fn is_linebr(c: i32) -> bool {
    matches!(c, 10 | 0x85 | 0x2028 | 0x2029)
}

impl StreamReader {
    /// `new StreamReader(String)` — named `'string'` (what `Yaml.load(String)` uses).
    pub fn new(input: &str) -> StreamReader {
        Self::with_name(input, "'string'")
    }

    /// `new StreamReader(Reader)` — named `'reader'` (what Jackson's YAMLParser uses).
    pub fn from_reader_input(input: &str) -> StreamReader {
        Self::with_name(input, "'reader'")
    }

    /// A reader over `input` whose marks carry `name` as the source name.
    pub fn with_name(input: &str, name: &str) -> StreamReader {
        StreamReader {
            name: name.into(),
            src: input.encode_utf16().collect(),
            src_pos: 0,
            window: Arc::new(Vec::new()),
            pointer: 0,
            eof: false,
            index: 0,
            document_index: 0,
            line: 0,
            column: 0,
        }
    }

    /// `getMark()`: the current position, with the window snippet used in error messages.
    pub fn get_mark(&self) -> Mark {
        Mark { name: self.name.clone(), index: self.index, line: self.line, column: self.column, buffer: self.window.clone(), pointer: self.pointer }
    }

    /// `forward()`: advances one code point.
    pub fn forward(&mut self) -> Result<()> {
        self.forward_n(1)
    }

    /// `forward(length)`: advances `length` code points (fewer at the end of input), counting line breaks.
    pub fn forward_n(&mut self, length: usize) -> Result<()> {
        let mut i = 0;
        while i < length && self.ensure(0)? {
            let c = self.window[self.pointer];
            self.pointer += 1;
            self.index += 1;
            self.document_index += 1;
            let newline = is_linebr(c) || (c == 13 && self.ensure(0)? && self.window[self.pointer] != 10);
            if newline {
                self.line += 1;
                self.column = 0;
            } else if c != 65279 {
                self.column += 1;
            }
            i += 1;
        }
        Ok(())
    }

    /// `peek()`: the current code point, `0` at the end of input.
    pub fn peek(&mut self) -> Result<i32> {
        Ok(if self.ensure(0)? { self.window[self.pointer] } else { 0 })
    }

    /// `peek(index)`: the code point `index` ahead of the current one, `0` past the end of input.
    pub fn peek_at(&mut self, index: usize) -> Result<i32> {
        Ok(if self.ensure(index)? { self.window[self.pointer + index] } else { 0 })
    }

    /// `prefix(length)`: the next `length` code points as a string (fewer at the end of input), without advancing.
    pub fn prefix(&mut self, length: usize) -> Result<String> {
        if length == 0 {
            return Ok(String::new());
        }
        let n = if self.ensure(length)? { length } else { length.min(self.window.len() - self.pointer) };
        let mut s = String::new();
        for &cp in &self.window[self.pointer..self.pointer + n] {
            crate::error::push_code_point(&mut s, cp);
        }
        Ok(s)
    }

    /// `prefixForward(length)`: [`Self::prefix`] then advances past it; the column moves by `length` (no line breaks expected).
    pub fn prefix_forward(&mut self, length: usize) -> Result<String> {
        let p = self.prefix(length)?;
        self.pointer += length;
        self.index += length;
        self.document_index += length;
        self.column += length;
        Ok(p)
    }

    fn ensure(&mut self, size: usize) -> Result<bool> {
        if !self.eof && self.pointer + size >= self.window.len() {
            self.update()?;
        }
        Ok(self.pointer + size < self.window.len())
    }

    fn update(&mut self) -> Result<()> {
        let remaining = self.src.len() - self.src_pos;
        let mut read = remaining.min(1023);
        if read == 0 {
            self.eof = true;
            return Ok(());
        }
        let last = self.src[self.src_pos + read - 1];
        if (0xD800..0xDC00).contains(&last) {
            if self.src_pos + read >= self.src.len() {
                return Err(YamlError {
                    class: READER_EXCEPTION,
                    kind: YamlErrorKind::Reader {
                        name: self.name.to_string(),
                        position: self.index + read,
                        code_point: last as i32,
                        message: "The last char is HighSurrogate (no LowSurrogate detected).".into(),
                    },
                });
            }
            read += 1;
        }
        let chunk = &self.src[self.src_pos..self.src_pos + read];
        self.src_pos += read;
        let mut data: Vec<i32> = self.window[self.pointer..].to_vec();
        let mut non_printable: Option<i32> = None;
        let mut i = 0;
        while i < chunk.len() {
            let u = chunk[i];
            let (cp, n) = if (0xD800..0xDC00).contains(&u) && i + 1 < chunk.len() && (0xDC00..0xE000).contains(&chunk[i + 1]) {
                (0x10000 + (((u as i32) - 0xD800) << 10) + ((chunk[i + 1] as i32) - 0xDC00), 2)
            } else {
                (u as i32, 1)
            };
            data.push(cp);
            if is_printable(cp) {
                i += n;
            } else {
                non_printable = Some(cp);
                break;
            }
        }
        self.window = Arc::new(data);
        self.pointer = 0;
        if let Some(cp) = non_printable {
            return Err(YamlError {
                class: READER_EXCEPTION,
                kind: YamlErrorKind::Reader {
                    name: self.name.to_string(),
                    position: self.index + self.window.len() - 1,
                    code_point: cp,
                    message: "special characters are not allowed".into(),
                },
            });
        }
        Ok(())
    }

    /// `getColumn()`: the 0-based column of the current position.
    pub fn get_column(&self) -> usize {
        self.column
    }

    /// `getLine()`: the 0-based line of the current position.
    pub fn get_line(&self) -> usize {
        self.line
    }

    /// `getIndex()`: the number of code points consumed since the start of the stream.
    pub fn get_index(&self) -> usize {
        self.index
    }

    /// `getDocumentIndex()`: the number of code points consumed since the current document started.
    pub fn get_document_index(&self) -> usize {
        self.document_index
    }

    /// `resetDocumentIndex()`: restarts the per-document count at a document boundary.
    pub fn reset_document_index(&mut self) {
        self.document_index = 0;
    }
}
