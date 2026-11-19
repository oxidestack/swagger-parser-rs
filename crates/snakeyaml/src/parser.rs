//! `ParserImpl`: tokens → events.

use crate::error::{Mark, Result, YamlError};
use crate::events::{Event, EventKind, ImplicitTuple};
use crate::options::{FlowStyle, LoaderOptions, ScalarStyle, Version};
use crate::reader::StreamReader;
use crate::scanner::{DirectiveValue, Scanner, TokenData, TokenId};

#[derive(Clone, Debug)]
enum State {
    StreamStart,
    ImplicitDocumentStart,
    DocumentStart,
    DocumentEnd,
    DocumentContent,
    BlockNode,
    BlockSequenceFirstEntry,
    BlockSequenceEntryKey,
    IndentlessSequenceEntryKey,
    BlockMappingFirstKey,
    BlockMappingKey,
    BlockMappingValue,
    FlowSequenceFirstEntry,
    FlowSequenceEntry(bool),
    FlowSequenceEntryMappingKey,
    FlowSequenceEntryMappingValue,
    FlowSequenceEntryMappingEnd,
    FlowMappingFirstKey,
    FlowMappingKey(bool),
    FlowMappingValue,
    FlowMappingEmptyValue,
}

/// Tag handles and their prefixes, in declaration order.
type TagHandles = Vec<(String, String)>;

/// `org.yaml.snakeyaml.parser.ParserImpl`: turns scanner tokens into [`Event`]s on demand.
pub struct Parser {
    scanner: Scanner,
    current: Option<Event>,
    states: Vec<State>,
    marks: Vec<Mark>,
    state: Option<State>,
    /// tag handles in effect (`directives.getTags()`)
    tags: Vec<(String, String)>,
    version: Option<Version>,
}

fn default_tags() -> Vec<(String, String)> {
    vec![("!".into(), "!".into()), ("!!".into(), "tag:yaml.org,2002:".into())]
}

impl Parser {
    /// `new ParserImpl(new StreamReader(input), options)`: a parser over a string, named `'string'` in marks.
    pub fn new(input: &str, options: LoaderOptions) -> Parser {
        Parser::from_reader(StreamReader::new(input), options)
    }

    /// A parser over an existing reader.
    pub fn from_reader(reader: StreamReader, options: LoaderOptions) -> Parser {
        Parser {
            scanner: Scanner::new(reader, options),
            current: None,
            states: Vec::new(),
            marks: Vec::new(),
            state: Some(State::StreamStart),
            tags: default_tags(),
            version: None,
        }
    }

    /// `peekEvent()`: the next event, left in place; `Ok(None)` after `StreamEnd`.
    pub fn peek_event(&mut self) -> Result<Option<&Event>> {
        if self.current.is_none() {
            if let Some(st) = self.state.take() {
                let e = self.produce(st)?;
                self.current = Some(e);
            }
        }
        Ok(self.current.as_ref())
    }

    /// `getEvent()`: removes and returns the next event; `Ok(None)` after `StreamEnd`.
    pub fn get_event(&mut self) -> Result<Option<Event>> {
        self.peek_event()?;
        Ok(self.current.take())
    }

    fn pop_state(&mut self) -> Option<State> {
        self.states.pop()
    }

    fn produce(&mut self, st: State) -> Result<Event> {
        match st {
            State::StreamStart => {
                let t = self.scanner.get_token()?;
                self.state = Some(State::ImplicitDocumentStart);
                Ok(Event::new(EventKind::StreamStart, t.start_mark, t.end_mark))
            }
            State::ImplicitDocumentStart => {
                if !self.scanner.check_token(&[TokenId::Directive, TokenId::DocumentStart, TokenId::StreamEnd])? {
                    let start = self.scanner.peek_token()?.start_mark.clone();
                    self.states.push(State::DocumentEnd);
                    self.state = Some(State::BlockNode);
                    Ok(Event::new(EventKind::DocumentStart { explicit: false, version: None, tags: None }, start.clone(), start))
                } else {
                    self.produce(State::DocumentStart)
                }
            }
            State::DocumentStart => {
                while self.scanner.check_token(&[TokenId::DocumentEnd])? {
                    self.scanner.get_token()?;
                }
                if !self.scanner.check_token(&[TokenId::StreamEnd])? {
                    self.scanner.reset_document_index();
                    let start = self.scanner.peek_token()?.start_mark.clone();
                    let (version, tags) = self.process_directives()?;
                    if !self.scanner.check_token(&[TokenId::StreamEnd])? {
                        if !self.scanner.check_token(&[TokenId::DocumentStart])? {
                            let t = self.scanner.peek_token()?;
                            return Err(YamlError::parser(None, None, &format!("expected '<document start>', but found '{}'", t.id), t.start_mark.clone()));
                        }
                        let t = self.scanner.get_token()?;
                        self.states.push(State::DocumentEnd);
                        self.state = Some(State::DocumentContent);
                        return Ok(Event::new(EventKind::DocumentStart { explicit: true, version, tags: Some(tags) }, start, t.end_mark));
                    }
                }
                let t = self.scanner.get_token()?;
                if !self.states.is_empty() {
                    return Err(YamlError::plain(format!("Unexpected end of stream. States left: {:?}", self.states)));
                }
                if !self.marks.is_empty() {
                    return Err(YamlError::plain("Unexpected end of stream. Marks left"));
                }
                self.state = None;
                Ok(Event::new(EventKind::StreamEnd, t.start_mark, t.end_mark))
            }
            State::DocumentEnd => {
                let t = self.scanner.peek_token()?;
                let start = t.start_mark.clone();
                let mut end = start.clone();
                let mut explicit = false;
                if self.scanner.check_token(&[TokenId::DocumentEnd])? {
                    let t = self.scanner.get_token()?;
                    end = t.end_mark;
                    explicit = true;
                }
                self.state = Some(State::DocumentStart);
                Ok(Event::new(EventKind::DocumentEnd { explicit }, start, end))
            }
            State::DocumentContent => {
                if self.scanner.check_token(&[TokenId::Directive, TokenId::DocumentStart, TokenId::DocumentEnd, TokenId::StreamEnd])? {
                    let m = self.scanner.peek_token()?.start_mark.clone();
                    self.state = self.pop_state();
                    Ok(empty_scalar(m))
                } else {
                    self.parse_node(true, false)
                }
            }
            State::BlockNode => self.parse_node(true, false),
            State::BlockSequenceFirstEntry => {
                let t = self.scanner.get_token()?;
                self.marks.push(t.start_mark);
                self.produce(State::BlockSequenceEntryKey)
            }
            State::BlockSequenceEntryKey => {
                if self.scanner.check_token(&[TokenId::BlockEntry])? {
                    let t = self.scanner.get_token()?;
                    if !self.scanner.check_token(&[TokenId::BlockEntry, TokenId::BlockEnd])? {
                        self.states.push(State::BlockSequenceEntryKey);
                        self.parse_node(true, false)
                    } else {
                        self.state = Some(State::BlockSequenceEntryKey);
                        Ok(empty_scalar(t.end_mark))
                    }
                } else if !self.scanner.check_token(&[TokenId::BlockEnd])? {
                    let t = self.scanner.peek_token()?.clone();
                    let m = self.marks.pop();
                    Err(YamlError::parser(Some("while parsing a block collection"), m, &format!("expected <block end>, but found '{}'", t.id), t.start_mark))
                } else {
                    let t = self.scanner.get_token()?;
                    self.state = self.pop_state();
                    self.marks.pop();
                    Ok(Event::new(EventKind::SequenceEnd, t.start_mark, t.end_mark))
                }
            }
            State::IndentlessSequenceEntryKey => {
                if self.scanner.check_token(&[TokenId::BlockEntry])? {
                    let t = self.scanner.get_token()?;
                    if !self.scanner.check_token(&[TokenId::BlockEntry, TokenId::Key, TokenId::Value, TokenId::BlockEnd])? {
                        self.states.push(State::IndentlessSequenceEntryKey);
                        self.parse_node(true, false)
                    } else {
                        self.state = Some(State::IndentlessSequenceEntryKey);
                        Ok(empty_scalar(t.end_mark))
                    }
                } else {
                    let t = self.scanner.peek_token()?;
                    let (s, e) = (t.start_mark.clone(), t.end_mark.clone());
                    self.state = self.pop_state();
                    Ok(Event::new(EventKind::SequenceEnd, s, e))
                }
            }
            State::BlockMappingFirstKey => {
                let t = self.scanner.get_token()?;
                self.marks.push(t.start_mark);
                self.produce(State::BlockMappingKey)
            }
            State::BlockMappingKey => {
                if self.scanner.check_token(&[TokenId::Key])? {
                    let t = self.scanner.get_token()?;
                    if !self.scanner.check_token(&[TokenId::Key, TokenId::Value, TokenId::BlockEnd])? {
                        self.states.push(State::BlockMappingValue);
                        self.parse_node(true, true)
                    } else {
                        self.state = Some(State::BlockMappingValue);
                        Ok(empty_scalar(t.end_mark))
                    }
                } else if !self.scanner.check_token(&[TokenId::BlockEnd])? {
                    let t = self.scanner.peek_token()?.clone();
                    let m = self.marks.pop();
                    Err(YamlError::parser(Some("while parsing a block mapping"), m, &format!("expected <block end>, but found '{}'", t.id), t.start_mark))
                } else {
                    let t = self.scanner.get_token()?;
                    self.state = self.pop_state();
                    self.marks.pop();
                    Ok(Event::new(EventKind::MappingEnd, t.start_mark, t.end_mark))
                }
            }
            State::BlockMappingValue => {
                if self.scanner.check_token(&[TokenId::Value])? {
                    let t = self.scanner.get_token()?;
                    if !self.scanner.check_token(&[TokenId::Key, TokenId::Value, TokenId::BlockEnd])? {
                        self.states.push(State::BlockMappingKey);
                        self.parse_node(true, true)
                    } else {
                        self.state = Some(State::BlockMappingKey);
                        Ok(empty_scalar(t.end_mark))
                    }
                } else if self.scanner.check_token(&[TokenId::Scalar])? {
                    self.states.push(State::BlockMappingKey);
                    self.parse_node(true, true)
                } else {
                    self.state = Some(State::BlockMappingKey);
                    let m = self.scanner.peek_token()?.start_mark.clone();
                    Ok(empty_scalar(m))
                }
            }
            State::FlowSequenceFirstEntry => {
                let t = self.scanner.get_token()?;
                self.marks.push(t.start_mark);
                self.produce(State::FlowSequenceEntry(true))
            }
            State::FlowSequenceEntry(first) => {
                if !self.scanner.check_token(&[TokenId::FlowSequenceEnd])? {
                    if !first {
                        if !self.scanner.check_token(&[TokenId::FlowEntry])? {
                            let t = self.scanner.peek_token()?.clone();
                            let m = self.marks.pop();
                            return Err(YamlError::parser(Some("while parsing a flow sequence"), m, &format!("expected ',' or ']', but got {}", t.id), t.start_mark));
                        }
                        self.scanner.get_token()?;
                    }
                    if self.scanner.check_token(&[TokenId::Key])? {
                        let t = self.scanner.peek_token()?;
                        let (s, e) = (t.start_mark.clone(), t.end_mark.clone());
                        self.state = Some(State::FlowSequenceEntryMappingKey);
                        return Ok(Event::new(EventKind::MappingStart { anchor: None, tag: None, implicit: true, flow_style: FlowStyle::Flow }, s, e));
                    }
                    if !self.scanner.check_token(&[TokenId::FlowSequenceEnd])? {
                        self.states.push(State::FlowSequenceEntry(false));
                        return self.parse_node(false, false);
                    }
                }
                let t = self.scanner.get_token()?;
                self.state = self.pop_state();
                self.marks.pop();
                Ok(Event::new(EventKind::SequenceEnd, t.start_mark, t.end_mark))
            }
            State::FlowSequenceEntryMappingKey => {
                let t = self.scanner.get_token()?;
                if !self.scanner.check_token(&[TokenId::Value, TokenId::FlowEntry, TokenId::FlowSequenceEnd])? {
                    self.states.push(State::FlowSequenceEntryMappingValue);
                    self.parse_node(false, false)
                } else {
                    self.state = Some(State::FlowSequenceEntryMappingValue);
                    Ok(empty_scalar(t.end_mark))
                }
            }
            State::FlowSequenceEntryMappingValue => {
                if self.scanner.check_token(&[TokenId::Value])? {
                    let t = self.scanner.get_token()?;
                    if !self.scanner.check_token(&[TokenId::FlowEntry, TokenId::FlowSequenceEnd])? {
                        self.states.push(State::FlowSequenceEntryMappingEnd);
                        self.parse_node(false, false)
                    } else {
                        self.state = Some(State::FlowSequenceEntryMappingEnd);
                        Ok(empty_scalar(t.end_mark))
                    }
                } else {
                    self.state = Some(State::FlowSequenceEntryMappingEnd);
                    let m = self.scanner.peek_token()?.start_mark.clone();
                    Ok(empty_scalar(m))
                }
            }
            State::FlowSequenceEntryMappingEnd => {
                self.state = Some(State::FlowSequenceEntry(false));
                let t = self.scanner.peek_token()?;
                Ok(Event::new(EventKind::MappingEnd, t.start_mark.clone(), t.end_mark.clone()))
            }
            State::FlowMappingFirstKey => {
                let t = self.scanner.get_token()?;
                self.marks.push(t.start_mark);
                self.produce(State::FlowMappingKey(true))
            }
            State::FlowMappingKey(first) => {
                if !self.scanner.check_token(&[TokenId::FlowMappingEnd])? {
                    if !first {
                        if !self.scanner.check_token(&[TokenId::FlowEntry])? {
                            let t = self.scanner.peek_token()?.clone();
                            let m = self.marks.pop();
                            return Err(YamlError::parser(Some("while parsing a flow mapping"), m, &format!("expected ',' or '}}', but got {}", t.id), t.start_mark));
                        }
                        self.scanner.get_token()?;
                    }
                    if self.scanner.check_token(&[TokenId::Key])? {
                        let t = self.scanner.get_token()?;
                        if !self.scanner.check_token(&[TokenId::Value, TokenId::FlowEntry, TokenId::FlowMappingEnd])? {
                            self.states.push(State::FlowMappingValue);
                            return self.parse_node(false, false);
                        }
                        self.state = Some(State::FlowMappingValue);
                        return Ok(empty_scalar(t.end_mark));
                    }
                    if !self.scanner.check_token(&[TokenId::FlowMappingEnd])? {
                        self.states.push(State::FlowMappingEmptyValue);
                        return self.parse_node(false, false);
                    }
                }
                let t = self.scanner.get_token()?;
                self.marks.pop();
                self.state = self.pop_state();
                Ok(Event::new(EventKind::MappingEnd, t.start_mark, t.end_mark))
            }
            State::FlowMappingValue => {
                if self.scanner.check_token(&[TokenId::Value])? {
                    let t = self.scanner.get_token()?;
                    if !self.scanner.check_token(&[TokenId::FlowEntry, TokenId::FlowMappingEnd])? {
                        self.states.push(State::FlowMappingKey(false));
                        self.parse_node(false, false)
                    } else {
                        self.state = Some(State::FlowMappingKey(false));
                        Ok(empty_scalar(t.end_mark))
                    }
                } else {
                    self.state = Some(State::FlowMappingKey(false));
                    let m = self.scanner.peek_token()?.start_mark.clone();
                    Ok(empty_scalar(m))
                }
            }
            State::FlowMappingEmptyValue => {
                self.state = Some(State::FlowMappingKey(false));
                let m = self.scanner.peek_token()?.start_mark.clone();
                Ok(empty_scalar(m))
            }
        }
    }

    fn process_directives(&mut self) -> Result<(Option<Version>, TagHandles)> {
        // tag handles from the previous document minus the defaults
        let mut handles: Vec<(String, String)> = self.tags.iter().filter(|(k, _)| k != "!" && k != "!!").cloned().collect();
        self.version = None;
        while self.scanner.check_token(&[TokenId::Directive])? {
            let t = self.scanner.get_token()?;
            if let TokenData::Directive { name, value } = &t.data {
                if name == "YAML" {
                    if self.version.is_some() {
                        return Err(YamlError::parser(None, None, "found duplicate YAML directive", t.start_mark));
                    }
                    if let DirectiveValue::Yaml(major, minor) = value {
                        if *major != 1 {
                            return Err(YamlError::parser(None, None, "found incompatible YAML document (version 1.* is required)", t.start_mark));
                        }
                        self.version = Some(if *minor == 0 { Version::V1_0 } else { Version::V1_1 });
                    }
                } else if name == "TAG" {
                    if let DirectiveValue::Tag(handle, prefix) = value {
                        if handles.iter().any(|(k, _)| k == handle) {
                            return Err(YamlError::parser(None, None, &format!("duplicate tag handle {handle}"), t.start_mark));
                        }
                        handles.push((handle.clone(), prefix.clone()));
                    }
                }
            }
        }
        let detected = handles.clone();
        for (k, v) in default_tags() {
            if !handles.iter().any(|(h, _)| *h == k) {
                handles.push((k, v));
            }
        }
        self.tags = handles;
        Ok((self.version, detected))
    }

    fn parse_node(&mut self, block: bool, indentless_sequence: bool) -> Result<Event> {
        if self.scanner.check_token(&[TokenId::Alias])? {
            let t = self.scanner.get_token()?;
            let anchor = match t.data {
                TokenData::Name(n) => n,
                _ => unreachable!(),
            };
            self.state = self.pop_state();
            return Ok(Event::new(EventKind::Alias { anchor }, t.start_mark, t.end_mark));
        }
        let mut anchor: Option<String> = None;
        let mut tag_tuple: Option<(Option<String>, String)> = None;
        let mut start_mark: Option<Mark> = None;
        let mut end_mark: Option<Mark> = None;
        let mut tag_mark: Option<Mark> = None;
        if self.scanner.check_token(&[TokenId::Anchor])? {
            let t = self.scanner.get_token()?;
            start_mark = Some(t.start_mark);
            end_mark = Some(t.end_mark);
            if let TokenData::Name(n) = t.data {
                anchor = Some(n);
            }
            if self.scanner.check_token(&[TokenId::Tag])? {
                let t = self.scanner.get_token()?;
                tag_mark = Some(t.start_mark);
                end_mark = Some(t.end_mark);
                if let TokenData::Tag(h, s) = t.data {
                    tag_tuple = Some((h, s));
                }
            }
        } else if self.scanner.check_token(&[TokenId::Tag])? {
            let t = self.scanner.get_token()?;
            start_mark = Some(t.start_mark.clone());
            tag_mark = Some(t.start_mark);
            end_mark = Some(t.end_mark);
            if let TokenData::Tag(h, s) = t.data {
                tag_tuple = Some((h, s));
            }
            if self.scanner.check_token(&[TokenId::Anchor])? {
                let t = self.scanner.get_token()?;
                end_mark = Some(t.end_mark);
                if let TokenData::Name(n) = t.data {
                    anchor = Some(n);
                }
            }
        }
        let mut tag: Option<String> = None;
        if let Some((handle, suffix)) = tag_tuple {
            match handle {
                Some(h) => match self.tags.iter().find(|(k, _)| *k == h) {
                    Some((_, prefix)) => tag = Some(format!("{prefix}{suffix}")),
                    None => {
                        return Err(YamlError::parser(Some("while parsing a node"), start_mark, &format!("found undefined tag handle {h}"), tag_mark.unwrap()));
                    }
                },
                None => tag = Some(suffix),
            }
        }
        let start_mark = match start_mark {
            Some(m) => m,
            None => {
                let m = self.scanner.peek_token()?.start_mark.clone();
                end_mark = Some(m.clone());
                m
            }
        };
        let mut end_mark = end_mark.unwrap();
        let implicit = tag.is_none() || tag.as_deref() == Some("!");
        if indentless_sequence && self.scanner.check_token(&[TokenId::BlockEntry])? {
            end_mark = self.scanner.peek_token()?.end_mark.clone();
            self.state = Some(State::IndentlessSequenceEntryKey);
            return Ok(Event::new(EventKind::SequenceStart { anchor, tag, implicit, flow_style: FlowStyle::Block }, start_mark, end_mark));
        }
        if self.scanner.check_token(&[TokenId::Scalar])? {
            let t = self.scanner.get_token()?;
            let (value, plain, style) = match t.data {
                TokenData::Scalar { value, plain, style } => (value, plain, style),
                _ => unreachable!(),
            };
            let implicit_values = if (plain && tag.is_none()) || tag.as_deref() == Some("!") {
                ImplicitTuple { plain: true, non_plain: false }
            } else if tag.is_none() {
                ImplicitTuple { plain: false, non_plain: true }
            } else {
                ImplicitTuple { plain: false, non_plain: false }
            };
            self.state = self.pop_state();
            return Ok(Event::new(EventKind::Scalar { anchor, tag, implicit: implicit_values, value, style }, start_mark, t.end_mark));
        }
        if self.scanner.check_token(&[TokenId::FlowSequenceStart])? {
            end_mark = self.scanner.peek_token()?.end_mark.clone();
            self.state = Some(State::FlowSequenceFirstEntry);
            return Ok(Event::new(EventKind::SequenceStart { anchor, tag, implicit, flow_style: FlowStyle::Flow }, start_mark, end_mark));
        }
        if self.scanner.check_token(&[TokenId::FlowMappingStart])? {
            end_mark = self.scanner.peek_token()?.end_mark.clone();
            self.state = Some(State::FlowMappingFirstKey);
            return Ok(Event::new(EventKind::MappingStart { anchor, tag, implicit, flow_style: FlowStyle::Flow }, start_mark, end_mark));
        }
        if block && self.scanner.check_token(&[TokenId::BlockSequenceStart])? {
            end_mark = self.scanner.peek_token()?.start_mark.clone();
            self.state = Some(State::BlockSequenceFirstEntry);
            return Ok(Event::new(EventKind::SequenceStart { anchor, tag, implicit, flow_style: FlowStyle::Block }, start_mark, end_mark));
        }
        if block && self.scanner.check_token(&[TokenId::BlockMappingStart])? {
            end_mark = self.scanner.peek_token()?.start_mark.clone();
            self.state = Some(State::BlockMappingFirstKey);
            return Ok(Event::new(EventKind::MappingStart { anchor, tag, implicit, flow_style: FlowStyle::Block }, start_mark, end_mark));
        }
        if anchor.is_none() && tag.is_none() {
            let t = self.scanner.peek_token()?;
            return Err(YamlError::parser(
                Some(&format!("while parsing a {} node", if block { "block" } else { "flow" })),
                Some(start_mark),
                &format!("expected the node content, but found '{}'", t.id),
                t.start_mark.clone(),
            ));
        }
        self.state = self.pop_state();
        Ok(Event::new(
            EventKind::Scalar { anchor, tag, implicit: ImplicitTuple { plain: implicit, non_plain: false }, value: String::new(), style: ScalarStyle::Plain },
            start_mark,
            end_mark,
        ))
    }
}

fn empty_scalar(mark: Mark) -> Event {
    Event::new(
        EventKind::Scalar { anchor: None, tag: None, implicit: ImplicitTuple { plain: true, non_plain: false }, value: String::new(), style: ScalarStyle::Plain },
        mark.clone(),
        mark,
    )
}
