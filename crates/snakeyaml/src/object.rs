//! The Java object graph `SafeConstructor` builds: `LinkedHashMap`, `ArrayList`, `LinkedHashSet`, boxed
//! scalars, `byte[]`, `java.util.Date`. Containers have identity and may form cycles.

use num_bigint::BigInt;
use parking_lot::RwLock;
use std::sync::Arc;

/// A Java object as SnakeYAML's `SafeConstructor` builds it from a node.
#[derive(Clone, Debug)]
pub enum YamlObject {
    /// Java `null`.
    Null,
    /// `java.lang.Boolean`.
    Boolean(bool),
    /// `java.lang.Integer`: an `!!int` that fits in 32 bits.
    Integer(i32),
    /// `java.lang.Long`: an `!!int` that fits in 64 bits but not 32.
    Long(i64),
    /// `java.math.BigInteger`: an `!!int` too large for a `Long`.
    BigInteger(Arc<BigInt>),
    /// `java.lang.Double` (from `!!float`).
    Double(f64),
    /// `java.lang.String`.
    String(Arc<str>),
    /// `byte[]` (from `!!binary`)
    Bytes(Arc<[u8]>),
    /// `java.util.Date` as epoch milliseconds (from `!!timestamp`)
    Date(i64),
    /// `LinkedHashMap<Object, Object>`
    Map(YamlMap),
    /// `ArrayList<Object>`
    List(YamlList),
    /// `LinkedHashSet<Object>` (`!!set`)
    Set(YamlList),
    /// `List<Object[]>` built by `!!pairs`
    Pairs(Arc<Vec<(YamlObject, YamlObject)>>),
}

/// A shared `LinkedHashMap<Object, Object>`: entries in insertion order, identity kept across clones.
#[derive(Clone, Debug, Default)]
pub struct YamlMap(pub Arc<RwLock<Vec<(YamlObject, YamlObject)>>>);

/// A shared `ArrayList<Object>` (or a `LinkedHashSet` for `!!set`), identity kept across clones.
#[derive(Clone, Debug, Default)]
pub struct YamlList(pub Arc<RwLock<Vec<YamlObject>>>);

impl YamlMap {
    /// An empty map.
    pub fn new() -> YamlMap {
        YamlMap::default()
    }

    /// `Map.put`: replaces the value of an equal key in place, else appends.
    pub fn put(&self, key: YamlObject, value: YamlObject) {
        let mut entries = self.0.write();
        let pos = { entries.iter().position(|(k, _)| k.java_equals(&key)) };
        match pos {
            Some(i) => entries[i].1 = value,
            None => entries.push((key, value)),
        }
    }

    /// A snapshot of the entries in insertion order.
    pub fn entries(&self) -> Vec<(YamlObject, YamlObject)> {
        self.0.read().clone()
    }

    /// `Map.size()`.
    pub fn len(&self) -> usize {
        self.0.read().len()
    }

    /// `Map.isEmpty()`.
    pub fn is_empty(&self) -> bool {
        self.0.read().is_empty()
    }

    /// `true` when both handles are the same map (Java `==`).
    pub fn ptr_eq(&self, o: &YamlMap) -> bool {
        Arc::ptr_eq(&self.0, &o.0)
    }

    /// A process-unique identity of the map, used where Java uses `System.identityHashCode`.
    pub fn id(&self) -> usize {
        Arc::as_ptr(&self.0) as usize
    }
}

impl YamlList {
    /// An empty list.
    pub fn new() -> YamlList {
        YamlList::default()
    }

    /// `List.add`: appends `v`.
    pub fn push(&self, v: YamlObject) {
        self.0.write().push(v);
    }

    /// `Set.add` semantics (no duplicates by Java equality).
    pub fn add_to_set(&self, v: YamlObject) {
        let mut items = self.0.write();
        if !items.iter().any(|x| x.java_equals(&v)) {
            items.push(v);
        }
    }

    /// A snapshot of the items in order.
    pub fn items(&self) -> Vec<YamlObject> {
        self.0.read().clone()
    }

    /// `List.size()`.
    pub fn len(&self) -> usize {
        self.0.read().len()
    }

    /// `List.isEmpty()`.
    pub fn is_empty(&self) -> bool {
        self.0.read().is_empty()
    }

    /// `true` when both handles are the same list (Java `==`).
    pub fn ptr_eq(&self, o: &YamlList) -> bool {
        Arc::ptr_eq(&self.0, &o.0)
    }

    /// A process-unique identity of the list, used where Java uses `System.identityHashCode`.
    pub fn id(&self) -> usize {
        Arc::as_ptr(&self.0) as usize
    }
}

impl YamlObject {
    /// `true` for Java `null`.
    pub fn is_null(&self) -> bool {
        matches!(self, YamlObject::Null)
    }

    /// Identity of a container (for `IdentityHashMap` walks); `None` for values.
    pub fn identity(&self) -> Option<usize> {
        match self {
            YamlObject::Map(m) => Some(m.id()),
            YamlObject::List(l) | YamlObject::Set(l) => Some(l.id()),
            _ => None,
        }
    }

    /// `Objects.equals`
    pub fn java_equals(&self, other: &YamlObject) -> bool {
        use YamlObject::*;
        match (self, other) {
            (Null, Null) => true,
            (Boolean(a), Boolean(b)) => a == b,
            (Integer(a), Integer(b)) => a == b,
            (Long(a), Long(b)) => a == b,
            (BigInteger(a), BigInteger(b)) => a == b,
            (Double(a), Double(b)) => (a.is_nan() && b.is_nan()) || a.to_bits() == b.to_bits(),
            (String(a), String(b)) => a == b,
            (Bytes(a), Bytes(b)) => Arc::ptr_eq(a, b),
            (Date(a), Date(b)) => a == b,
            (Map(a), Map(b)) => {
                if a.ptr_eq(b) {
                    return true;
                }
                let (ea, eb) = (a.entries(), b.entries());
                ea.len() == eb.len() && ea.iter().all(|(k, v)| eb.iter().any(|(k2, v2)| k.java_equals(k2) && v.java_equals(v2)))
            }
            (List(a), List(b)) => {
                a.ptr_eq(b) || {
                    let (x, y) = (a.items(), b.items());
                    x.len() == y.len() && x.iter().zip(y.iter()).all(|(p, q)| p.java_equals(q))
                }
            }
            (Set(a), Set(b)) => {
                a.ptr_eq(b) || {
                    let (x, y) = (a.items(), b.items());
                    x.len() == y.len() && x.iter().all(|p| y.iter().any(|q| p.java_equals(q)))
                }
            }
            (Pairs(a), Pairs(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }

    /// `String.valueOf(obj)`
    pub fn to_java_string(&self) -> std::string::String {
        use YamlObject::*;
        match self {
            Null => "null".into(),
            Boolean(b) => b.to_string(),
            Integer(v) => v.to_string(),
            Long(v) => v.to_string(),
            BigInteger(v) => v.to_string(),
            Double(v) => java_compat::double_to_string(*v),
            String(s) => s.to_string(),
            Bytes(b) => format!("[B@{:x}", (Arc::as_ptr(b) as *const u8 as usize) & 0x7fff_ffff),
            Date(ms) => format!("Date({ms})"),
            Map(m) => {
                let parts: Vec<std::string::String> = m
                    .entries()
                    .iter()
                    .map(|(k, v)| {
                        let vs = if v.identity() == Some(m.id()) { "(this Map)".to_string() } else { v.to_java_string() };
                        format!("{}={}", k.to_java_string(), vs)
                    })
                    .collect();
                format!("{{{}}}", parts.join(", "))
            }
            List(l) | Set(l) => {
                let parts: Vec<std::string::String> = l
                    .items()
                    .iter()
                    .map(|v| if v.identity() == Some(l.id()) { "(this Collection)".to_string() } else { v.to_java_string() })
                    .collect();
                format!("[{}]", parts.join(", "))
            }
            Pairs(p) => format!("{:?}", p.len()),
        }
    }
}
