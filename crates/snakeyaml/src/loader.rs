//! `Composer` (events → nodes), `SafeConstructor` (nodes → Java objects) and the `Yaml` loading facade.

use crate::error::{COMPOSER_EXCEPTION, CONSTRUCTOR_EXCEPTION, DUPLICATE_KEY_EXCEPTION, Mark, Result, YamlError};
use crate::events::EventKind;
use crate::nodes::*;
use crate::object::{YamlList, YamlMap, YamlObject};
use crate::options::{FlowStyle, LoaderOptions};
use crate::parser::Parser;
use crate::resolver::Resolver;
use num_bigint::BigInt;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};

/// `org.yaml.snakeyaml.composer.Composer`: builds the node graph of a document from parser events, resolving implicit tags and aliases.
pub struct Composer<'a> {
    parser: Parser,
    resolver: &'a Resolver,
    options: &'a LoaderOptions,
    anchors: HashMap<String, Node>,
    recursive_nodes: HashSet<usize>,
    non_scalar_aliases_count: usize,
    nesting_depth: usize,
}

fn composer_err(context: Option<&str>, context_mark: Option<Mark>, problem: &str, problem_mark: Option<Mark>) -> YamlError {
    YamlError::marked(COMPOSER_EXCEPTION, context, context_mark, problem, problem_mark)
}

impl<'a> Composer<'a> {
    /// A composer parsing `input` (named `'string'`), resolving implicit tags with `resolver` and enforcing `options`' limits.
    pub fn new(input: &str, resolver: &'a Resolver, options: &'a LoaderOptions) -> Composer<'a> {
        Composer {
            parser: Parser::new(input, options.clone()),
            resolver,
            options,
            anchors: HashMap::new(),
            recursive_nodes: HashSet::new(),
            non_scalar_aliases_count: 0,
            nesting_depth: 0,
        }
    }

    fn check(&mut self, f: fn(&EventKind) -> bool) -> Result<bool> {
        Ok(self.parser.peek_event()?.is_some_and(|e| f(&e.kind)))
    }

    /// `getSingleNode()`
    pub fn get_single_node(&mut self) -> Result<Option<Node>> {
        self.parser.get_event()?; // stream start
        let mut document = None;
        if !self.check(|k| matches!(k, EventKind::StreamEnd))? {
            document = Some(self.get_node()?);
        }
        if !self.check(|k| matches!(k, EventKind::StreamEnd))? {
            let event = self.parser.get_event()?.expect("event");
            let context_mark = document.as_ref().map(|d| d.start_mark());
            return Err(composer_err(Some("expected a single document in the stream"), context_mark, "but found another document", event.start_mark));
        }
        self.parser.get_event()?;
        Ok(document)
    }

    fn get_node(&mut self) -> Result<Node> {
        self.parser.get_event()?; // document start
        let node = self.compose_node(None)?;
        self.parser.get_event()?; // document end
        self.anchors.clear();
        self.recursive_nodes.clear();
        Ok(node)
    }

    fn compose_node(&mut self, parent: Option<&Node>) -> Result<Node> {
        if let Some(p) = parent {
            self.recursive_nodes.insert(p.id());
        }
        let node;
        if self.check(|k| matches!(k, EventKind::Alias { .. }))? {
            let ev = self.parser.get_event()?.unwrap();
            let anchor = match &ev.kind {
                EventKind::Alias { anchor } => anchor.clone(),
                _ => unreachable!(),
            };
            let found = match self.anchors.get(&anchor) {
                Some(n) => n.clone(),
                None => return Err(composer_err(None, None, &format!("found undefined alias {anchor}"), ev.start_mark)),
            };
            if found.node_id() != NodeId::Scalar {
                self.non_scalar_aliases_count += 1;
                if self.non_scalar_aliases_count > self.options.max_aliases_for_collections {
                    return Err(YamlError::plain(format!(
                        "Number of aliases for non-scalar nodes exceeds the specified max={}",
                        self.options.max_aliases_for_collections
                    )));
                }
            }
            if self.recursive_nodes.remove(&found.id()) {
                found.0.write().two_steps_construction = true;
            }
            node = found;
        } else {
            let anchor = match &self.parser.peek_event()?.unwrap().kind {
                EventKind::Scalar { anchor, .. } | EventKind::SequenceStart { anchor, .. } | EventKind::MappingStart { anchor, .. } => anchor.clone(),
                _ => None,
            };
            if self.nesting_depth > self.options.nesting_depth_limit {
                return Err(YamlError::plain(format!("Nesting Depth exceeded max {}", self.options.nesting_depth_limit)));
            }
            self.nesting_depth += 1;
            node = if self.check(|k| matches!(k, EventKind::Scalar { .. }))? {
                self.compose_scalar(anchor)?
            } else if self.check(|k| matches!(k, EventKind::SequenceStart { .. }))? {
                self.compose_sequence(anchor)?
            } else {
                self.compose_mapping(anchor)?
            };
            self.nesting_depth -= 1;
        }
        if let Some(p) = parent {
            self.recursive_nodes.remove(&p.id());
        }
        Ok(node)
    }

    fn node_tag(&self, tag: &Option<String>, kind: NodeId, value: &str, implicit: bool, mark: &Mark) -> Result<(String, bool)> {
        match tag {
            Some(t) if t != "!" => {
                if is_custom_global(t) {
                    return Err(composer_err(None, None, &format!("Global tag is not allowed: {t}"), Some(mark.clone())));
                }
                Ok((t.clone(), false))
            }
            _ => Ok((self.resolver.resolve(kind, value, implicit).to_string(), true)),
        }
    }

    fn compose_scalar(&mut self, anchor: Option<String>) -> Result<Node> {
        let ev = self.parser.get_event()?.unwrap();
        let (tag, implicit, value, style) = match ev.kind {
            EventKind::Scalar { tag, implicit, value, style, .. } => (tag, implicit, value, style),
            _ => unreachable!(),
        };
        let start = ev.start_mark.unwrap();
        let (tag, resolved) = self.node_tag(&tag, NodeId::Scalar, &value, implicit.plain, &start)?;
        let node = Node::new(NodeData {
            tag,
            resolved,
            value: NodeValue::Scalar { value, style },
            start_mark: start,
            end_mark: ev.end_mark,
            flow_style: FlowStyle::Auto,
            anchor: anchor.clone(),
            two_steps_construction: false,
            merged: false,
        });
        if let Some(a) = anchor {
            self.anchors.insert(a, node.clone());
        }
        Ok(node)
    }

    fn compose_sequence(&mut self, anchor: Option<String>) -> Result<Node> {
        let ev = self.parser.get_event()?.unwrap();
        let (tag, implicit, flow_style) = match ev.kind {
            EventKind::SequenceStart { tag, implicit, flow_style, .. } => (tag, implicit, flow_style),
            _ => unreachable!(),
        };
        let start = ev.start_mark.unwrap();
        let (tag, resolved) = self.node_tag(&tag, NodeId::Sequence, "", implicit, &start)?;
        let node = Node::new(NodeData {
            tag,
            resolved,
            value: NodeValue::Sequence(Vec::new()),
            start_mark: start,
            end_mark: None,
            flow_style,
            anchor: anchor.clone(),
            two_steps_construction: false,
            merged: false,
        });
        if let Some(a) = anchor {
            self.anchors.insert(a, node.clone());
        }
        while !self.check(|k| matches!(k, EventKind::SequenceEnd))? {
            let child = self.compose_node(Some(&node))?;
            if let NodeValue::Sequence(v) = &mut node.0.write().value {
                v.push(child);
            }
        }
        let end = self.parser.get_event()?.unwrap();
        node.0.write().end_mark = end.end_mark;
        Ok(node)
    }

    fn compose_mapping(&mut self, anchor: Option<String>) -> Result<Node> {
        let ev = self.parser.get_event()?.unwrap();
        let (tag, implicit, flow_style) = match ev.kind {
            EventKind::MappingStart { tag, implicit, flow_style, .. } => (tag, implicit, flow_style),
            _ => unreachable!(),
        };
        let start = ev.start_mark.unwrap();
        let (tag, resolved) = self.node_tag(&tag, NodeId::Mapping, "", implicit, &start)?;
        let node = Node::new(NodeData {
            tag,
            resolved,
            value: NodeValue::Mapping(Vec::new()),
            start_mark: start,
            end_mark: None,
            flow_style,
            anchor: anchor.clone(),
            two_steps_construction: false,
            merged: false,
        });
        if let Some(a) = anchor {
            self.anchors.insert(a, node.clone());
        }
        while !self.check(|k| matches!(k, EventKind::MappingEnd))? {
            let key = self.compose_node(Some(&node))?;
            if key.tag() == TAG_MERGE {
                node.0.write().merged = true;
            }
            let value = self.compose_node(Some(&node))?;
            if let NodeValue::Mapping(v) = &mut node.0.write().value {
                v.push((key, value));
            }
        }
        let end = self.parser.get_event()?.unwrap();
        node.0.write().end_mark = end.end_mark;
        Ok(node)
    }
}

fn constructor_err(context: Option<&str>, context_mark: Option<Mark>, problem: &str, problem_mark: Mark) -> YamlError {
    YamlError::marked(CONSTRUCTOR_EXCEPTION, context, context_mark, problem, Some(problem_mark))
}

/// `SafeConstructor`
pub struct SafeConstructor<'a> {
    options: &'a LoaderOptions,
    constructed: HashMap<usize, YamlObject>,
    recursive: HashSet<usize>,
    maps2fill: Vec<(YamlMap, YamlObject, YamlObject)>,
    sets2fill: Vec<(YamlList, YamlObject)>,
}

fn runtime_err(class: &'static str, msg: String) -> YamlError {
    YamlError { class, kind: crate::error::YamlErrorKind::Plain(msg) }
}

impl<'a> SafeConstructor<'a> {
    /// A constructor with no objects built yet, enforcing `options` (duplicate keys, recursive keys).
    pub fn new(options: &'a LoaderOptions) -> SafeConstructor<'a> {
        SafeConstructor { options, constructed: HashMap::new(), recursive: HashSet::new(), maps2fill: Vec::new(), sets2fill: Vec::new() }
    }

    /// `constructDocument(node)`
    pub fn construct_document(&mut self, node: &Node) -> Result<YamlObject> {
        let r = self.construct_object(node);
        let r = r.inspect(|_| self.fill_recursive());
        self.constructed.clear();
        self.recursive.clear();
        r
    }

    fn fill_recursive(&mut self) {
        for (m, k, v) in self.maps2fill.drain(..) {
            m.put(k, v);
        }
        for (s, v) in self.sets2fill.drain(..) {
            s.add_to_set(v);
        }
    }

    fn construct_object(&mut self, node: &Node) -> Result<YamlObject> {
        if let Some(o) = self.constructed.get(&node.id()) {
            return Ok(o.clone());
        }
        if self.recursive.contains(&node.id()) {
            return Err(constructor_err(None, None, "found unconstructable recursive node", node.start_mark()));
        }
        self.recursive.insert(node.id());
        let data = self.construct_first_step(node)?;
        self.constructed.insert(node.id(), data.clone());
        self.recursive.remove(&node.id());
        if node.is_two_steps_construction() {
            self.construct_second_step(node, &data)?;
        }
        Ok(data)
    }

    fn construct_first_step(&mut self, node: &Node) -> Result<YamlObject> {
        let tag = node.tag();
        let two_steps = node.is_two_steps_construction();
        let scalar = || node.scalar_value().ok_or_else(|| class_cast(node, "ScalarNode"));
        match tag.as_str() {
            TAG_NULL => Ok(YamlObject::Null),
            TAG_BOOL => {
                let v = scalar()?;
                Ok(match v.to_lowercase().as_str() {
                    "yes" | "true" | "on" => YamlObject::Boolean(true),
                    "no" | "false" | "off" => YamlObject::Boolean(false),
                    _ => YamlObject::Null,
                })
            }
            TAG_INT => construct_int(node, &scalar()?),
            TAG_FLOAT => construct_float(node, &scalar()?),
            TAG_BINARY => {
                let v: String = scalar()?.chars().filter(|c| !matches!(c, ' ' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\r')).collect();
                decode_base64(&v).map(|b| YamlObject::Bytes(b.into()))
            }
            TAG_TIMESTAMP => construct_timestamp(&scalar()?),
            TAG_STR => Ok(YamlObject::String(scalar()?.into())),
            TAG_SEQ => {
                if node.node_id() != NodeId::Sequence {
                    return Err(class_cast(node, "SequenceNode"));
                }
                let list = YamlList::new();
                if !two_steps {
                    for child in node.sequence_children() {
                        let v = self.construct_object(&child)?;
                        list.push(v);
                    }
                }
                Ok(YamlObject::List(list))
            }
            TAG_MAP => {
                if node.node_id() != NodeId::Mapping {
                    return Err(class_cast(node, "MappingNode"));
                }
                let map = YamlMap::new();
                if !two_steps {
                    self.construct_mapping_2nd_step(node, &map)?;
                }
                Ok(YamlObject::Map(map))
            }
            TAG_SET => {
                if node.node_id() != NodeId::Mapping {
                    return Err(class_cast(node, "MappingNode"));
                }
                let set = YamlList::new();
                if !two_steps {
                    self.construct_set_2nd_step(node, &set)?;
                }
                Ok(YamlObject::Set(set))
            }
            TAG_OMAP => {
                let map = YamlMap::new();
                if node.node_id() != NodeId::Sequence {
                    return Err(constructor_err(Some("while constructing an ordered map"), Some(node.start_mark()), &format!("expected a sequence, but found {}", node.node_id()), node.start_mark()));
                }
                for sub in node.sequence_children() {
                    if sub.node_id() != NodeId::Mapping {
                        return Err(constructor_err(Some("while constructing an ordered map"), Some(node.start_mark()), &format!("expected a mapping of length 1, but found {}", sub.node_id()), sub.start_mark()));
                    }
                    let kids = sub.mapping_children();
                    if kids.len() != 1 {
                        return Err(constructor_err(Some("while constructing an ordered map"), Some(node.start_mark()), &format!("expected a single mapping item, but found {} items", kids.len()), sub.start_mark()));
                    }
                    let k = self.construct_object(&kids[0].0)?;
                    let v = self.construct_object(&kids[0].1)?;
                    map.put(k, v);
                }
                Ok(YamlObject::Map(map))
            }
            TAG_PAIRS => {
                if node.node_id() != NodeId::Sequence {
                    return Err(constructor_err(Some("while constructing pairs"), Some(node.start_mark()), &format!("expected a sequence, but found {}", node.node_id()), node.start_mark()));
                }
                let mut pairs = Vec::new();
                for sub in node.sequence_children() {
                    if sub.node_id() != NodeId::Mapping {
                        return Err(constructor_err(Some("while constructingpairs"), Some(node.start_mark()), &format!("expected a mapping of length 1, but found {}", sub.node_id()), sub.start_mark()));
                    }
                    let kids = sub.mapping_children();
                    if kids.len() != 1 {
                        return Err(constructor_err(Some("while constructing pairs"), Some(node.start_mark()), &format!("expected a single mapping item, but found {} items", kids.len()), sub.start_mark()));
                    }
                    let k = self.construct_object(&kids[0].0)?;
                    let v = self.construct_object(&kids[0].1)?;
                    pairs.push((k, v));
                }
                Ok(YamlObject::Pairs(Arc::new(pairs)))
            }
            other => Err(constructor_err(None, None, &format!("could not determine a constructor for the tag {other}"), node.start_mark())),
        }
    }

    fn construct_second_step(&mut self, node: &Node, data: &YamlObject) -> Result<()> {
        match data {
            YamlObject::List(l) if node.tag() == TAG_SEQ => {
                for child in node.sequence_children() {
                    let v = self.construct_object(&child)?;
                    l.push(v);
                }
                Ok(())
            }
            YamlObject::Map(m) if node.tag() == TAG_MAP => self.construct_mapping_2nd_step(node, m),
            YamlObject::Set(s) if node.tag() == TAG_SET => self.construct_set_2nd_step(node, s),
            _ => Err(YamlError::plain(format!("Unexpected recursive structure. Node: {}", node.to_java_string()))),
        }
    }

    fn flatten_mapping(&mut self, node: &Node) -> Result<()> {
        self.process_duplicate_keys(node)?;
        if node.0.read().merged {
            let merged = self.merge_node(node, true, &mut Vec::new(), &mut Vec::new())?;
            node.set_mapping_children(merged);
        }
        Ok(())
    }

    fn process_duplicate_keys(&mut self, node: &Node) -> Result<()> {
        let tuples = node.mapping_children();
        let mut keys: Vec<(YamlObject, usize)> = Vec::new();
        let mut to_remove: Vec<usize> = Vec::new();
        for (i, (key_node, _)) in tuples.iter().enumerate() {
            if key_node.tag() == TAG_MERGE {
                continue;
            }
            let key = self.construct_object(key_node)?;
            if !key.is_null() && key_node.is_two_steps_construction() && !self.options.allow_recursive_keys {
                return Err(YamlError::plain("Recursive key for mapping is detected but it is not configured to be allowed."));
            }
            match keys.iter_mut().find(|(k, _)| k.java_equals(&key)) {
                Some(entry) => {
                    let prev = entry.1;
                    entry.1 = i;
                    if !self.options.allow_duplicate_keys {
                        return Err(YamlError::marked(
                            DUPLICATE_KEY_EXCEPTION,
                            Some("while constructing a mapping"),
                            Some(node.start_mark()),
                            &format!("found duplicate key {}", key.to_java_string()),
                            Some(key_node.start_mark()),
                        ));
                    }
                    if !to_remove.contains(&prev) {
                        to_remove.push(prev);
                    }
                }
                None => keys.push((key, i)),
            }
        }
        if !to_remove.is_empty() {
            to_remove.sort_unstable();
            let mut kept = tuples.clone();
            for idx in to_remove.iter().rev() {
                kept.remove(*idx);
            }
            node.set_mapping_children(kept);
        }
        Ok(())
    }

    fn merge_node(&mut self, node: &Node, preferred: bool, key2index: &mut Vec<(YamlObject, usize)>, values: &mut Vec<(Node, Node)>) -> Result<Vec<(Node, Node)>> {
        let tuples = node.mapping_children();
        let mut remaining = Vec::new();
        for (key_node, value_node) in tuples {
            if key_node.tag() == TAG_MERGE {
                match value_node.node_id() {
                    NodeId::Mapping => {
                        self.merge_node(&value_node, false, key2index, values)?;
                    }
                    NodeId::Sequence => {
                        for sub in value_node.sequence_children() {
                            if sub.node_id() != NodeId::Mapping {
                                return Err(constructor_err(
                                    Some("while constructing a mapping"),
                                    Some(node.start_mark()),
                                    &format!("expected a mapping for merging, but found {}", sub.node_id()),
                                    sub.start_mark(),
                                ));
                            }
                            self.merge_node(&sub, false, key2index, values)?;
                        }
                    }
                    other => {
                        return Err(constructor_err(
                            Some("while constructing a mapping"),
                            Some(node.start_mark()),
                            &format!("expected a mapping or list of mappings for merging, but found {other}"),
                            value_node.start_mark(),
                        ));
                    }
                }
            } else {
                remaining.push((key_node.clone(), value_node.clone()));
                let key = self.construct_object(&key_node)?;
                match key2index.iter().find(|(k, _)| k.java_equals(&key)) {
                    None => {
                        values.push((key_node, value_node));
                        key2index.push((key, values.len() - 1));
                    }
                    Some((_, idx)) => {
                        if preferred {
                            values[*idx] = (key_node, value_node);
                        }
                    }
                }
            }
        }
        // iter.remove() of the merge tuples mutates the node itself
        node.set_mapping_children(remaining);
        Ok(values.clone())
    }

    fn construct_mapping_2nd_step(&mut self, node: &Node, map: &YamlMap) -> Result<()> {
        self.flatten_mapping(node)?;
        for (key_node, value_node) in node.mapping_children() {
            let key = self.construct_object(&key_node)?;
            let value = self.construct_object(&value_node)?;
            if key_node.is_two_steps_construction() {
                if !self.options.allow_recursive_keys {
                    return Err(YamlError::plain("Recursive key for mapping is detected but it is not configured to be allowed."));
                }
                self.maps2fill.insert(0, (map.clone(), key, value));
            } else {
                map.put(key, value);
            }
        }
        Ok(())
    }

    fn construct_set_2nd_step(&mut self, node: &Node, set: &YamlList) -> Result<()> {
        self.flatten_mapping(node)?;
        for (key_node, _) in node.mapping_children() {
            let key = self.construct_object(&key_node)?;
            if key_node.is_two_steps_construction() {
                self.sets2fill.insert(0, (set.clone(), key));
            } else {
                set.add_to_set(key);
            }
        }
        Ok(())
    }
}

fn class_cast(node: &Node, target: &str) -> YamlError {
    let actual = match node.node_id() {
        NodeId::Scalar => "ScalarNode",
        NodeId::Sequence => "SequenceNode",
        NodeId::Mapping => "MappingNode",
    };
    runtime_err(
        "java.lang.ClassCastException",
        format!(
            "class org.yaml.snakeyaml.nodes.{actual} cannot be cast to class org.yaml.snakeyaml.nodes.{target} (org.yaml.snakeyaml.nodes.{actual} and org.yaml.snakeyaml.nodes.{target} are in unnamed module of loader 'app')"
        ),
    )
}

fn radix_max(radix: u32) -> (usize, usize) {
    match radix {
        2 => (31, 63),
        8 => (11, 21),
        10 => (10, 19),
        16 => (8, 16),
        _ => (usize::MAX, usize::MAX),
    }
}

fn nfe(s: &str, radix: u32) -> YamlError {
    let msg = if radix == 10 { format!("For input string: \"{s}\"") } else { format!("For input string: \"{s}\" under radix {radix}") };
    runtime_err("java.lang.NumberFormatException", msg)
}

fn parse_big(s: &str, radix: u32) -> Result<BigInt> {
    BigInt::parse_bytes(s.as_bytes(), radix).ok_or_else(|| nfe(s, radix))
}

fn create_number(sign: i32, number: &str, radix: u32) -> Result<YamlObject> {
    let len = number.len();
    let n = if sign < 0 { format!("-{number}") } else { number.to_string() };
    let (max_int, max_long) = radix_max(radix);
    if len > max_int {
        if len > max_long {
            return parse_big(&n, radix).map(|b| YamlObject::BigInteger(Arc::new(b)));
        }
        return long_or_big(&n, radix);
    }
    match i32::from_str_radix(&n, radix) {
        Ok(v) => Ok(YamlObject::Integer(v)),
        Err(_) => long_or_big(&n, radix),
    }
}

fn long_or_big(n: &str, radix: u32) -> Result<YamlObject> {
    match i64::from_str_radix(n, radix) {
        Ok(v) => Ok(YamlObject::Long(v)),
        Err(_) => parse_big(n, radix).map(|b| YamlObject::BigInteger(Arc::new(b))),
    }
}

fn construct_int(node: &Node, raw: &str) -> Result<YamlObject> {
    let mut value = raw.replace('_', "");
    if value.is_empty() {
        return Err(constructor_err(Some("while constructing an int"), Some(node.start_mark()), "found empty value", node.start_mark()));
    }
    let mut sign = 1;
    if let Some(rest) = value.strip_prefix('-') {
        sign = -1;
        value = rest.to_string();
    } else if let Some(rest) = value.strip_prefix('+') {
        value = rest.to_string();
    }
    if value == "0" {
        return Ok(YamlObject::Integer(0));
    }
    if let Some(rest) = value.strip_prefix("0b") {
        return create_number(sign, rest, 2);
    }
    if let Some(rest) = value.strip_prefix("0x") {
        return create_number(sign, rest, 16);
    }
    if let Some(rest) = value.strip_prefix('0') {
        return create_number(sign, rest, 8);
    }
    if !value.contains(':') {
        return create_number(sign, &value, 10);
    }
    let digits: Vec<&str> = value.split(':').collect();
    let mut bes: i32 = 1;
    let mut val: i32 = 0;
    for d in digits.iter().rev() {
        let part: i64 = d.parse().map_err(|_| nfe(d, 10))?;
        val = (val as i64).wrapping_add(part.wrapping_mul(bes as i64)) as i32;
        bes = bes.wrapping_mul(60);
    }
    create_number(sign, &val.to_string(), 10)
}

fn construct_float(node: &Node, raw: &str) -> Result<YamlObject> {
    let mut value = raw.replace('_', "");
    if value.is_empty() {
        return Err(constructor_err(Some("while constructing a float"), Some(node.start_mark()), "found empty value", node.start_mark()));
    }
    let mut sign = 1.0f64;
    if let Some(rest) = value.strip_prefix('-') {
        sign = -1.0;
        value = rest.to_string();
    } else if let Some(rest) = value.strip_prefix('+') {
        value = rest.to_string();
    }
    let lower = value.to_lowercase();
    if lower == ".inf" {
        return Ok(YamlObject::Double(if sign < 0.0 { f64::NEG_INFINITY } else { f64::INFINITY }));
    }
    if lower == ".nan" {
        return Ok(YamlObject::Double(f64::NAN));
    }
    if !value.contains(':') {
        let d: f64 = java_parse_double(&value)?;
        return Ok(YamlObject::Double(d * sign));
    }
    let mut bes: i32 = 1;
    let mut val = 0.0f64;
    for d in value.split(':').collect::<Vec<_>>().iter().rev() {
        val += java_parse_double(d)? * bes as f64;
        bes = bes.wrapping_mul(60);
    }
    Ok(YamlObject::Double(sign * val))
}

/// `Double.parseDouble` for the digit forms the FLOAT resolver admits.
fn java_parse_double(s: &str) -> Result<f64> {
    let t = s.trim();
    let normalized = if t.ends_with('.') { format!("{t}0") } else { t.to_string() };
    let normalized = if normalized.starts_with('.') { format!("0{normalized}") } else { normalized };
    normalized.parse::<f64>().map_err(|_| {
        runtime_err("java.lang.NumberFormatException", if t.is_empty() { "empty String".into() } else { format!("For input string: \"{s}\"") })
    })
}

/// `Base64.getDecoder().decode(String)` (basic alphabet, padding optional).
fn decode_base64(s: &str) -> Result<Vec<u8>> {
    use base64::Engine;
    let engine = base64::engine::GeneralPurpose::new(
        &base64::alphabet::STANDARD,
        base64::engine::GeneralPurposeConfig::new().with_decode_padding_mode(base64::engine::DecodePaddingMode::Indifferent).with_decode_allow_trailing_bits(true),
    );
    for c in s.chars() {
        if !(c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=') {
            return Err(runtime_err("java.lang.IllegalArgumentException", format!("Illegal base64 character {:x}", c as u32 as u8)));
        }
    }
    engine.decode(s).map_err(|e| runtime_err("java.lang.IllegalArgumentException", e.to_string()))
}

fn ymd_regex() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\A([0-9][0-9][0-9][0-9])-([0-9][0-9]?)-([0-9][0-9]?)\z").unwrap())
}

fn timestamp_regex() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"\A([0-9][0-9][0-9][0-9])-([0-9][0-9]?)-([0-9][0-9]?)(?:(?:[Tt]|[ \t]+)([0-9][0-9]?):([0-9][0-9]):([0-9][0-9])(?:\.([0-9]*))?(?:[ \t]*(?:Z|([-+][0-9][0-9]?)(?::([0-9][0-9])?)?))?)?\z").unwrap()
    })
}

/// Lenient `GregorianCalendar` field arithmetic (proleptic Gregorian; the 1582 cutover is ignored): the date's
/// midnight plus `time_ms`, the time of day less the zone offset, in milliseconds.
fn lenient_epoch_millis(year: i64, month0: i64, day: i64, time_ms: i64) -> i64 {
    let y = year + month0.div_euclid(12);
    let m = month0.rem_euclid(12) + 1;
    let days = days_from_civil(y, m as u32, 1) + (day - 1);
    days * 86_400_000 + time_ms
}

/// The milliseconds a time of day adds to a date's midnight, less the zone offset (`lenient_epoch_millis`'s `time_ms`).
fn time_millis(hour: i64, minute: i64, second: i64, millis: i64, offset_ms: i64) -> i64 {
    hour * 3_600_000 + minute * 60_000 + second * 1000 + millis - offset_ms
}

pub(crate) fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let mp = if m > 2 { m - 3 } else { m + 9 } as u64;
    let doy = (153 * mp + 2) / 5 + d as u64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe as i64 - 719468
}

fn construct_timestamp(value: &str) -> Result<YamlObject> {
    if let Some(c) = ymd_regex().captures(value) {
        let y: i64 = c[1].parse().unwrap();
        let m: i64 = c[2].parse().unwrap();
        let d: i64 = c[3].parse().unwrap();
        return Ok(YamlObject::Date(lenient_epoch_millis(y, m - 1, d, 0)));
    }
    let c = match timestamp_regex().captures(value) {
        Some(c) => c,
        None => return Err(YamlError::plain(format!("Unexpected timestamp: {value}"))),
    };
    let y: i64 = c[1].parse().unwrap();
    let m: i64 = c[2].parse().unwrap();
    let d: i64 = c[3].parse().unwrap();
    let h: i64 = c.get(4).map(|x| x.as_str().parse().unwrap()).unwrap_or(0);
    let mi: i64 = c.get(5).map(|x| x.as_str().parse().unwrap()).unwrap_or(0);
    let mut seconds = c.get(6).map(|x| x.as_str().to_string()).unwrap_or_else(|| "0".into());
    if let Some(frac) = c.get(7) {
        seconds = format!("{seconds}.{}", frac.as_str());
    }
    let fractions: f64 = java_parse_double(&seconds)?;
    let sec = fractions.floor() as i64;
    let usec = ((fractions - sec as f64) * 1000.0).round() as i64;
    let offset_ms = match c.get(8) {
        Some(hs) => {
            let hs = hs.as_str();
            let neg = hs.starts_with('-');
            let hours: i64 = hs[1..].parse().unwrap_or(0);
            let minutes: i64 = c.get(9).map(|x| x.as_str().parse().unwrap_or(0)).unwrap_or(0);
            let total = hours * 3_600_000 + minutes * 60_000;
            if hours > 23 || minutes > 59 { 0 } else if neg { -total } else { total }
        }
        None => 0,
    };
    Ok(YamlObject::Date(lenient_epoch_millis(y, m - 1, d, time_millis(h, mi, sec, usec, offset_ms))))
}

/// `new Yaml(new SafeConstructor(loaderOptions), …, resolver)` loading API.
#[derive(Clone, Default)]
pub struct Yaml {
    /// The loader settings used by both composing and constructing.
    pub options: LoaderOptions,
    /// The implicit tag resolver applied to plain scalars.
    pub resolver: Resolver,
}

impl Yaml {
    /// A loader with the given settings and resolver.
    pub fn new(options: LoaderOptions, resolver: Resolver) -> Yaml {
        Yaml { options, resolver }
    }

    /// `composer.getSingleNode()`
    pub fn compose(&self, input: &str) -> Result<Option<Node>> {
        Composer::new(input, &self.resolver, &self.options).get_single_node()
    }

    /// `constructDocument(node)` (null document → null).
    pub fn construct(&self, node: Option<&Node>) -> Result<YamlObject> {
        match node {
            None => Ok(YamlObject::Null),
            Some(n) => SafeConstructor::new(&self.options).construct_document(n),
        }
    }

    /// `yaml.load(String)` = `BaseConstructor.getSingleData(Object.class)`.
    pub fn load(&self, input: &str) -> Result<YamlObject> {
        let node = self.compose(input)?;
        match &node {
            Some(n) if n.tag() != TAG_NULL => self.construct(Some(n)),
            _ => Ok(YamlObject::Null),
        }
    }
}
