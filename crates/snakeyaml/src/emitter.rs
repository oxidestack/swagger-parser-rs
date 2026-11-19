//! `org.yaml.snakeyaml.emitter.Emitter` (comments are never emitted: `processComments` is off), writing to a
//! `String`. Text is handled as UTF-16 code units, as Java does, so columns and line splitting match.

use crate::events::{Event, EventKind, ImplicitTuple};
use crate::options::{FlowStyle, ScalarStyle, Version};
use std::collections::VecDeque;

/// `DumperOptions` (the settings the emitter reads)
#[derive(Clone, Debug)]
pub struct DumperOptions {
    /// `setCanonical`: write the canonical form (explicit tags, flow collections, double-quoted scalars).
    pub canonical: bool,
    /// `setPrettyFlow`: break flow collections across lines.
    pub pretty_flow: bool,
    /// `setAllowUnicode`: write non-ASCII characters as they are rather than escaped.
    pub allow_unicode: bool,
    /// `setIndent`: the block indentation step; values outside `2..=9` fall back to 2.
    pub indent: i32,
    /// `setIndicatorIndent`: how far a block sequence's `-` is indented under its parent.
    pub indicator_indent: i32,
    /// `setIndentWithIndicator`: count the `-` indicator as part of the item indentation.
    pub indent_with_indicator: bool,
    /// `setWidth`: the preferred line width; a value not above twice the indent falls back to 80.
    pub width: i32,
    /// `setSplitLines`: fold long scalars across lines at the width.
    pub split_lines: bool,
    /// `setLineBreak`: the line break written between lines.
    pub line_break: String,
    /// `setMaxSimpleKeyLength`: the longest key written as an implicit `key:`; longer keys use `? key`.
    pub max_simple_key_length: i32,
    /// `setDefaultFlowStyle`: the collection style when an event does not force one.
    pub default_flow_style: FlowStyle,
    /// `setVersion`: the `%YAML` directive to write, `None` for none.
    pub version: Option<Version>,
}

impl Default for DumperOptions {
    fn default() -> Self {
        DumperOptions {
            canonical: false,
            pretty_flow: false,
            allow_unicode: true,
            indent: 2,
            indicator_indent: 0,
            indent_with_indicator: false,
            width: 80,
            split_lines: true,
            line_break: "\n".into(),
            max_simple_key_length: 128,
            default_flow_style: FlowStyle::Auto,
            version: None,
        }
    }
}

/// `EmitterException`
#[derive(Clone, Debug)]
pub struct EmitterError(pub String);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    StreamStart,
    Nothing,
    FirstDocumentStart,
    DocumentStart,
    DocumentEnd,
    DocumentRoot,
    FirstFlowSequenceItem,
    FlowSequenceItem,
    FirstFlowMappingKey,
    FlowMappingKey,
    FlowMappingSimpleValue,
    FlowMappingValue,
    FirstBlockSequenceItem,
    BlockSequenceItem,
    FirstBlockMappingKey,
    BlockMappingKey,
    BlockMappingSimpleValue,
    BlockMappingValue,
}

/// `ScalarAnalysis`
#[derive(Clone, Debug)]
struct ScalarAnalysis {
    scalar: Vec<u16>,
    empty: bool,
    multiline: bool,
    allow_flow_plain: bool,
    allow_block_plain: bool,
    allow_single_quoted: bool,
    allow_block: bool,
}

const LINEBR: &[u16] = &[0x0a, 0x85, 0x2028, 0x2029];

fn is_linebr(c: u32) -> bool {
    LINEBR.iter().any(|&x| x as u32 == c)
}

fn is_null_bl_t(c: u32) -> bool {
    c == 0 || c == 0x20 || c == 0x09
}

fn is_null_bl_t_linebr(c: u32) -> bool {
    matches!(c, 0x09 | 0x20 | 0x00 | 0x0d | 0x0a | 0x85 | 0x2028 | 0x2029)
}

fn code_point_at(s: &[u16], i: usize) -> u32 {
    let c = s[i] as u32;
    if (0xd800..0xdc00).contains(&c) && i + 1 < s.len() {
        let d = s[i + 1] as u32;
        if (0xdc00..0xe000).contains(&d) {
            return 0x10000 + ((c - 0xd800) << 10) + (d - 0xdc00);
        }
    }
    c
}

fn char_count(cp: u32) -> usize {
    if cp >= 0x10000 { 2 } else { 1 }
}

fn escape_replacement(ch: u16) -> Option<&'static str> {
    Some(match ch {
        0x00 => "0",
        0x07 => "a",
        0x08 => "b",
        0x09 => "t",
        0x0a => "n",
        0x0b => "v",
        0x0c => "f",
        0x0d => "r",
        0x1b => "e",
        0x22 => "\"",
        0x5c => "\\",
        0x85 => "N",
        0xa0 => "_",
        0x2028 => "L",
        0x2029 => "P",
        _ => return None,
    })
}

fn is_printable(c: u32) -> bool {
    (32..=126).contains(&c)
        || c == 9
        || c == 10
        || c == 13
        || c == 133
        || (160..=55295).contains(&c)
        || (57344..=65533).contains(&c)
        || (65536..=1114111).contains(&c)
}

/// `org.yaml.snakeyaml.emitter.Emitter`: turns a stream of [`Event`]s into YAML text.
pub struct Emitter {
    out: Vec<u16>,
    states: Vec<State>,
    state: State,
    events: VecDeque<Event>,
    event: Option<Event>,
    indents: Vec<Option<i32>>,
    indent: Option<i32>,
    flow_level: i32,
    root_context: bool,
    mapping_context: bool,
    simple_key_context: bool,
    column: i32,
    whitespace: bool,
    indention: bool,
    open_ended: bool,
    canonical: bool,
    pretty_flow: bool,
    allow_unicode: bool,
    best_indent: i32,
    indicator_indent: i32,
    indent_with_indicator: bool,
    best_width: i32,
    best_line_break: Vec<u16>,
    split_lines: bool,
    max_simple_key_length: i32,
    tag_prefixes: Vec<(String, String)>,
    prepared_anchor: Option<String>,
    prepared_tag: Option<String>,
    analysis: Option<ScalarAnalysis>,
    style: Option<ScalarStyle>,
}

type R = Result<(), EmitterError>;

impl Emitter {
    /// An emitter writing with the given options.
    pub fn new(opts: &DumperOptions) -> Emitter {
        let mut best_indent = 2;
        if opts.indent > 1 && opts.indent < 10 {
            best_indent = opts.indent;
        }
        let mut best_width = 80;
        if opts.width > best_indent * 2 {
            best_width = opts.width;
        }
        Emitter {
            out: Vec::new(),
            states: Vec::new(),
            state: State::StreamStart,
            events: VecDeque::new(),
            event: None,
            indents: Vec::new(),
            indent: None,
            flow_level: 0,
            root_context: false,
            mapping_context: false,
            simple_key_context: false,
            column: 0,
            whitespace: true,
            indention: true,
            open_ended: false,
            canonical: opts.canonical,
            pretty_flow: opts.pretty_flow,
            allow_unicode: opts.allow_unicode,
            best_indent,
            indicator_indent: opts.indicator_indent,
            indent_with_indicator: opts.indent_with_indicator,
            best_width,
            best_line_break: opts.line_break.encode_utf16().collect(),
            split_lines: opts.split_lines,
            max_simple_key_length: opts.max_simple_key_length,
            tag_prefixes: Vec::new(),
            prepared_anchor: None,
            prepared_tag: None,
            analysis: None,
            style: None,
        }
    }

    /// The text written so far
    pub fn output(&self) -> String {
        String::from_utf16_lossy(&self.out)
    }

    /// `emit(event)`
    pub fn emit(&mut self, event: Event) -> R {
        self.events.push_back(event);
        while !self.need_more_events() {
            self.event = self.events.pop_front();
            self.expect()?;
            self.event = None;
        }
        Ok(())
    }

    fn need_more_events(&self) -> bool {
        let Some(first) = self.events.front() else { return true };
        match first.kind {
            EventKind::DocumentStart { .. } => self.need_events(1),
            EventKind::SequenceStart { .. } => self.need_events(2),
            EventKind::MappingStart { .. } => self.need_events(3),
            EventKind::StreamStart => self.need_events(2),
            _ => false,
        }
    }

    fn need_events(&self, count: usize) -> bool {
        let mut level = 0i32;
        let mut actual = 0usize;
        for e in self.events.iter().skip(1) {
            actual += 1;
            match e.kind {
                EventKind::DocumentStart { .. } | EventKind::SequenceStart { .. } | EventKind::MappingStart { .. } => level += 1,
                EventKind::DocumentEnd { .. } | EventKind::SequenceEnd | EventKind::MappingEnd => level -= 1,
                EventKind::StreamEnd => level = -1,
                _ => {}
            }
            if level < 0 {
                return false;
            }
        }
        actual < count
    }

    fn ev(&self) -> &Event {
        self.event.as_ref().unwrap()
    }

    fn describe(&self) -> String {
        format!("{:?}", self.event.as_ref().map(|e| &e.kind))
    }

    fn increase_indent(&mut self, flow: bool, indentless: bool) {
        self.indents.push(self.indent);
        match self.indent {
            None => self.indent = Some(if flow { self.best_indent } else { 0 }),
            Some(i) => {
                if !indentless {
                    self.indent = Some(i + self.best_indent);
                }
            }
        }
    }

    fn pop_indent(&mut self) {
        self.indent = self.indents.pop().flatten();
    }

    fn pop_state(&mut self) {
        self.state = self.states.pop().unwrap_or(State::Nothing);
    }

    fn expect(&mut self) -> R {
        match self.state {
            State::StreamStart => {
                if matches!(self.ev().kind, EventKind::StreamStart) {
                    self.state = State::FirstDocumentStart;
                    Ok(())
                } else {
                    Err(EmitterError(format!("expected StreamStartEvent, but got {}", self.describe())))
                }
            }
            State::Nothing => Err(EmitterError(format!("expecting nothing, but got {}", self.describe()))),
            State::FirstDocumentStart => self.expect_document_start(true),
            State::DocumentStart => self.expect_document_start(false),
            State::DocumentEnd => {
                if let EventKind::DocumentEnd { explicit } = self.ev().kind {
                    self.write_indent();
                    if explicit {
                        self.write_indicator("...", true, false, false);
                        self.write_indent();
                    }
                    self.state = State::DocumentStart;
                    Ok(())
                } else {
                    Err(EmitterError(format!("expected DocumentEndEvent, but got {}", self.describe())))
                }
            }
            State::DocumentRoot => {
                self.states.push(State::DocumentEnd);
                self.expect_node(true, false, false)
            }
            State::FirstFlowSequenceItem => {
                if matches!(self.ev().kind, EventKind::SequenceEnd) {
                    self.pop_indent();
                    self.flow_level -= 1;
                    self.write_indicator("]", false, false, false);
                    self.pop_state();
                    Ok(())
                } else {
                    if self.canonical || self.column > self.best_width && self.split_lines || self.pretty_flow {
                        self.write_indent();
                    }
                    self.states.push(State::FlowSequenceItem);
                    self.expect_node(false, false, false)
                }
            }
            State::FlowSequenceItem => {
                if matches!(self.ev().kind, EventKind::SequenceEnd) {
                    self.pop_indent();
                    self.flow_level -= 1;
                    if self.canonical {
                        self.write_indicator(",", false, false, false);
                        self.write_indent();
                    } else if self.pretty_flow {
                        self.write_indent();
                    }
                    self.write_indicator("]", false, false, false);
                    if self.pretty_flow {
                        self.write_indent();
                    }
                    self.pop_state();
                    Ok(())
                } else {
                    self.write_indicator(",", false, false, false);
                    if self.canonical || self.column > self.best_width && self.split_lines || self.pretty_flow {
                        self.write_indent();
                    }
                    self.states.push(State::FlowSequenceItem);
                    self.expect_node(false, false, false)
                }
            }
            State::FirstFlowMappingKey => {
                if matches!(self.ev().kind, EventKind::MappingEnd) {
                    self.pop_indent();
                    self.flow_level -= 1;
                    self.write_indicator("}", false, false, false);
                    self.pop_state();
                    Ok(())
                } else {
                    if self.canonical || self.column > self.best_width && self.split_lines || self.pretty_flow {
                        self.write_indent();
                    }
                    if !self.canonical && self.check_simple_key()? {
                        self.states.push(State::FlowMappingSimpleValue);
                        self.expect_node(false, true, true)
                    } else {
                        self.write_indicator("?", true, false, false);
                        self.states.push(State::FlowMappingValue);
                        self.expect_node(false, true, false)
                    }
                }
            }
            State::FlowMappingKey => {
                if matches!(self.ev().kind, EventKind::MappingEnd) {
                    self.pop_indent();
                    self.flow_level -= 1;
                    if self.canonical {
                        self.write_indicator(",", false, false, false);
                        self.write_indent();
                    }
                    if self.pretty_flow {
                        self.write_indent();
                    }
                    self.write_indicator("}", false, false, false);
                    self.pop_state();
                    Ok(())
                } else {
                    self.write_indicator(",", false, false, false);
                    if self.canonical || self.column > self.best_width && self.split_lines || self.pretty_flow {
                        self.write_indent();
                    }
                    if !self.canonical && self.check_simple_key()? {
                        self.states.push(State::FlowMappingSimpleValue);
                        self.expect_node(false, true, true)
                    } else {
                        self.write_indicator("?", true, false, false);
                        self.states.push(State::FlowMappingValue);
                        self.expect_node(false, true, false)
                    }
                }
            }
            State::FlowMappingSimpleValue => {
                self.write_indicator(":", false, false, false);
                self.states.push(State::FlowMappingKey);
                self.expect_node(false, true, false)
            }
            State::FlowMappingValue => {
                if self.canonical || self.column > self.best_width || self.pretty_flow {
                    self.write_indent();
                }
                self.write_indicator(":", true, false, false);
                self.states.push(State::FlowMappingKey);
                self.expect_node(false, true, false)
            }
            State::FirstBlockSequenceItem => self.expect_block_sequence_item(true),
            State::BlockSequenceItem => self.expect_block_sequence_item(false),
            State::FirstBlockMappingKey => self.expect_block_mapping_key(true),
            State::BlockMappingKey => self.expect_block_mapping_key(false),
            State::BlockMappingSimpleValue => {
                self.write_indicator(":", false, false, false);
                self.states.push(State::BlockMappingKey);
                self.expect_node(false, true, false)
            }
            State::BlockMappingValue => {
                self.write_indent();
                self.write_indicator(":", true, false, true);
                self.states.push(State::BlockMappingKey);
                self.expect_node(false, true, false)
            }
        }
    }

    fn expect_document_start(&mut self, first: bool) -> R {
        match &self.ev().kind {
            EventKind::DocumentStart { explicit, version, tags } => {
                let (explicit, version, tags) = (*explicit, *version, tags.clone());
                if (version.is_some() || tags.is_some()) && self.open_ended {
                    self.write_indicator("...", true, false, false);
                    self.write_indent();
                }
                if let Some(v) = version {
                    let text = match v {
                        Version::V1_0 => "1.0",
                        Version::V1_1 => "1.1",
                    };
                    self.write_str("%YAML ");
                    self.write_str(text);
                    self.write_line_break(None);
                }
                self.tag_prefixes = vec![("!".into(), "!".into()), ("tag:yaml.org,2002:".into(), "!!".into())];
                if let Some(tags) = &tags {
                    let mut handles: Vec<&(String, String)> = tags.iter().collect();
                    handles.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
                    for (handle, prefix) in handles {
                        self.tag_prefixes.push((prefix.clone(), handle.clone()));
                        self.write_str("%TAG ");
                        self.write_str(handle);
                        self.write_str(" ");
                        self.write_str(prefix);
                        self.write_line_break(None);
                    }
                }
                let implicit = first && !explicit && !self.canonical && version.is_none() && tags.as_ref().is_none_or(|t| t.is_empty()) && !self.check_empty_document();
                if !implicit {
                    self.write_indent();
                    self.write_indicator("---", true, false, false);
                    if self.canonical {
                        self.write_indent();
                    }
                }
                self.state = State::DocumentRoot;
                Ok(())
            }
            EventKind::StreamEnd => {
                self.state = State::Nothing;
                Ok(())
            }
            _ => Err(EmitterError(format!("expected DocumentStartEvent, but got {}", self.describe()))),
        }
    }

    fn expect_block_sequence_item(&mut self, first: bool) -> R {
        if !first && matches!(self.ev().kind, EventKind::SequenceEnd) {
            self.pop_indent();
            self.pop_state();
            return Ok(());
        }
        self.write_indent();
        if !self.indent_with_indicator || first {
            self.write_whitespace(self.indicator_indent);
        }
        self.write_indicator("-", true, false, true);
        if self.indent_with_indicator && first {
            self.indent = Some(self.indent.unwrap_or(0) + self.indicator_indent);
        }
        self.states.push(State::BlockSequenceItem);
        self.expect_node(false, false, false)
    }

    fn expect_block_mapping_key(&mut self, first: bool) -> R {
        if !first && matches!(self.ev().kind, EventKind::MappingEnd) {
            self.pop_indent();
            self.pop_state();
            return Ok(());
        }
        self.write_indent();
        if self.check_simple_key()? {
            self.states.push(State::BlockMappingSimpleValue);
            self.expect_node(false, true, true)
        } else {
            self.write_indicator("?", true, false, true);
            self.states.push(State::BlockMappingValue);
            self.expect_node(false, true, false)
        }
    }

    fn expect_node(&mut self, root: bool, mapping: bool, simple_key: bool) -> R {
        self.root_context = root;
        self.mapping_context = mapping;
        self.simple_key_context = simple_key;
        match &self.ev().kind {
            EventKind::Alias { .. } => {
                self.process_anchor("*")?;
                self.pop_state();
                Ok(())
            }
            EventKind::Scalar { .. } => {
                self.process_anchor("&")?;
                self.process_tag()?;
                self.increase_indent(true, false);
                self.process_scalar()?;
                self.pop_indent();
                self.pop_state();
                Ok(())
            }
            EventKind::SequenceStart { flow_style, .. } => {
                let flow = *flow_style == FlowStyle::Flow;
                self.process_anchor("&")?;
                self.process_tag()?;
                if self.flow_level == 0 && !self.canonical && !flow && !self.check_empty_sequence() {
                    let indentless = self.mapping_context && !self.indention;
                    self.increase_indent(false, indentless);
                    self.state = State::FirstBlockSequenceItem;
                } else {
                    self.write_indicator("[", true, true, false);
                    self.flow_level += 1;
                    self.increase_indent(true, false);
                    if self.pretty_flow {
                        self.write_indent();
                    }
                    self.state = State::FirstFlowSequenceItem;
                }
                Ok(())
            }
            EventKind::MappingStart { flow_style, .. } => {
                let flow = *flow_style == FlowStyle::Flow;
                self.process_anchor("&")?;
                self.process_tag()?;
                if self.flow_level == 0 && !self.canonical && !flow && !self.check_empty_mapping() {
                    self.increase_indent(false, false);
                    self.state = State::FirstBlockMappingKey;
                } else {
                    self.write_indicator("{", true, true, false);
                    self.flow_level += 1;
                    self.increase_indent(true, false);
                    if self.pretty_flow {
                        self.write_indent();
                    }
                    self.state = State::FirstFlowMappingKey;
                }
                Ok(())
            }
            _ => Err(EmitterError(format!("expected NodeEvent, but got {}", self.describe()))),
        }
    }

    fn check_empty_sequence(&self) -> bool {
        matches!(self.ev().kind, EventKind::SequenceStart { .. }) && self.events.front().is_some_and(|e| matches!(e.kind, EventKind::SequenceEnd))
    }

    fn check_empty_mapping(&self) -> bool {
        matches!(self.ev().kind, EventKind::MappingStart { .. }) && self.events.front().is_some_and(|e| matches!(e.kind, EventKind::MappingEnd))
    }

    fn check_empty_document(&self) -> bool {
        if !matches!(self.ev().kind, EventKind::DocumentStart { .. }) {
            return false;
        }
        match self.events.front().map(|e| &e.kind) {
            Some(EventKind::Scalar { anchor, tag, value, .. }) => anchor.is_none() && tag.is_none() && value.is_empty(),
            _ => false,
        }
    }

    fn event_anchor(&self) -> Option<String> {
        match &self.ev().kind {
            EventKind::Scalar { anchor, .. } | EventKind::SequenceStart { anchor, .. } | EventKind::MappingStart { anchor, .. } => anchor.clone(),
            EventKind::Alias { anchor } => Some(anchor.clone()),
            _ => None,
        }
    }

    fn check_simple_key(&mut self) -> Result<bool, EmitterError> {
        let mut length = 0i32;
        if let Some(a) = self.event_anchor() {
            if self.prepared_anchor.is_none() {
                self.prepared_anchor = Some(prepare_anchor(&a)?);
            }
            length += self.prepared_anchor.as_ref().unwrap().encode_utf16().count() as i32;
        }
        let tag = match &self.ev().kind {
            EventKind::Scalar { tag, .. } | EventKind::SequenceStart { tag, .. } | EventKind::MappingStart { tag, .. } => tag.clone(),
            _ => None,
        };
        if let Some(t) = tag {
            if self.prepared_tag.is_none() {
                self.prepared_tag = Some(self.prepare_tag(&t)?);
            }
            length += self.prepared_tag.as_ref().unwrap().encode_utf16().count() as i32;
        }
        let mut is_scalar_ok = false;
        if let EventKind::Scalar { value, .. } = &self.ev().kind {
            if self.analysis.is_none() {
                self.analysis = Some(self.analyze_scalar(value));
            }
            let a = self.analysis.as_ref().unwrap();
            length += a.scalar.len() as i32;
            is_scalar_ok = !a.empty && !a.multiline;
        }
        let is_alias = matches!(self.ev().kind, EventKind::Alias { .. });
        Ok(length < self.max_simple_key_length && (is_alias || is_scalar_ok || self.check_empty_sequence() || self.check_empty_mapping()))
    }

    fn process_anchor(&mut self, indicator: &str) -> R {
        match self.event_anchor() {
            None => {
                self.prepared_anchor = None;
                Ok(())
            }
            Some(a) => {
                if self.prepared_anchor.is_none() {
                    self.prepared_anchor = Some(prepare_anchor(&a)?);
                }
                let text = format!("{indicator}{}", self.prepared_anchor.take().unwrap());
                self.write_indicator(&text, true, false, false);
                Ok(())
            }
        }
    }

    fn process_tag(&mut self) -> R {
        let mut tag: Option<String>;
        if let EventKind::Scalar { tag: t, implicit, .. } = &self.ev().kind {
            let (t, implicit) = (t.clone(), *implicit);
            tag = t;
            if self.style.is_none() {
                self.style = Some(self.choose_scalar_style());
            }
            let style = self.style.unwrap();
            if (!self.canonical || tag.is_none())
                && (style == ScalarStyle::Plain && implicit.can_omit_tag_in_plain_scalar() || style != ScalarStyle::Plain && implicit.can_omit_tag_in_non_plain_scalar())
            {
                self.prepared_tag = None;
                return Ok(());
            }
            if implicit.can_omit_tag_in_plain_scalar() && tag.is_none() {
                tag = Some("!".into());
                self.prepared_tag = None;
            }
        } else {
            let (t, implicit) = match &self.ev().kind {
                EventKind::SequenceStart { tag, implicit, .. } | EventKind::MappingStart { tag, implicit, .. } => (tag.clone(), *implicit),
                _ => (None, true),
            };
            tag = t;
            if (!self.canonical || tag.is_none()) && implicit {
                self.prepared_tag = None;
                return Ok(());
            }
        }
        let Some(tag) = tag else { return Err(EmitterError("tag is not specified".into())) };
        if self.prepared_tag.is_none() {
            self.prepared_tag = Some(self.prepare_tag(&tag)?);
        }
        let t = self.prepared_tag.take().unwrap();
        self.write_indicator(&t, true, false, false);
        Ok(())
    }

    fn choose_scalar_style(&mut self) -> ScalarStyle {
        let EventKind::Scalar { value, style, implicit, .. } = &self.ev().kind else { return ScalarStyle::Plain };
        let (value, ev_style, implicit): (String, ScalarStyle, ImplicitTuple) = (value.clone(), *style, *implicit);
        if self.analysis.is_none() {
            self.analysis = Some(self.analyze_scalar(&value));
        }
        let a = self.analysis.as_ref().unwrap();
        let is_plain = ev_style == ScalarStyle::Plain;
        let is_dquoted = ev_style == ScalarStyle::DoubleQuoted;
        let is_json = ev_style == ScalarStyle::JsonScalarStyle;
        if (is_plain || !is_dquoted) && !self.canonical {
            if is_json {
                // Tag.STR equals the tag: never the case for Jackson-emitted scalars
            }
            if !is_plain && !is_json
                || !implicit.can_omit_tag_in_plain_scalar()
                || self.simple_key_context && (a.empty || a.multiline)
                || (self.flow_level == 0 || !a.allow_flow_plain) && (self.flow_level != 0 || !a.allow_block_plain)
            {
                if (ev_style == ScalarStyle::Literal || ev_style == ScalarStyle::Folded) && self.flow_level == 0 && !self.simple_key_context && a.allow_block {
                    ev_style
                } else if !is_plain && ev_style != ScalarStyle::SingleQuoted || !a.allow_single_quoted || self.simple_key_context && a.multiline {
                    ScalarStyle::DoubleQuoted
                } else {
                    ScalarStyle::SingleQuoted
                }
            } else {
                ScalarStyle::Plain
            }
        } else {
            ScalarStyle::DoubleQuoted
        }
    }

    fn process_scalar(&mut self) -> R {
        let EventKind::Scalar { value, .. } = &self.ev().kind else { return Ok(()) };
        let value = value.clone();
        if self.analysis.is_none() {
            self.analysis = Some(self.analyze_scalar(&value));
        }
        let split = !self.simple_key_context && self.split_lines;
        let text = self.analysis.as_ref().unwrap().scalar.clone();
        match self.style {
            Some(ScalarStyle::Plain) => self.write_plain(&text, split),
            Some(ScalarStyle::DoubleQuoted) => self.write_double_quoted(&text, split),
            Some(ScalarStyle::SingleQuoted) => self.write_single_quoted(&text, split),
            Some(ScalarStyle::Folded) => self.write_folded(&text, split),
            Some(ScalarStyle::Literal) => self.write_literal(&text),
            other => return Err(EmitterError(format!("Unexpected style: {other:?}"))),
        }
        self.analysis = None;
        self.style = None;
        Ok(())
    }

    fn prepare_tag(&self, tag: &str) -> Result<String, EmitterError> {
        if tag.is_empty() {
            return Err(EmitterError("tag must not be empty".into()));
        }
        if tag == "!" {
            return Ok(tag.to_string());
        }
        let mut handle: Option<String> = None;
        let mut suffix = tag.to_string();
        for (prefix, _) in &self.tag_prefixes {
            if tag.starts_with(prefix.as_str()) && (prefix == "!" || prefix.len() < tag.len()) {
                handle = Some(prefix.clone());
            }
        }
        if let Some(h) = &handle {
            suffix = tag[h.len()..].to_string();
            handle = self.tag_prefixes.iter().rev().find(|(p, _)| p == h).map(|(_, v)| v.clone());
        }
        Ok(match handle {
            Some(h) => format!("{h}{suffix}"),
            None => format!("!<{suffix}>"),
        })
    }

    fn analyze_scalar(&self, scalar: &str) -> ScalarAnalysis {
        let s: Vec<u16> = scalar.encode_utf16().collect();
        if s.is_empty() {
            return ScalarAnalysis { scalar: s, empty: true, multiline: false, allow_flow_plain: false, allow_block_plain: true, allow_single_quoted: true, allow_block: false };
        }
        let mut block_indicators = false;
        let mut flow_indicators = false;
        let mut line_breaks = false;
        let mut special_characters = false;
        let leading_zero_number = s.len() > 1 && s[0] == b'0' as u16 && s[1..].iter().all(|&c| (b'0' as u16..=b'9' as u16).contains(&c) || c == b'_' as u16);
        let (mut leading_space, mut leading_break, mut trailing_space, mut trailing_break, mut break_space, mut space_break) = (false, false, false, false, false, false);
        if scalar.starts_with("---") || scalar.starts_with("...") {
            block_indicators = true;
            flow_indicators = true;
        }
        let mut preceded_by_whitespace = true;
        let mut followed_by_whitespace = s.len() == 1 || is_null_bl_t_linebr(code_point_at(&s, 1));
        let mut previous_space = false;
        let mut previous_break = false;
        let mut index = 0usize;
        let contains = |set: &str, c: u32| set.chars().any(|x| x as u32 == c);
        while index < s.len() {
            let c = code_point_at(&s, index);
            if index == 0 {
                if contains("#,[]{}&*!|>'\"%@`", c) {
                    flow_indicators = true;
                    block_indicators = true;
                }
                if c == '?' as u32 || c == ':' as u32 {
                    flow_indicators = true;
                    if followed_by_whitespace {
                        block_indicators = true;
                    }
                }
                if c == '-' as u32 && followed_by_whitespace {
                    flow_indicators = true;
                    block_indicators = true;
                }
            } else {
                if contains(",?[]{}", c) {
                    flow_indicators = true;
                }
                if c == ':' as u32 {
                    flow_indicators = true;
                    if followed_by_whitespace {
                        block_indicators = true;
                    }
                }
                if c == '#' as u32 && preceded_by_whitespace {
                    flow_indicators = true;
                    block_indicators = true;
                }
            }
            let is_line_break = is_linebr(c);
            if is_line_break {
                line_breaks = true;
            }
            if c != 10 && !(32..=126).contains(&c) {
                if c == 133 || (160..=55295).contains(&c) || (57344..=65533).contains(&c) || (65536..=1114111).contains(&c) {
                    if !self.allow_unicode {
                        special_characters = true;
                    }
                } else {
                    special_characters = true;
                }
            }
            if c == 32 {
                if index == 0 {
                    leading_space = true;
                }
                if index == s.len() - 1 {
                    trailing_space = true;
                }
                if previous_break {
                    break_space = true;
                }
                previous_space = true;
                previous_break = false;
            } else if is_line_break {
                if index == 0 {
                    leading_break = true;
                }
                if index == s.len() - 1 {
                    trailing_break = true;
                }
                if previous_space {
                    space_break = true;
                }
                previous_space = false;
                previous_break = true;
            } else {
                previous_space = false;
                previous_break = false;
            }
            index += char_count(c);
            preceded_by_whitespace = is_null_bl_t(c) || is_line_break;
            followed_by_whitespace = true;
            if index + 1 < s.len() {
                let next_index = index + char_count(code_point_at(&s, index));
                if next_index < s.len() {
                    followed_by_whitespace = is_null_bl_t(code_point_at(&s, next_index)) || is_line_break;
                }
            }
        }
        let mut allow_flow_plain = true;
        let mut allow_block_plain = true;
        let mut allow_single_quoted = true;
        let mut allow_block = true;
        if leading_space || leading_break || trailing_space || trailing_break || leading_zero_number {
            allow_block_plain = false;
            allow_flow_plain = false;
        }
        if trailing_space {
            allow_block = false;
        }
        if break_space {
            allow_single_quoted = false;
            allow_block_plain = false;
            allow_flow_plain = false;
        }
        if space_break || special_characters {
            allow_block = false;
            allow_single_quoted = false;
            allow_block_plain = false;
            allow_flow_plain = false;
        }
        if line_breaks {
            allow_flow_plain = false;
        }
        if flow_indicators {
            allow_flow_plain = false;
        }
        if block_indicators {
            allow_block_plain = false;
        }
        ScalarAnalysis { scalar: s, empty: false, multiline: line_breaks, allow_flow_plain, allow_block_plain, allow_single_quoted, allow_block }
    }

    // ---- writers ----

    fn write_units(&mut self, u: &[u16]) {
        self.out.extend_from_slice(u);
    }

    fn write_str(&mut self, s: &str) {
        self.out.extend(s.encode_utf16());
    }

    fn write_indicator(&mut self, indicator: &str, need_whitespace: bool, whitespace: bool, indentation: bool) {
        if !self.whitespace && need_whitespace {
            self.column += 1;
            self.out.push(b' ' as u16);
        }
        self.whitespace = whitespace;
        self.indention = self.indention && indentation;
        self.column += indicator.encode_utf16().count() as i32;
        self.open_ended = false;
        self.write_str(indicator);
    }

    fn write_indent(&mut self) {
        let indent = self.indent.unwrap_or(0);
        if !self.indention || self.column > indent || self.column == indent && !self.whitespace {
            self.write_line_break(None);
        }
        self.write_whitespace(indent - self.column);
    }

    fn write_whitespace(&mut self, length: i32) {
        if length > 0 {
            self.whitespace = true;
            for _ in 0..length {
                self.out.push(b' ' as u16);
            }
            self.column += length;
        }
    }

    fn write_line_break(&mut self, data: Option<u16>) {
        self.whitespace = true;
        self.indention = true;
        self.column = 0;
        match data {
            None => {
                let lb = self.best_line_break.clone();
                self.write_units(&lb);
            }
            Some(c) => self.out.push(c),
        }
    }

    fn write_breaks(&mut self, data: &[u16]) {
        for &br in data {
            if br == b'\n' as u16 {
                self.write_line_break(None);
            } else {
                self.write_line_break(Some(br));
            }
        }
    }

    fn write_single_quoted(&mut self, text: &[u16], split: bool) {
        self.write_indicator("'", true, false, false);
        let mut spaces = false;
        let mut breaks = false;
        let mut start = 0usize;
        let n = text.len();
        let mut end = 0usize;
        while end <= n {
            let ch: u16 = if end < n { text[end] } else { 0 };
            if spaces {
                if ch == 0 || ch != b' ' as u16 {
                    if start + 1 == end && self.column > self.best_width && split && start != 0 && end != n {
                        self.write_indent();
                    } else {
                        self.column += (end - start) as i32;
                        self.write_units(&text[start..end]);
                    }
                    start = end;
                }
            } else if !breaks {
                if (is_linebr(ch as u32) || ch == 0 || ch == b' ' as u16 || ch == b'\'' as u16) && start < end {
                    self.column += (end - start) as i32;
                    self.write_units(&text[start..end]);
                    start = end;
                }
            } else if ch == 0 || !is_linebr(ch as u32) {
                if text[start] == b'\n' as u16 {
                    self.write_line_break(None);
                }
                let data = text[start..end].to_vec();
                self.write_breaks(&data);
                self.write_indent();
                start = end;
            }
            if ch == b'\'' as u16 {
                self.column += 2;
                self.write_str("''");
                start = end + 1;
            }
            if ch != 0 {
                spaces = ch == b' ' as u16;
                breaks = is_linebr(ch as u32);
            }
            end += 1;
        }
        self.write_indicator("'", false, false, false);
    }

    fn write_double_quoted(&mut self, text: &[u16], split: bool) {
        self.write_indicator("\"", true, false, false);
        let mut start = 0usize;
        let n = text.len();
        let mut end = 0usize;
        while end <= n {
            let ch: Option<u16> = if end < n { Some(text[end]) } else { None };
            let special = match ch {
                None => true,
                Some(c) => [0x22u16, 0x5c, 0x85, 0x2028, 0x2029, 0xfeff].contains(&c) || !(0x20..=0x7e).contains(&c),
            };
            if special {
                if start < end {
                    self.column += (end - start) as i32;
                    self.write_units(&text[start..end]);
                    start = end;
                }
                if let Some(c) = ch {
                    let data: String = if let Some(r) = escape_replacement(c) {
                        format!("\\{r}")
                    } else {
                        let code_point = if (0xd800..0xdc00).contains(&c) && end + 1 < n {
                            let c2 = text[end + 1];
                            0x10000 + (((c as u32) - 0xd800) << 10) + ((c2 as u32).wrapping_sub(0xdc00))
                        } else {
                            c as u32
                        };
                        if self.allow_unicode && is_printable(code_point) {
                            let s = char::from_u32(code_point).map(|x| x.to_string()).unwrap_or_default();
                            if char_count(code_point) == 2 {
                                end += 1;
                            }
                            s
                        } else if c <= 255 {
                            format!("\\x{:02x}", c)
                        } else if char_count(code_point) == 2 {
                            end += 1;
                            let s = format!("000{:x}", code_point);
                            format!("\\U{}", &s[s.len() - 8..])
                        } else {
                            let s = format!("000{:x}", c);
                            format!("\\u{}", &s[s.len() - 4..])
                        }
                    };
                    self.column += data.encode_utf16().count() as i32;
                    self.write_str(&data);
                    start = end + 1;
                }
            }
            let is_space = ch == Some(b' ' as u16);
            if 0 < end && end + 1 < n && (is_space || start >= end) && self.column + (end as i32 - start as i32) > self.best_width && split {
                let data: Vec<u16> = if start >= end {
                    vec![b'\\' as u16]
                } else {
                    let mut d = text[start..end].to_vec();
                    d.push(b'\\' as u16);
                    d
                };
                if start < end {
                    start = end;
                }
                self.column += data.len() as i32;
                self.write_units(&data);
                self.write_indent();
                self.whitespace = false;
                self.indention = false;
                if text[start] == b' ' as u16 {
                    self.column += 1;
                    self.write_str("\\");
                }
            }
            end += 1;
        }
        self.write_indicator("\"", false, false, false);
    }

    fn determine_block_hints(&self, text: &[u16]) -> String {
        let mut hints = String::new();
        if is_linebr(text[0] as u32) || text[0] == b' ' as u16 {
            hints.push_str(&self.best_indent.to_string());
        }
        let ch1 = text[text.len() - 1];
        if !is_linebr(ch1 as u32) {
            hints.push('-');
        } else if text.len() == 1 || is_linebr(text[text.len() - 2] as u32) {
            hints.push('+');
        }
        hints
    }

    fn write_folded(&mut self, text: &[u16], split: bool) {
        let hints = self.determine_block_hints(text);
        self.write_indicator(&format!(">{hints}"), true, false, false);
        if hints.ends_with('+') {
            self.open_ended = true;
        }
        self.write_line_break(None);
        let mut leading_space = true;
        let mut spaces = false;
        let mut breaks = true;
        let mut start = 0usize;
        let n = text.len();
        let mut end = 0usize;
        while end <= n {
            let ch: u16 = if end < n { text[end] } else { 0 };
            if breaks {
                if ch == 0 || !is_linebr(ch as u32) {
                    if !leading_space && ch != 0 && ch != b' ' as u16 && text[start] == b'\n' as u16 {
                        self.write_line_break(None);
                    }
                    leading_space = ch == b' ' as u16;
                    let data = text[start..end].to_vec();
                    self.write_breaks(&data);
                    if ch != 0 {
                        self.write_indent();
                    }
                    start = end;
                }
            } else if spaces {
                if ch != b' ' as u16 {
                    if start + 1 == end && self.column > self.best_width && split {
                        self.write_indent();
                    } else {
                        self.column += (end - start) as i32;
                        self.write_units(&text[start..end]);
                    }
                    start = end;
                }
            } else if is_linebr(ch as u32) || ch == 0 || ch == b' ' as u16 {
                self.column += (end - start) as i32;
                self.write_units(&text[start..end]);
                if ch == 0 {
                    self.write_line_break(None);
                }
                start = end;
            }
            if ch != 0 {
                breaks = is_linebr(ch as u32);
                spaces = ch == b' ' as u16;
            }
            end += 1;
        }
    }

    fn write_literal(&mut self, text: &[u16]) {
        let hints = self.determine_block_hints(text);
        self.write_indicator(&format!("|{hints}"), true, false, false);
        if hints.ends_with('+') {
            self.open_ended = true;
        }
        self.write_line_break(None);
        let mut breaks = true;
        let mut start = 0usize;
        let n = text.len();
        let mut end = 0usize;
        while end <= n {
            let ch: u16 = if end < n { text[end] } else { 0 };
            if !breaks {
                if ch == 0 || is_linebr(ch as u32) {
                    self.write_units(&text[start..end]);
                    if ch == 0 {
                        self.write_line_break(None);
                    }
                    start = end;
                }
            } else if ch == 0 || !is_linebr(ch as u32) {
                let data = text[start..end].to_vec();
                self.write_breaks(&data);
                if ch != 0 {
                    self.write_indent();
                }
                start = end;
            }
            if ch != 0 {
                breaks = is_linebr(ch as u32);
            }
            end += 1;
        }
    }

    fn write_plain(&mut self, text: &[u16], split: bool) {
        if self.root_context {
            self.open_ended = true;
        }
        if text.is_empty() {
            return;
        }
        if !self.whitespace {
            self.column += 1;
            self.out.push(b' ' as u16);
        }
        self.whitespace = false;
        self.indention = false;
        let mut spaces = false;
        let mut breaks = false;
        let mut start = 0usize;
        let n = text.len();
        let mut end = 0usize;
        while end <= n {
            let ch: u16 = if end < n { text[end] } else { 0 };
            if spaces {
                if ch != b' ' as u16 {
                    if start + 1 == end && self.column > self.best_width && split {
                        self.write_indent();
                        self.whitespace = false;
                        self.indention = false;
                    } else {
                        self.column += (end - start) as i32;
                        self.write_units(&text[start..end]);
                    }
                    start = end;
                }
            } else if !breaks {
                if is_linebr(ch as u32) || ch == 0 || ch == b' ' as u16 {
                    self.column += (end - start) as i32;
                    self.write_units(&text[start..end]);
                    start = end;
                }
            } else if !is_linebr(ch as u32) {
                if text[start] == b'\n' as u16 {
                    self.write_line_break(None);
                }
                let data = text[start..end].to_vec();
                self.write_breaks(&data);
                self.write_indent();
                self.whitespace = false;
                self.indention = false;
                start = end;
            }
            if ch != 0 {
                spaces = ch == b' ' as u16;
                breaks = is_linebr(ch as u32);
            }
            end += 1;
        }
    }
}

/// `prepareAnchor(anchor)`
fn prepare_anchor(anchor: &str) -> Result<String, EmitterError> {
    if anchor.is_empty() {
        return Err(EmitterError("anchor must not be empty".into()));
    }
    for invalid in ['[', ']', '{', '}', ',', '*', '&'] {
        if anchor.contains(invalid) {
            return Err(EmitterError(format!("Invalid character '{invalid}' in the anchor: {anchor}")));
        }
    }
    if anchor.chars().any(|c| matches!(c, ' ' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\r')) {
        return Err(EmitterError(format!("Anchor may not contain spaces: {anchor}")));
    }
    Ok(anchor.to_string())
}
