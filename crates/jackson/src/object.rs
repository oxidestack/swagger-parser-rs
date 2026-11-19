//! Untyped Java values: what Jackson binds to `Object.class`, and what swagger models keep in their
//! `Object`-typed fields (`example`, `default`, `enum` items, extension values, …).

use crate::bigdecimal::BigDecimal;
use crate::generator::JsonGenerator;
use crate::node::JsonNode;
use indexmap::IndexMap;
use num_bigint::BigInt;
use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;

/// A `java.time.OffsetDateTime`: local date-time plus a fixed offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OffsetDateTime {
    /// Seconds since the epoch of the *instant* (UTC).
    pub epoch_second: i64,
    /// Nano-of-second, `0..1_000_000_000`.
    pub nano: u32,
    /// Offset from UTC in seconds (`ZoneOffset.getTotalSeconds()`).
    pub offset_seconds: i32,
}

/// An object Jackson serializes through a bean serializer defined outside this crate (a swagger model
/// instance stored in an `Object` field). `mapper` identifies the configured `ObjectMapper` (e.g.
/// `Json.mapper()` vs `Json31.mapper()`); only the implementing crate interprets it.
pub trait Pojo: Send + Sync + Debug {
    /// `getClass().getName()`
    fn class_name(&self) -> String;
    /// Serialize with the bean serializer of the given mapper.
    fn serialize(&self, generator: &mut dyn JsonGenerator, mapper: &dyn Any) -> Result<(), crate::JacksonError>;
    /// `toString()`
    fn to_java_string(&self) -> String;
    /// `equals(Object)`
    fn java_equals(&self, other: &dyn Pojo) -> bool;
    /// `hashCode()`
    fn java_hash_code(&self) -> i32;
    /// The implementing value as `Any`, for downcasting to its concrete type.
    fn as_any(&self) -> &dyn Any;
}

/// A Java value of static type `Object`.
#[derive(Clone, Debug)]
pub enum JavaObject {
    /// Java `null`.
    Null,
    /// `java.lang.String`
    String(Arc<str>),
    /// `java.lang.Boolean`
    Boolean(bool),
    /// `java.lang.Short`
    Short(i16),
    /// `java.lang.Integer`
    Integer(i32),
    /// `java.lang.Long`
    Long(i64),
    /// `java.math.BigInteger`
    BigInteger(Arc<BigInt>),
    /// `java.lang.Float`
    Float(f32),
    /// `java.lang.Double`
    Double(f64),
    /// `java.math.BigDecimal`
    BigDecimal(Arc<BigDecimal>),
    /// `java.util.LinkedHashMap<String, Object>`
    Map(Arc<IndexMap<String, JavaObject>>),
    /// `java.util.ArrayList<Object>`
    List(Arc<Vec<JavaObject>>),
    /// `java.util.HashSet<Object>`, elements in the set's iteration order (see [`java_hash_set`])
    Set(Arc<Vec<JavaObject>>),
    /// `byte[]`
    Bytes(Arc<[u8]>),
    /// `java.util.Date` as epoch milliseconds
    Date(i64),
    /// `java.time.OffsetDateTime`
    OffsetDateTime(OffsetDateTime),
    /// `java.util.UUID` (most significant bits first)
    Uuid(u128),
    /// A `JsonNode` held as an `Object` (e.g. an `ObjectNode` example)
    Node(JsonNode),
    /// Any other bean
    Pojo(Arc<dyn Pojo>),
}

impl JavaObject {
    /// A `java.lang.String` value.
    pub fn string(s: impl Into<Arc<str>>) -> JavaObject {
        JavaObject::String(s.into())
    }

    /// `true` for Java `null`.
    pub fn is_null(&self) -> bool {
        matches!(self, JavaObject::Null)
    }

    /// The text of a `java.lang.String`, `None` for any other value.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            JavaObject::String(s) => Some(s),
            _ => None,
        }
    }
}

impl JavaObject {
    /// `String.valueOf(obj)` — Java `toString()` of the value (`null` for null).
    pub fn to_java_string(&self) -> String {
        match self {
            JavaObject::Null => "null".to_string(),
            JavaObject::String(s) => s.to_string(),
            JavaObject::Boolean(b) => b.to_string(),
            JavaObject::Short(v) => v.to_string(),
            JavaObject::Integer(v) => v.to_string(),
            JavaObject::Long(v) => v.to_string(),
            JavaObject::BigInteger(v) => v.to_string(),
            JavaObject::Float(v) => java_compat::float_to_string(*v),
            JavaObject::Double(v) => java_compat::double_to_string(*v),
            JavaObject::BigDecimal(v) => v.to_java_string(),
            JavaObject::Map(m) => {
                let parts: Vec<String> = m.iter().map(|(k, v)| format!("{k}={}", v.to_java_string())).collect();
                format!("{{{}}}", parts.join(", "))
            }
            JavaObject::List(l) | JavaObject::Set(l) => {
                let parts: Vec<String> = l.iter().map(|v| v.to_java_string()).collect();
                format!("[{}]", parts.join(", "))
            }
            JavaObject::Bytes(b) => format!("[B@{:x}", (Arc::as_ptr(b) as *const u8 as usize) & 0x7fff_ffff),
            JavaObject::Date(ms) => date_to_string_utc(*ms),
            JavaObject::OffsetDateTime(o) => crate::convert::offset_date_time_to_string(o),
            JavaObject::Uuid(u) => crate::convert::uuid_to_string(*u),
            JavaObject::Node(n) => n.to_java_string(),
            JavaObject::Pojo(p) => p.to_java_string(),
        }
    }

    /// `Objects.equals(a, b)`
    pub fn java_equals(&self, other: &JavaObject) -> bool {
        use JavaObject::*;
        match (self, other) {
            (Null, Null) => true,
            (String(a), String(b)) => a == b,
            (Boolean(a), Boolean(b)) => a == b,
            (Short(a), Short(b)) => a == b,
            (Integer(a), Integer(b)) => a == b,
            (Long(a), Long(b)) => a == b,
            (BigInteger(a), BigInteger(b)) => a == b,
            (Float(a), Float(b)) => (a.is_nan() && b.is_nan()) || a.to_bits() == b.to_bits(),
            (Double(a), Double(b)) => (a.is_nan() && b.is_nan()) || a.to_bits() == b.to_bits(),
            (BigDecimal(a), BigDecimal(b)) => a == b,
            (Map(a), Map(b)) => {
                Arc::ptr_eq(a, b) || (a.len() == b.len() && a.iter().all(|(k, v)| b.get(k).is_some_and(|w| v.java_equals(w))))
            }
            (List(a), List(b)) => Arc::ptr_eq(a, b) || (a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.java_equals(y))),
            (Set(a), Set(b)) => Arc::ptr_eq(a, b) || (a.len() == b.len() && a.iter().all(|x| b.iter().any(|y| x.java_equals(y)))),
            (Bytes(a), Bytes(b)) => Arc::ptr_eq(a, b),
            (Date(a), Date(b)) => a == b,
            (OffsetDateTime(a), OffsetDateTime(b)) => a == b,
            (Uuid(a), Uuid(b)) => a == b,
            (Node(a), Node(b)) => a == b,
            (Pojo(a), Pojo(b)) => a.java_equals(&**b),
            _ => false,
        }
    }

    /// `Objects.hashCode(obj)`
    pub fn java_hash_code(&self) -> i32 {
        use JavaObject::*;
        match self {
            Null => 0,
            String(s) => java_compat::lang::string_hash(s),
            Boolean(b) => {
                if *b {
                    1231
                } else {
                    1237
                }
            }
            Short(v) => *v as i32,
            Integer(v) => *v,
            Long(v) => (*v ^ ((*v as u64) >> 32) as i64) as i32,
            BigInteger(v) => crate::bigdecimal::big_integer_hash_code(v),
            Float(v) => {
                let bits = if v.is_nan() { 0x7fc00000u32 } else { v.to_bits() };
                bits as i32
            }
            Double(v) => {
                let bits = if v.is_nan() { 0x7ff8000000000000u64 } else { v.to_bits() };
                (bits ^ (bits >> 32)) as i32
            }
            BigDecimal(v) => v.java_hash_code(),
            Map(m) => m.iter().fold(0i32, |acc, (k, v)| acc.wrapping_add(java_compat::lang::string_hash(k) ^ v.java_hash_code())),
            List(l) => l.iter().fold(1i32, |acc, v| acc.wrapping_mul(31).wrapping_add(v.java_hash_code())),
            Set(l) => l.iter().fold(0i32, |acc, v| acc.wrapping_add(v.java_hash_code())),
            Bytes(b) => (Arc::as_ptr(b) as *const u8 as usize) as i32,
            Date(ms) => (*ms ^ ((*ms as u64) >> 32) as i64) as i32,
            OffsetDateTime(o) => {
                // dateTime.hashCode() ^ offset.hashCode(); LocalDateTime = date ^ time
                let local = o.epoch_second + o.offset_seconds as i64;
                let days = local.div_euclid(86400);
                let secs = local.rem_euclid(86400);
                let (y, m, d) = crate::convert::civil_from_days(days);
                let (y, m, d) = (y as i32, m as i32, d as i32);
                let date_hash = (y & (0xFFFFF800u32 as i32)) ^ ((y << 11) + (m << 6) + d);
                let nano_of_day = secs * 1_000_000_000 + o.nano as i64;
                let time_hash = (nano_of_day ^ ((nano_of_day as u64) >> 32) as i64) as i32;
                (date_hash ^ time_hash) ^ o.offset_seconds
            }
            Uuid(u) => {
                let hilo = ((*u >> 64) as u64) ^ (*u as u64);
                ((hilo >> 32) as i32) ^ (hilo as i32)
            }
            Node(n) => n.java_hash_code(),
            Pojo(p) => p.java_hash_code(),
        }
    }
}

impl PartialEq for JavaObject {
    fn eq(&self, other: &Self) -> bool {
        self.java_equals(other)
    }
}

/// `java.util.Date.toString()` rendered in UTC: `EEE MMM dd HH:mm:ss zzz yyyy`.
pub fn date_to_string_utc(ms: i64) -> String {
    const DAYS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let days = ms.div_euclid(86_400_000);
    let rem = ms.rem_euclid(86_400_000);
    let (y, m, d) = crate::convert::civil_from_days(days);
    let dow = DAYS[days.rem_euclid(7) as usize];
    format!(
        "{dow} {} {d:02} {:02}:{:02}:{:02} UTC {y}",
        MONTHS[(m - 1) as usize],
        rem / 3_600_000,
        (rem / 60_000) % 60,
        (rem / 1000) % 60
    )
}

/// Builds a `new HashSet<>()` from `items` added in order: duplicates (`equals`) dropped, elements returned
/// in the set's iteration order (bucket of `hashCode()` spread for the table capacity, then insertion order).
pub fn java_hash_set(items: impl IntoIterator<Item = JavaObject>) -> Vec<JavaObject> {
    let mut entries: Vec<(u32, JavaObject)> = Vec::new();
    let mut capacity: usize = 16;
    let mut threshold: usize = 12;
    for item in items {
        if entries.iter().any(|(_, e)| e.java_equals(&item)) {
            continue;
        }
        let h = item.java_hash_code() as u32;
        entries.push((h ^ (h >> 16), item));
        if entries.len() > threshold {
            capacity *= 2;
            threshold *= 2;
        }
    }
    let mask = (capacity - 1) as u32;
    let mut indexed: Vec<(u32, usize, JavaObject)> = entries.into_iter().enumerate().map(|(i, (h, e))| (h & mask, i, e)).collect();
    indexed.sort_by_key(|e| (e.0, e.1));
    indexed.into_iter().map(|e| e.2).collect()
}
