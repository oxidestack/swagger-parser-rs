//! `YAMLMapper.readTree(String)`: jackson-dataformat-yaml's `YAMLParser` over SnakeYAML events, feeding
//! `JsonNodeDeserializer`.

use crate::JacksonError;
use crate::node::{JsonNode, ObjectNode};
use num_bigint::BigInt;
use snakeyaml::events::{Event, EventKind};
use snakeyaml::nodes::*;
use snakeyaml::parser::Parser;
use snakeyaml::reader::StreamReader;
use snakeyaml::{LoaderOptions, Resolver, YamlError};
use std::collections::HashSet;
use std::sync::{Arc, OnceLock};

const MAX_NESTING_DEPTH: usize = 1000;

/// `JsonParser`/`YAMLParser` features the YAML tree reader honours.
#[derive(Clone, Copy, Debug)]
pub struct YamlReadFeatures {
    /// `StreamReadFeature.STRICT_DUPLICATE_DETECTION`
    pub strict_duplicate_detection: bool,
    /// `YAMLParser.Feature.EMPTY_STRING_AS_NULL` (on for `new YAMLFactory()`, off for `YAMLFactory.builder()`)
    pub empty_string_as_null: bool,
}

impl YamlReadFeatures {
    /// swagger-core `Yaml.mapper()` (`new YAMLFactory()`)
    pub fn swagger_core() -> YamlReadFeatures {
        YamlReadFeatures { strict_duplicate_detection: false, empty_string_as_null: true }
    }

    /// swagger-parser `ObjectMapperFactory.createYaml()` (`YAMLFactory.builder()` + strict duplicates)
    pub fn swagger_parser() -> YamlReadFeatures {
        YamlReadFeatures { strict_duplicate_detection: true, empty_string_as_null: false }
    }
}

fn resolver() -> &'static Resolver {
    static R: OnceLock<Resolver> = OnceLock::new();
    R.get_or_init(Resolver::default)
}

enum Tok {
    StartObject,
    EndObject,
    StartArray,
    EndArray,
    Field(String),
    Value(JsonNode),
}

enum Frame {
    Obj(ObjectNode, Option<String>, Option<HashSet<String>>),
    Arr(Vec<JsonNode>),
}

struct YParser {
    parser: Parser,
    last_end: Option<snakeyaml::Mark>,
    features: YamlReadFeatures,
    /// true when the previous token was a FIELD_NAME
    after_field: bool,
    depth: usize,
}

fn location(m: &Option<snakeyaml::Mark>) -> String {
    match m {
        Some(m) => format!("[Source: (StringReader); line: {}, column: {}]", m.line + 1, m.column + 1),
        None => "[Source: (StringReader); line: -1, column: -1]".to_string(),
    }
}

impl YParser {
    fn error(&self, msg: &str) -> JacksonError {
        JacksonError { class: "com.fasterxml.jackson.core.JsonParseException".into(), message: format!("{msg}\n at {}", location(&self.last_end)) }
    }

    fn wrap_yaml(&self, e: YamlError) -> JacksonError {
        let class = if matches!(e.kind, snakeyaml::error::YamlErrorKind::Marked(_)) {
            "com.fasterxml.jackson.dataformat.yaml.snakeyaml.error.MarkedYAMLException"
        } else {
            "com.fasterxml.jackson.dataformat.yaml.JacksonYAMLParseException"
        };
        JacksonError { class: class.into(), message: format!("{}\n at {}", e.message(), location(&self.last_end)) }
    }

    fn next_event(&mut self) -> Result<Option<Event>, JacksonError> {
        match self.parser.get_event() {
            Ok(e) => Ok(e),
            Err(e) => Err(self.wrap_yaml(e)),
        }
    }

    /// `nextToken()`; `in_object` tells whether the innermost open container is an object.
    fn next_token(&mut self, in_object: bool, in_array: bool) -> Result<Option<Tok>, JacksonError> {
        loop {
            let evt = match self.next_event()? {
                None => return Ok(None),
                Some(e) => e,
            };
            self.last_end = evt.end_mark.clone();
            if in_object && !self.after_field {
                match &evt.kind {
                    EventKind::Scalar { value, .. } => {
                        self.after_field = true;
                        return Ok(Some(Tok::Field(value.clone())));
                    }
                    EventKind::MappingEnd => return Ok(Some(Tok::EndObject)),
                    _ => return Err(self.error(&format!("Expected a field name (Scalar value in YAML), got this instead: {}", event_to_string(&evt)))),
                }
            }
            self.after_field = false;
            let _ = in_array;
            match &evt.kind {
                EventKind::Scalar { .. } => return Ok(Some(Tok::Value(self.decode_scalar(&evt)?))),
                EventKind::MappingStart { .. } => {
                    self.depth += 1;
                    if self.depth > MAX_NESTING_DEPTH {
                        return Err(nesting_error(self.depth));
                    }
                    return Ok(Some(Tok::StartObject));
                }
                EventKind::MappingEnd => return Err(self.error("Not expecting END_OBJECT but a value")),
                EventKind::SequenceStart { .. } => {
                    self.depth += 1;
                    if self.depth > MAX_NESTING_DEPTH {
                        return Err(nesting_error(self.depth));
                    }
                    return Ok(Some(Tok::StartArray));
                }
                EventKind::SequenceEnd => return Ok(Some(Tok::EndArray)),
                EventKind::Alias { anchor } => return Ok(Some(Tok::Value(JsonNode::Text(anchor.as_str().into())))),
                EventKind::StreamEnd => return Ok(None),
                _ => continue,
            }
        }
    }

    fn decode_scalar(&mut self, evt: &Event) -> Result<JsonNode, JacksonError> {
        let (tag, implicit, value) = match &evt.kind {
            EventKind::Scalar { tag, implicit, value, .. } => (tag, implicit, value),
            _ => unreachable!(),
        };
        if !self.features.empty_string_as_null && value.is_empty() {
            return Ok(JsonNode::Text("".into()));
        }
        let len = value.encode_utf16().count();
        match tag {
            Some(t) if t != "!" => {
                let mut type_tag = t.as_str();
                let stripped;
                if let Some(rest) = type_tag.strip_prefix(TAG_PREFIX) {
                    stripped = rest.split(',').next().unwrap_or("").to_string();
                    type_tag = &stripped;
                }
                if type_tag == "binary" {
                    return match crate::base64::decode_mime(value.trim()) {
                        Ok(b) => Ok(JsonNode::Binary(b.into())),
                        Err(m) => Err(self.error(&m)),
                    };
                }
                if type_tag == "bool" {
                    if let Some(b) = match_yaml_boolean(value) {
                        return Ok(JsonNode::Boolean(b));
                    }
                } else if len > 0 {
                    if type_tag == "int" {
                        return self.decode_number_scalar(value);
                    }
                    if type_tag == "float" {
                        return self.float_node(value);
                    }
                    if type_tag == "null" {
                        return Ok(JsonNode::Null);
                    }
                }
                Ok(JsonNode::Text(value.as_str().into()))
            }
            _ => {
                let node_tag = resolver().resolve(NodeId::Scalar, value, implicit.plain);
                match node_tag {
                    TAG_STR => Ok(JsonNode::Text(value.as_str().into())),
                    TAG_INT => self.decode_number_scalar(value),
                    TAG_FLOAT => self.float_node(value),
                    TAG_BOOL => match match_yaml_boolean(value) {
                        Some(b) => Ok(JsonNode::Boolean(b)),
                        None => Ok(JsonNode::Text(value.as_str().into())),
                    },
                    TAG_NULL => Ok(JsonNode::Null),
                    _ => Ok(JsonNode::Text(value.as_str().into())),
                }
            }
        }
    }

    fn float_node(&self, value: &str) -> Result<JsonNode, JacksonError> {
        let cleaned: String = if value.contains('_') && !value.is_empty() {
            let skip = if value.starts_with('+') { 1 } else { 0 };
            value.chars().skip(skip).filter(|c| *c != '_').collect()
        } else {
            value.to_string()
        };
        match crate::javanum::parse_double(&cleaned) {
            Some(d) => Ok(JsonNode::Double(d)),
            None => Err(self.error(&format!("Malformed numeric value '{value}'"))),
        }
    }

    fn decode_number_scalar(&self, value: &str) -> Result<JsonNode, JacksonError> {
        let chars: Vec<char> = value.chars().collect();
        let len = chars.len();
        let (negative, mut i) = match chars[0] {
            '-' => (true, 1),
            '+' => {
                if len == 1 {
                    return Ok(JsonNode::Null);
                }
                (false, 1)
            }
            _ => (false, 0),
        };
        if len == i {
            return Ok(JsonNode::Null);
        }
        if chars[i] == '0' {
            i += 1;
            if i == len {
                return Ok(JsonNode::Int(0));
            }
            let rest: String = chars[i..].iter().collect();
            return match chars[i] {
                '0'..='9' | '_' => self.decode_radix(&rest, 8, negative, 10, 21, false),
                'b' | 'B' => self.decode_radix(&chars[i + 1..].iter().collect::<String>(), 2, negative, 31, 63, true),
                'x' | 'X' => self.decode_radix(&chars[i + 1..].iter().collect::<String>(), 16, negative, 7, 15, true),
                _ => Ok(JsonNode::Text(value.into())),
            };
        }
        let mut underscores = false;
        for &c in &chars[i..] {
            if !c.is_ascii_digit() {
                if c != '_' {
                    return Ok(JsonNode::Text(value.into()));
                }
                underscores = true;
            }
        }
        let cleaned: String = if underscores {
            let skip = if chars[0] == '+' { 1 } else { 0 };
            let c: String = chars.iter().skip(skip).filter(|c| **c != '_').collect();
            if c.is_empty() || c == "-" {
                return Err(self.error(&format!("Invalid number ('{value}')")));
            }
            c
        } else {
            value.to_string()
        };
        // _parseNumericValue
        let mut dlen = cleaned.chars().count();
        if negative {
            dlen -= 1;
        }
        if dlen <= 9 {
            return match cleaned.parse::<i32>() {
                Ok(v) => Ok(JsonNode::Int(v)),
                Err(_) => Err(self.error(&format!("Invalid base-10 number ('{cleaned}'), problem: For input string: \"{cleaned}\""))),
            };
        }
        if dlen <= 18 {
            let l: i64 = cleaned.parse().unwrap();
            if dlen == 10 && l >= i32::MIN as i64 && l <= i32::MAX as i64 {
                return Ok(JsonNode::Int(l as i32));
            }
            return Ok(JsonNode::Long(l));
        }
        let n: BigInt = cleaned.trim_start_matches('+').parse().unwrap();
        if dlen == 19 && n.bits() <= 63 {
            return Ok(JsonNode::Long(i64::try_from(&n).unwrap()));
        }
        Ok(JsonNode::BigInteger(Arc::new(n)))
    }

    /// `_decodeNumberIntOctal` / `Binary` / `Hex`
    fn decode_radix(&self, digits: &str, radix: u32, negative: bool, int_max: usize, long_max: usize, check_if_int_at_boundary: bool) -> Result<JsonNode, JacksonError> {
        let cleansed: String = digits.chars().filter(|c| *c != '_').collect();
        let n = cleansed.chars().count();
        let invalid = |s: &str| -> JacksonError {
            let problem = if radix == 10 { format!("For input string: \"{s}\"") } else { format!("For input string: \"{s}\" under radix {radix}") };
            self.error(&format!("Invalid base-{radix} number ('{s}'), problem: {problem}"))
        };
        if n <= int_max {
            let v = i32::from_str_radix(&cleansed, radix).map_err(|_| invalid(&cleansed))?;
            return Ok(JsonNode::Int(if negative { v.wrapping_neg() } else { v }));
        }
        if n <= long_max {
            let unsigned = i64::from_str_radix(&cleansed, radix).map_err(|_| invalid(&cleansed))?;
            let check = check_if_int_at_boundary && n == int_max + 1;
            if negative {
                let actual = unsigned.wrapping_neg();
                if check && actual >= i32::MIN as i64 {
                    return Ok(JsonNode::Int(actual as i32));
                }
                return Ok(JsonNode::Long(actual));
            }
            if check && unsigned < i32::MAX as i64 {
                return Ok(JsonNode::Int(unsigned as i32));
            }
            return Ok(JsonNode::Long(unsigned));
        }
        let b = BigInt::parse_bytes(cleansed.as_bytes(), radix).ok_or_else(|| invalid(&cleansed))?;
        Ok(JsonNode::BigInteger(Arc::new(if negative { -b } else { b })))
    }
}

fn nesting_error(depth: usize) -> JacksonError {
    JacksonError {
        class: "com.fasterxml.jackson.core.exc.StreamConstraintsException".into(),
        message: format!("Document nesting depth ({depth}) exceeds the maximum allowed ({MAX_NESTING_DEPTH}, from `StreamReadConstraints.getMaxNestingDepth()`)"),
    }
}

fn match_yaml_boolean(value: &str) -> Option<bool> {
    let lower = value.to_ascii_lowercase();
    match value.chars().count() {
        1 => match value.chars().next().unwrap() {
            'N' | 'n' => Some(false),
            'Y' | 'y' => Some(true),
            _ => None,
        },
        2 if lower == "no" => Some(false),
        2 if lower == "on" => Some(true),
        3 if lower == "yes" => Some(true),
        3 if lower == "off" => Some(false),
        4 if lower == "true" => Some(true),
        5 if lower == "false" => Some(false),
        _ => None,
    }
}

/// `Event.toString()`
pub fn event_to_string(e: &Event) -> String {
    let opt = |s: &Option<String>| s.clone().unwrap_or_else(|| "null".into());
    match &e.kind {
        EventKind::SequenceStart { anchor, tag, implicit, .. } => {
            format!("<org.yaml.snakeyaml.events.SequenceStartEvent(anchor={}, tag={}, implicit={implicit})>", opt(anchor), opt(tag))
        }
        EventKind::MappingStart { anchor, tag, implicit, .. } => {
            format!("<org.yaml.snakeyaml.events.MappingStartEvent(anchor={}, tag={}, implicit={implicit})>", opt(anchor), opt(tag))
        }
        EventKind::Alias { anchor } => format!("<org.yaml.snakeyaml.events.AliasEvent(anchor={anchor})>"),
        EventKind::SequenceEnd => "<org.yaml.snakeyaml.events.SequenceEndEvent()>".into(),
        EventKind::MappingEnd => "<org.yaml.snakeyaml.events.MappingEndEvent()>".into(),
        EventKind::Scalar { anchor, tag, implicit, value, style } => format!(
            "<org.yaml.snakeyaml.events.ScalarEvent(anchor={}, tag={}, style=Scalar style: '{}',org.yaml.snakeyaml.events.ImplicitTuple@?, value={value})>",
            opt(anchor),
            opt(tag),
            style.char().map(|c| c.to_string()).unwrap_or_else(|| "null".into()),
        )
        .replace("ImplicitTuple@?", &format!("ImplicitTuple(plain={}, nonPlain={})", implicit.plain, implicit.non_plain)),
        EventKind::DocumentStart { .. } => "<org.yaml.snakeyaml.events.DocumentStartEvent()>".into(),
        EventKind::DocumentEnd { .. } => "<org.yaml.snakeyaml.events.DocumentEndEvent()>".into(),
        EventKind::StreamStart => "<org.yaml.snakeyaml.events.StreamStartEvent()>".into(),
        EventKind::StreamEnd => "<org.yaml.snakeyaml.events.StreamEndEvent()>".into(),
    }
}

/// Reads the first YAML document into a tree (`MissingNode` for empty content).
pub fn read_tree(text: &str, features: YamlReadFeatures) -> Result<JsonNode, JacksonError> {
    let parser = Parser::from_reader(StreamReader::from_reader_input(text), LoaderOptions::default());
    let mut p = YParser { parser, last_end: None, features, after_field: false, depth: 0 };
    let mut stack: Vec<Frame> = Vec::new();
    loop {
        let (in_object, in_array) = match stack.last() {
            Some(Frame::Obj(..)) => (true, false),
            Some(Frame::Arr(_)) => (false, true),
            None => (false, false),
        };
        let tok = match p.next_token(in_object, in_array)? {
            None => return Ok(JsonNode::Missing),
            Some(t) => t,
        };
        let value = match tok {
            Tok::StartObject => {
                let names = if features.strict_duplicate_detection { Some(HashSet::new()) } else { None };
                stack.push(Frame::Obj(ObjectNode::new(), None, names));
                continue;
            }
            Tok::StartArray => {
                stack.push(Frame::Arr(Vec::new()));
                continue;
            }
            Tok::Field(name) => {
                if let Some(Frame::Obj(_, n, names)) = stack.last_mut() {
                    if let Some(set) = names {
                        if !set.insert(name.clone()) {
                            return Err(p.error(&format!("Duplicate field '{name}'")));
                        }
                    }
                    *n = Some(name);
                }
                continue;
            }
            Tok::EndObject => {
                p.depth -= 1;
                match stack.pop() {
                    Some(Frame::Obj(m, ..)) => JsonNode::Object(Arc::new(m)),
                    _ => unreachable!(),
                }
            }
            Tok::EndArray => {
                p.depth -= 1;
                match stack.pop() {
                    Some(Frame::Arr(a)) => JsonNode::Array(Arc::new(a)),
                    _ => return Err(p.error("Unexpected close marker ']': expected '}'")),
                }
            }
            Tok::Value(v) => v,
        };
        match stack.last_mut() {
            None => return Ok(value),
            Some(Frame::Arr(a)) => a.push(value),
            Some(Frame::Obj(m, n, _)) => {
                m.insert(n.take().unwrap_or_default(), value);
            }
        }
    }
}
