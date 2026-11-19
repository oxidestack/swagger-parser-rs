//! `org.yaml.snakeyaml.error`: marks and the exception hierarchy with SnakeYAML's exact message formatting.

use std::sync::Arc;

/// `Mark`: a position plus the reader's data window at the time (used to render the snippet).
#[derive(Clone, Debug)]
pub struct Mark {
    /// `getName()`: the source name, e.g. `'string'` or `'reader'`.
    pub name: Arc<str>,
    /// `getIndex()`: the code-point index from the start of the stream.
    pub index: usize,
    /// `getLine()`: the 0-based line.
    pub line: usize,
    /// `getColumn()`: the 0-based column.
    pub column: usize,
    /// `getBuffer()`: the reader's code-point window the snippet is drawn from.
    pub buffer: Arc<Vec<i32>>,
    /// `getPointer()`: the position of the mark within `buffer`.
    pub pointer: usize,
}

fn is_null_or_linebr(c: i32) -> bool {
    matches!(c, 0 | 13 | 10 | 0x85 | 0x2028 | 0x2029)
}

impl Mark {
    /// `get_snippet(indent, max_length)`
    pub fn get_snippet_with(&self, indent: usize, max_length: usize) -> String {
        let half = max_length as f32 / 2.0 - 1.0;
        let mut start = self.pointer;
        let mut head = "";
        while start > 0 && !is_null_or_linebr(self.buffer[start - 1]) {
            start -= 1;
            if (self.pointer - start) as f32 > half {
                head = " ... ";
                start += 5;
                break;
            }
        }
        let mut tail = "";
        let mut end = self.pointer;
        while end < self.buffer.len() && !is_null_or_linebr(self.buffer[end]) {
            end += 1;
            if (end - self.pointer) as f32 > half {
                tail = " ... ";
                end -= 5;
                break;
            }
        }
        let mut result = String::new();
        for _ in 0..indent {
            result.push(' ');
        }
        result.push_str(head);
        for i in start..end {
            push_code_point(&mut result, self.buffer[i]);
        }
        result.push_str(tail);
        result.push('\n');
        for _ in 0..(indent + self.pointer - start + head.len()) {
            result.push(' ');
        }
        result.push('^');
        result
    }

    /// `get_snippet()`: the line around the mark, indented by 4 and at most 75 wide, with a `^` under the mark.
    pub fn get_snippet(&self) -> String {
        self.get_snippet_with(4, 75)
    }
}

impl std::fmt::Display for Mark {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, " in {}, line {}, column {}:\n{}", self.name, self.line + 1, self.column + 1, self.get_snippet())
    }
}

pub(crate) fn push_code_point(s: &mut String, cp: i32) {
    match char::from_u32(cp as u32) {
        Some(c) => s.push(c),
        None => s.push('\u{FFFD}'),
    }
}

/// A `MarkedYAMLException`'s parts.
#[derive(Clone, Debug)]
pub struct Marked {
    /// `getContext()`: what was being read, e.g. `while scanning a simple key`.
    pub context: Option<String>,
    /// `getContextMark()`: where that context started.
    pub context_mark: Option<Mark>,
    /// `getProblem()`: what went wrong.
    pub problem: Option<String>,
    /// `getProblemMark()`: where it went wrong.
    pub problem_mark: Option<Mark>,
    /// An extra note appended to the message.
    pub note: Option<String>,
}

impl Marked {
    /// `MarkedYAMLException.toString()` (= `getMessage()`).
    pub fn render(&self) -> String {
        let mut lines = String::new();
        if let Some(c) = &self.context {
            lines.push_str(c);
            lines.push('\n');
        }
        if let Some(cm) = &self.context_mark {
            let show = match (&self.problem, &self.problem_mark) {
                (Some(_), Some(pm)) => cm.name == pm.name || cm.line != pm.line || cm.column != pm.column,
                _ => true,
            };
            if show {
                lines.push_str(&cm.to_string());
                lines.push('\n');
            }
        }
        if let Some(p) = &self.problem {
            lines.push_str(p);
            lines.push('\n');
        }
        if let Some(pm) = &self.problem_mark {
            lines.push_str(&pm.to_string());
            lines.push('\n');
        }
        if let Some(n) = &self.note {
            lines.push_str(n);
            lines.push('\n');
        }
        lines
    }
}

/// The kind of [`YamlError`] and the data its message is built from.
#[derive(Clone, Debug)]
pub enum YamlErrorKind {
    /// `YAMLException(message)`
    Plain(String),
    /// A `MarkedYAMLException` subclass (scanner, parser, composer, constructor, duplicate key), boxed: its two marks
    /// would otherwise make every `Result` carrying the error that much larger.
    Marked(Box<Marked>),
    /// `ReaderException`
    Reader {
        /// The source name, e.g. `'string'` or `'reader'`.
        name: String,
        /// The code-point index of the offending character.
        position: usize,
        /// The offending code point.
        code_point: i32,
        /// What is wrong with it.
        message: String,
    },
}

/// A SnakeYAML exception: its Java class and its content.
#[derive(Clone, Debug)]
pub struct YamlError {
    /// Fully-qualified Java class, e.g. `org.yaml.snakeyaml.scanner.ScannerException`.
    pub class: &'static str,
    /// The exception's content.
    pub kind: YamlErrorKind,
}

/// `ScannerException`.
pub const SCANNER_EXCEPTION: &str = "org.yaml.snakeyaml.scanner.ScannerException";
/// `ParserException`.
pub const PARSER_EXCEPTION: &str = "org.yaml.snakeyaml.parser.ParserException";
/// `ComposerException`.
pub const COMPOSER_EXCEPTION: &str = "org.yaml.snakeyaml.composer.ComposerException";
/// `ConstructorException`.
pub const CONSTRUCTOR_EXCEPTION: &str = "org.yaml.snakeyaml.constructor.ConstructorException";
/// `DuplicateKeyException`.
pub const DUPLICATE_KEY_EXCEPTION: &str = "org.yaml.snakeyaml.constructor.DuplicateKeyException";
/// `YAMLException`.
pub const YAML_EXCEPTION: &str = "org.yaml.snakeyaml.error.YAMLException";
/// `ReaderException`.
pub const READER_EXCEPTION: &str = "org.yaml.snakeyaml.reader.ReaderException";

impl YamlError {
    /// `new YAMLException(message)`.
    pub fn plain(msg: impl Into<String>) -> YamlError {
        YamlError { class: YAML_EXCEPTION, kind: YamlErrorKind::Plain(msg.into()) }
    }

    /// A `MarkedYAMLException` of the given class: `problem` at `problem_mark`, found while reading `context` that started at `context_mark`.
    pub fn marked(class: &'static str, context: Option<&str>, context_mark: Option<Mark>, problem: &str, problem_mark: Option<Mark>) -> YamlError {
        YamlError {
            class,
            kind: YamlErrorKind::Marked(Box::new(Marked {
                context: context.map(str::to_string),
                context_mark,
                problem: Some(problem.to_string()),
                problem_mark,
                note: None,
            })),
        }
    }

    /// `new ScannerException(context, contextMark, problem, problemMark)`.
    pub fn scanner(context: Option<&str>, context_mark: Option<Mark>, problem: &str, problem_mark: Mark) -> YamlError {
        Self::marked(SCANNER_EXCEPTION, context, context_mark, problem, Some(problem_mark))
    }

    /// `new ParserException(context, contextMark, problem, problemMark)`.
    pub fn parser(context: Option<&str>, context_mark: Option<Mark>, problem: &str, problem_mark: Mark) -> YamlError {
        Self::marked(PARSER_EXCEPTION, context, context_mark, problem, Some(problem_mark))
    }

    /// `getMessage()`
    pub fn message(&self) -> String {
        match &self.kind {
            YamlErrorKind::Plain(m) => m.clone(),
            YamlErrorKind::Marked(m) => m.render(),
            YamlErrorKind::Reader { message, .. } => message.clone(),
        }
    }

    /// `getProblem()` of a marked exception.
    pub fn problem(&self) -> Option<&str> {
        match &self.kind {
            YamlErrorKind::Marked(m) => m.problem.as_deref(),
            _ => None,
        }
    }

    /// `toString()`
    pub fn to_java_string(&self) -> String {
        match &self.kind {
            YamlErrorKind::Plain(m) => format!("{}: {m}", self.class),
            YamlErrorKind::Marked(m) => m.render(),
            YamlErrorKind::Reader { name, position, code_point, message } => {
                let mut s = String::new();
                push_code_point(&mut s, *code_point);
                format!("unacceptable code point '{s}' (0x{:X}) {message}\nin \"{name}\", position {position}", code_point)
            }
        }
    }
}

impl std::fmt::Display for YamlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message())
    }
}

impl std::error::Error for YamlError {}

/// The result of a SnakeYAML operation that may throw a [`YamlError`].
pub type Result<T> = std::result::Result<T, YamlError>;
