//! `Resolver`: implicit tag resolution of plain scalars.

use crate::nodes::*;
use regex::Regex;
use std::sync::OnceLock;

/// `Resolver.BOOL`: the YAML 1.1 booleans (`yes`/`no`/`true`/`false`/`on`/`off` in three casings).
pub const BOOL: &str = r"^(?:yes|Yes|YES|no|No|NO|true|True|TRUE|false|False|FALSE|on|On|ON|off|Off|OFF)$";
/// `Resolver.FLOAT`: YAML 1.1 floats, including sexagesimal, `.inf` and `.nan`.
pub const FLOAT: &str = r"^([-+]?(?:[0-9][0-9_]*)\.[0-9_]*(?:[eE][-+]?[0-9]+)?|[-+]?(?:[0-9][0-9_]*)(?:[eE][-+]?[0-9]+)|[-+]?\.[0-9_]+(?:[eE][-+]?[0-9]+)?|[-+]?[0-9][0-9_]*(?::[0-5]?[0-9])+\.[0-9_]*|[-+]?\.(?:inf|Inf|INF)|\.(?:nan|NaN|NAN))$";
/// `Resolver.INT`: YAML 1.1 integers in binary, octal, decimal, hex and sexagesimal.
pub const INT: &str = r"^(?:[-+]?0b_*[0-1][0-1_]*|[-+]?0_*[0-7][0-7_]*|[-+]?(?:0|[1-9][0-9_]*)|[-+]?0x_*[0-9a-fA-F][0-9a-fA-F_]*|[-+]?[1-9][0-9_]*(?::[0-5]?[0-9])+)$";
/// `Resolver.MERGE`: the `<<` merge key.
pub const MERGE: &str = r"^(?:<<)$";
/// `Resolver.NULL`: `~`, `null` in three casings, or a single space.
pub const NULL: &str = r"^(?:~|null|Null|NULL| )$";
/// `Resolver.EMPTY`: the empty string (resolves to null).
pub const EMPTY: &str = r"^$";
/// `Resolver.TIMESTAMP`: a date, or a date and time with optional fraction and zone.
pub const TIMESTAMP: &str = r"^(?:[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]|[0-9][0-9][0-9][0-9]-[0-9][0-9]?-[0-9][0-9]?(?:[Tt]|[ \t]+)[0-9][0-9]?:[0-9][0-9]:[0-9][0-9](?:\.[0-9]*)?(?:[ \t]*(?:Z|[-+][0-9][0-9]?(?::[0-9][0-9])?))?)$";
/// `Resolver.YAML`: the `!`, `&` and `*` indicators on their own.
pub const YAML: &str = r"^(?:!|&|\*)$";

fn compiled(p: &'static str) -> &'static Regex {
    static CACHE: OnceLock<parking_lot::Mutex<Vec<(&'static str, &'static Regex)>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| parking_lot::Mutex::new(Vec::new()));
    let mut c = cache.lock();
    if let Some((_, r)) = c.iter().find(|(k, _)| std::ptr::eq(*k, p)) {
        return r;
    }
    // Java `$` before a trailing newline is irrelevant under matches(); anchor strictly.
    let strict = format!(r"\A(?:{})\z", &p[1..p.len() - 1]);
    let r: &'static Regex = Box::leak(Box::new(Regex::new(&strict).unwrap()));
    c.push((p, r));
    r
}

#[derive(Clone)]
struct Tuple {
    tag: &'static str,
    regex: &'static Regex,
    limit: usize,
}

/// A resolver: first-character → ordered candidates (`None` key = fallback list), as `Resolver` builds it.
#[derive(Clone)]
pub struct Resolver {
    by_first: Vec<(Option<u16>, Vec<Tuple>)>,
}

impl Default for Resolver {
    fn default() -> Self {
        let mut r = Resolver::empty();
        r.add(TAG_BOOL, BOOL, Some("yYnNtTfFoO"), 10);
        r.add(TAG_INT, INT, Some("-+0123456789"), 1024);
        r.add(TAG_FLOAT, FLOAT, Some("-+0123456789."), 1024);
        r.add(TAG_MERGE, MERGE, Some("<"), 10);
        r.add(TAG_NULL, NULL, Some("~nN\0"), 10);
        r.add(TAG_NULL, EMPTY, None, 10);
        r.add(TAG_TIMESTAMP, TIMESTAMP, Some("0123456789"), 50);
        r.add(TAG_YAML, YAML, Some("!&*"), 10);
        r
    }
}

impl Resolver {
    /// A resolver with no implicit resolvers: every scalar resolves to `!!str`.
    pub fn empty() -> Resolver {
        Resolver { by_first: Vec::new() }
    }

    /// swagger-parser's `DeserializationUtils.CustomResolver` (no timestamps; default 1024 limits).
    pub fn swagger_custom() -> Resolver {
        let mut r = Resolver::empty();
        r.add(TAG_BOOL, BOOL, Some("yYnNtTfFoO"), 1024);
        r.add(TAG_INT, INT, Some("-+0123456789"), 1024);
        r.add(TAG_FLOAT, FLOAT, Some("-+0123456789."), 1024);
        r.add(TAG_MERGE, MERGE, Some("<"), 1024);
        r.add(TAG_NULL, NULL, Some("~nN\0"), 1024);
        r.add(TAG_NULL, EMPTY, None, 1024);
        r
    }

    /// `addImplicitResolver(tag, regexp, first, limit)`; a `\0` in `first` registers under the fallback key.
    pub fn add(&mut self, tag: &'static str, pattern: &'static str, first: Option<&str>, limit: usize) {
        let t = Tuple { tag, regex: compiled(pattern), limit };
        let keys: Vec<Option<u16>> = match first {
            None => vec![None],
            Some(f) => f.encode_utf16().map(|c| if c == 0 { None } else { Some(c) }).collect(),
        };
        for k in keys {
            match self.by_first.iter_mut().find(|(key, _)| *key == k) {
                Some((_, list)) => list.push(t.clone()),
                None => self.by_first.push((k, vec![t.clone()])),
            }
        }
    }

    fn list(&self, key: Option<u16>) -> Option<&Vec<Tuple>> {
        self.by_first.iter().find(|(k, _)| *k == key).map(|(_, l)| l)
    }

    /// `resolve(kind, value, implicit)`: the tag of the first implicit resolver matching a plain scalar (when `implicit`), else `!!str`, `!!seq` or `!!map` by node kind.
    pub fn resolve(&self, kind: NodeId, value: &str, implicit: bool) -> &'static str {
        if kind == NodeId::Scalar && implicit {
            let len = value.encode_utf16().count();
            let first = if value.is_empty() { Some(0u16) } else { value.encode_utf16().next() };
            if let Some(list) = self.list(first) {
                for t in list {
                    if len <= t.limit && t.regex.is_match(value) {
                        return t.tag;
                    }
                }
            }
            if let Some(list) = self.list(None) {
                for t in list {
                    if len <= t.limit && t.regex.is_match(value) {
                        return t.tag;
                    }
                }
            }
        }
        match kind {
            NodeId::Scalar => TAG_STR,
            NodeId::Sequence => TAG_SEQ,
            NodeId::Mapping => TAG_MAP,
        }
    }
}
