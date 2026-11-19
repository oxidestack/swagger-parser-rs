//! `com.fasterxml.jackson.databind.JsonNode` and its concrete node classes.
//!
//! Containers are `Arc`-shared so cloning a tree is cheap; mutation goes through `Arc::make_mut`
//! (copy-on-write). Object fields keep insertion order (Jackson's `ObjectNode` is a `LinkedHashMap`).

use crate::bigdecimal::BigDecimal;
use crate::object::JavaObject;
use indexmap::IndexMap;
use num_bigint::BigInt;
use std::sync::Arc;

/// The field map of an `ObjectNode` (a `LinkedHashMap<String, JsonNode>`). Removal must preserve order
/// (`shift_remove`), as `LinkedHashMap.remove` does.
pub type ObjectNode = IndexMap<String, JsonNode>;

/// One Jackson tree node. Each variant is one concrete Jackson node class.
#[derive(Clone, Debug)]
pub enum JsonNode {
    /// `ObjectNode`
    Object(Arc<ObjectNode>),
    /// `ArrayNode`
    Array(Arc<Vec<JsonNode>>),
    /// `TextNode`
    Text(Arc<str>),
    /// `BinaryNode`
    Binary(Arc<[u8]>),
    /// `BooleanNode`
    Boolean(bool),
    /// `NullNode`
    Null,
    /// `MissingNode` (returned by `path()`/`at()` lookups that find nothing)
    Missing,
    /// `ShortNode`
    Short(i16),
    /// `IntNode`
    Int(i32),
    /// `LongNode`
    Long(i64),
    /// `BigIntegerNode`
    BigInteger(Arc<BigInt>),
    /// `FloatNode`
    Float(f32),
    /// `DoubleNode`
    Double(f64),
    /// `DecimalNode`
    Decimal(Arc<BigDecimal>),
    /// `POJONode` — wraps an arbitrary Java value (e.g. from `valueToTree` of a `Date`).
    Pojo(Arc<JavaObject>),
}

impl JsonNode {
    /// `JsonNode.hashCode()` as each Jackson node class defines it
    pub fn java_hash_code(&self) -> i32 {
        fn dbl(v: f64) -> i32 {
            let l = if v.is_nan() { 0x7ff8000000000000u64 } else { v.to_bits() };
            (l as i32) ^ ((l >> 32) as i32)
        }
        match self {
            JsonNode::Object(o) => o.iter().fold(0i32, |acc, (k, v)| acc.wrapping_add(java_compat::lang::string_hash(k) ^ v.java_hash_code())),
            JsonNode::Array(a) => a.iter().fold(1i32, |acc, v| acc.wrapping_mul(31).wrapping_add(v.java_hash_code())),
            JsonNode::Text(t) => java_compat::lang::string_hash(t),
            JsonNode::Binary(b) => b.len() as i32,
            JsonNode::Boolean(b) => if *b { 3 } else { 1 },
            JsonNode::Null => 4,
            JsonNode::Missing => 3,
            JsonNode::Short(v) => *v as i32,
            JsonNode::Int(v) => *v,
            JsonNode::Long(v) => (*v as i32) ^ ((*v >> 32) as i32),
            JsonNode::BigInteger(v) => crate::bigdecimal::big_integer_hash_code(v),
            JsonNode::Float(v) => if v.is_nan() { 0x7fc00000 } else { v.to_bits() as i32 },
            JsonNode::Double(v) => dbl(*v),
            JsonNode::Decimal(v) => dbl(v.to_f64()),
            JsonNode::Pojo(p) => p.java_hash_code(),
        }
    }

    /// The simple name of the Jackson node class (`TextNode`, `ObjectNode`, …)
    pub fn node_class_name(&self) -> &'static str {
        match self {
            JsonNode::Object(_) => "ObjectNode",
            JsonNode::Array(_) => "ArrayNode",
            JsonNode::Text(_) => "TextNode",
            JsonNode::Binary(_) => "BinaryNode",
            JsonNode::Boolean(_) => "BooleanNode",
            JsonNode::Null => "NullNode",
            JsonNode::Missing => "MissingNode",
            JsonNode::Short(_) => "ShortNode",
            JsonNode::Int(_) => "IntNode",
            JsonNode::Long(_) => "LongNode",
            JsonNode::BigInteger(_) => "BigIntegerNode",
            JsonNode::Float(_) => "FloatNode",
            JsonNode::Double(_) => "DoubleNode",
            JsonNode::Decimal(_) => "DecimalNode",
            JsonNode::Pojo(_) => "POJONode",
        }
    }

    /// An `ObjectNode` holding the given fields in their order.
    pub fn object(fields: ObjectNode) -> JsonNode {
        JsonNode::Object(Arc::new(fields))
    }

    /// `JsonNodeFactory.objectNode()`: an `ObjectNode` with no fields.
    pub fn empty_object() -> JsonNode {
        JsonNode::Object(Arc::new(ObjectNode::new()))
    }

    /// An `ArrayNode` holding the given elements.
    pub fn array(items: Vec<JsonNode>) -> JsonNode {
        JsonNode::Array(Arc::new(items))
    }

    /// `TextNode.valueOf(s)`.
    pub fn text(s: impl Into<Arc<str>>) -> JsonNode {
        JsonNode::Text(s.into())
    }

    /// `JsonNode.get(String)` on an object; `None` for non-objects and absent fields.
    pub fn get(&self, field: &str) -> Option<&JsonNode> {
        match self {
            JsonNode::Object(m) => m.get(field),
            _ => None,
        }
    }

    /// `JsonNode.get(int)` on an array.
    pub fn get_index(&self, index: usize) -> Option<&JsonNode> {
        match self {
            JsonNode::Array(a) => a.get(index),
            _ => None,
        }
    }

    /// The fields of an `ObjectNode`, `None` for any other node.
    pub fn as_object(&self) -> Option<&ObjectNode> {
        match self {
            JsonNode::Object(m) => Some(m),
            _ => None,
        }
    }

    /// Mutable access to an object's fields (copy-on-write).
    pub fn as_object_mut(&mut self) -> Option<&mut ObjectNode> {
        match self {
            JsonNode::Object(m) => Some(Arc::make_mut(m)),
            _ => None,
        }
    }

    /// The elements of an `ArrayNode`, `None` for any other node.
    pub fn as_array(&self) -> Option<&Vec<JsonNode>> {
        match self {
            JsonNode::Array(a) => Some(a),
            _ => None,
        }
    }

    /// The elements of an `ArrayNode` for mutation (copy-on-write if the array is shared), `None` for any other node.
    pub fn as_array_mut(&mut self) -> Option<&mut Vec<JsonNode>> {
        match self {
            JsonNode::Array(a) => Some(Arc::make_mut(a)),
            _ => None,
        }
    }

    /// `isObject()`: `true` only for `ObjectNode`.
    pub fn is_object(&self) -> bool {
        matches!(self, JsonNode::Object(_))
    }

    /// `isArray()`: `true` only for `ArrayNode`.
    pub fn is_array(&self) -> bool {
        matches!(self, JsonNode::Array(_))
    }

    /// `isTextual()`
    pub fn is_textual(&self) -> bool {
        matches!(self, JsonNode::Text(_))
    }

    /// `isBoolean()`: `true` only for `BooleanNode`.
    pub fn is_boolean(&self) -> bool {
        matches!(self, JsonNode::Boolean(_))
    }

    /// `isNull()` — true only for `NullNode`.
    pub fn is_null(&self) -> bool {
        matches!(self, JsonNode::Null)
    }

    /// `isMissingNode()`: `true` only for `MissingNode`.
    pub fn is_missing(&self) -> bool {
        matches!(self, JsonNode::Missing)
    }

    /// `isNumber()`
    pub fn is_number(&self) -> bool {
        matches!(
            self,
            JsonNode::Short(_)
                | JsonNode::Int(_)
                | JsonNode::Long(_)
                | JsonNode::BigInteger(_)
                | JsonNode::Float(_)
                | JsonNode::Double(_)
                | JsonNode::Decimal(_)
        )
    }

    /// `isIntegralNumber()`
    pub fn is_integral_number(&self) -> bool {
        matches!(self, JsonNode::Short(_) | JsonNode::Int(_) | JsonNode::Long(_) | JsonNode::BigInteger(_))
    }

    /// `isFloatingPointNumber()`
    pub fn is_floating_point_number(&self) -> bool {
        matches!(self, JsonNode::Float(_) | JsonNode::Double(_) | JsonNode::Decimal(_))
    }

    /// `isValueNode()` — scalars (including null and binary), not containers or missing.
    pub fn is_value_node(&self) -> bool {
        !matches!(self, JsonNode::Object(_) | JsonNode::Array(_) | JsonNode::Missing)
    }

    /// `isContainerNode()`
    pub fn is_container_node(&self) -> bool {
        matches!(self, JsonNode::Object(_) | JsonNode::Array(_))
    }

    /// `textValue()` — the string of a `TextNode`, `None` for everything else.
    pub fn text_value(&self) -> Option<&str> {
        match self {
            JsonNode::Text(s) => Some(s),
            _ => None,
        }
    }

    /// `size()`
    pub fn size(&self) -> usize {
        match self {
            JsonNode::Object(m) => m.len(),
            JsonNode::Array(a) => a.len(),
            _ => 0,
        }
    }
}

impl JsonNode {
    /// `JsonNode.path(String)`: the field, or `MissingNode`.
    pub fn path(&self, field: &str) -> JsonNode {
        self.get(field).cloned().unwrap_or(JsonNode::Missing)
    }

    /// `asText()`
    pub fn as_text(&self) -> String {
        match self {
            JsonNode::Object(_) | JsonNode::Array(_) | JsonNode::Missing => String::new(),
            JsonNode::Text(s) => s.to_string(),
            JsonNode::Binary(b) => crate::json::write::base64_mime_no_linefeeds(b),
            JsonNode::Boolean(b) => b.to_string(),
            JsonNode::Null => "null".to_string(),
            JsonNode::Short(v) => v.to_string(),
            JsonNode::Int(v) => v.to_string(),
            JsonNode::Long(v) => v.to_string(),
            JsonNode::BigInteger(v) => v.to_string(),
            JsonNode::Float(v) => java_compat::float_to_string(*v),
            JsonNode::Double(v) => java_compat::double_to_string(*v),
            JsonNode::Decimal(v) => v.to_java_string(),
            JsonNode::Pojo(o) => match &**o {
                JavaObject::Null => "null".to_string(),
                o => o.to_java_string(),
            },
        }
    }

    /// `booleanValue()` (false for non-booleans)
    pub fn boolean_value(&self) -> bool {
        matches!(self, JsonNode::Boolean(true))
    }

    /// `asBoolean()`: booleans, "true" text (trimmed), non-zero integers.
    pub fn as_boolean(&self) -> bool {
        self.as_boolean_or(false)
    }

    /// `asBoolean(boolean defaultValue)`: text reads only a trimmed `true`/`false`; floating-point numbers, containers
    /// and nulls give the default.
    pub fn as_boolean_or(&self, default: bool) -> bool {
        match self {
            JsonNode::Boolean(b) => *b,
            JsonNode::Text(s) => match s.trim() {
                "true" => true,
                "false" => false,
                _ => default,
            },
            JsonNode::Short(v) => *v != 0,
            JsonNode::Int(v) => *v != 0,
            JsonNode::Long(v) => *v != 0,
            JsonNode::BigInteger(v) => !num_traits::Zero::is_zero(&**v),
            _ => default,
        }
    }

    /// `asText(String defaultValue)`: the default for null and missing nodes, `asText()` otherwise.
    pub fn as_text_or(&self, default: &str) -> String {
        match self {
            JsonNode::Null | JsonNode::Missing => default.to_string(),
            JsonNode::Pojo(o) if matches!(&**o, JavaObject::Null) => default.to_string(),
            other => other.as_text(),
        }
    }

    /// `asInt()`
    pub fn as_int(&self) -> i32 {
        self.as_int_or(0)
    }

    /// `asInt(int defaultValue)`: numbers narrowed (`intValue()`), booleans as 1/0, text through
    /// `NumberInput.parseAsInt`, anything else the default.
    pub fn as_int_or(&self, default: i32) -> i32 {
        match self {
            JsonNode::Boolean(b) => *b as i32,
            JsonNode::Text(s) => parse_as_int(s, default),
            n if n.is_number() => n.int_value(),
            _ => default,
        }
    }

    /// `intValue()` for numeric nodes (0 otherwise), with Java's narrowing.
    pub fn int_value(&self) -> i32 {
        match self {
            JsonNode::Short(v) => *v as i32,
            JsonNode::Int(v) => *v,
            JsonNode::Long(v) => *v as i32,
            JsonNode::BigInteger(v) => crate::bigdecimal::wrap_i64(v) as i32,
            JsonNode::Float(v) => *v as i32,
            JsonNode::Double(v) => *v as i32,
            JsonNode::Decimal(v) => v.to_i32_wrapping(),
            _ => 0,
        }
    }

    /// `longValue()`
    pub fn long_value(&self) -> i64 {
        match self {
            JsonNode::Short(v) => *v as i64,
            JsonNode::Int(v) => *v as i64,
            JsonNode::Long(v) => *v,
            JsonNode::BigInteger(v) => crate::bigdecimal::wrap_i64(v),
            JsonNode::Float(v) => *v as i64,
            JsonNode::Double(v) => *v as i64,
            JsonNode::Decimal(v) => v.to_i64_wrapping(),
            _ => 0,
        }
    }

    /// `doubleValue()`
    pub fn double_value(&self) -> f64 {
        match self {
            JsonNode::Short(v) => *v as f64,
            JsonNode::Int(v) => *v as f64,
            JsonNode::Long(v) => *v as f64,
            JsonNode::BigInteger(v) => v.to_string().parse().unwrap_or(0.0),
            JsonNode::Float(v) => *v as f64,
            JsonNode::Double(v) => *v,
            JsonNode::Decimal(v) => v.to_f64(),
            _ => 0.0,
        }
    }

    /// `decimalValue()` (BigDecimal.ZERO for non-numbers)
    pub fn decimal_value(&self) -> BigDecimal {
        match self {
            JsonNode::Short(v) => BigDecimal::from_i64(*v as i64),
            JsonNode::Int(v) => BigDecimal::from_i64(*v as i64),
            JsonNode::Long(v) => BigDecimal::from_i64(*v),
            JsonNode::BigInteger(v) => BigDecimal::from_bigint((**v).clone()),
            JsonNode::Float(v) => BigDecimal::parse(&java_compat::float_to_string(*v)).unwrap_or(BigDecimal::from_i64(0)),
            JsonNode::Double(v) => BigDecimal::try_value_of_f64(*v).unwrap_or(BigDecimal::from_i64(0)),
            JsonNode::Decimal(v) => (**v).clone(),
            _ => BigDecimal::from_i64(0),
        }
    }

    /// `numberValue()` as a Java value (`Integer`, `Long`, …); `None` for non-numbers.
    pub fn number_value(&self) -> Option<JavaObject> {
        Some(match self {
            JsonNode::Short(v) => JavaObject::Short(*v),
            JsonNode::Int(v) => JavaObject::Integer(*v),
            JsonNode::Long(v) => JavaObject::Long(*v),
            JsonNode::BigInteger(v) => JavaObject::BigInteger(v.clone()),
            JsonNode::Float(v) => JavaObject::Float(*v),
            JsonNode::Double(v) => JavaObject::Double(*v),
            JsonNode::Decimal(v) => JavaObject::BigDecimal(v.clone()),
            _ => return None,
        })
    }

    /// `isInt()`: `true` only for `IntNode`.
    pub fn is_int(&self) -> bool {
        matches!(self, JsonNode::Int(_))
    }
    /// `isLong()`: `true` only for `LongNode`.
    pub fn is_long(&self) -> bool {
        matches!(self, JsonNode::Long(_))
    }
    /// `isBigInteger()`: `true` only for `BigIntegerNode`.
    pub fn is_big_integer(&self) -> bool {
        matches!(self, JsonNode::BigInteger(_))
    }
    /// `isDouble()`: `true` only for `DoubleNode`.
    pub fn is_double(&self) -> bool {
        matches!(self, JsonNode::Double(_))
    }
    /// `isBigDecimal()`: `true` only for `DecimalNode`.
    pub fn is_big_decimal(&self) -> bool {
        matches!(self, JsonNode::Decimal(_))
    }
    /// `isBinary()`: `true` only for `BinaryNode`.
    pub fn is_binary(&self) -> bool {
        matches!(self, JsonNode::Binary(_))
    }
    /// `isPojo()`: `true` only for `POJONode`.
    pub fn is_pojo(&self) -> bool {
        matches!(self, JsonNode::Pojo(_))
    }

    /// `canConvertToInt()`
    pub fn can_convert_to_int(&self) -> bool {
        match self {
            JsonNode::Short(_) | JsonNode::Int(_) => true,
            JsonNode::Long(v) => *v >= i32::MIN as i64 && *v <= i32::MAX as i64,
            JsonNode::BigInteger(v) => i32::try_from(&**v).is_ok(),
            JsonNode::Float(v) => *v >= i32::MIN as f32 && *v <= i32::MAX as f32,
            JsonNode::Double(v) => *v >= i32::MIN as f64 && *v <= i32::MAX as f64,
            JsonNode::Decimal(v) => v.compare_to(&BigDecimal::from_i64(i32::MIN as i64)).is_ge() && v.compare_to(&BigDecimal::from_i64(i32::MAX as i64)).is_le(),
            _ => false,
        }
    }

    /// `canConvertToLong()`
    pub fn can_convert_to_long(&self) -> bool {
        match self {
            JsonNode::Short(_) | JsonNode::Int(_) | JsonNode::Long(_) => true,
            JsonNode::BigInteger(v) => i64::try_from(&**v).is_ok(),
            JsonNode::Float(v) => *v >= i64::MIN as f32 && *v <= i64::MAX as f32,
            JsonNode::Double(v) => *v >= i64::MIN as f64 && *v <= i64::MAX as f64,
            JsonNode::Decimal(v) => v.compare_to(&BigDecimal::from_i64(i64::MIN)).is_ge() && v.compare_to(&BigDecimal::from_i64(i64::MAX)).is_le(),
            _ => false,
        }
    }

    /// Field names of an object node, in order (`fieldNames()`).
    pub fn field_names(&self) -> Vec<String> {
        match self {
            JsonNode::Object(m) => m.keys().cloned().collect(),
            _ => Vec::new(),
        }
    }

    /// `JsonNode.at(String)`: JSON Pointer evaluation (`MissingNode` when absent).
    pub fn at(&self, pointer: &str) -> JsonNode {
        if pointer.is_empty() {
            return self.clone();
        }
        if !pointer.starts_with('/') {
            return JsonNode::Missing;
        }
        let mut cur = self.clone();
        for raw in pointer[1..].split('/') {
            let seg = raw.replace("~1", "/").replace("~0", "~");
            let next = match &cur {
                JsonNode::Object(m) => m.get(&seg).cloned(),
                JsonNode::Array(a) => parse_index(&seg).and_then(|i| a.get(i).cloned()),
                _ => None,
            };
            match next {
                Some(n) => cur = n,
                None => return JsonNode::Missing,
            }
        }
        cur
    }

    /// `toString()` — compact JSON as Jackson's `InternalNodeMapper` writes it (BigDecimal not plain).
    pub fn to_java_string(&self) -> String {
        if let JsonNode::Missing = self {
            return String::new();
        }
        let mut w = crate::json::JsonWriter::new(crate::json::WriteFeatures::default());
        let _ = crate::convert::write_node(&mut w, self, &crate::convert::SerConfig::internal_node_mapper());
        w.into_string()
    }
}

fn parse_index(seg: &str) -> Option<usize> {
    if seg.is_empty() || seg.len() > 10 || !seg.bytes().all(|b| b.is_ascii_digit()) || (seg.len() > 1 && seg.starts_with('0')) {
        return None;
    }
    seg.parse::<i64>().ok().filter(|v| *v <= i32::MAX as i64).map(|v| v as usize)
}

/// Jackson node equality: same node class and equal value; object fields compared as maps.
impl PartialEq for JsonNode {
    fn eq(&self, other: &Self) -> bool {
        use JsonNode::*;
        match (self, other) {
            (Object(a), Object(b)) => Arc::ptr_eq(a, b) || (a.len() == b.len() && a.iter().all(|(k, v)| b.get(k) == Some(v))),
            (Array(a), Array(b)) => a == b,
            (Text(a), Text(b)) => a == b,
            (Binary(a), Binary(b)) => a == b,
            (Boolean(a), Boolean(b)) => a == b,
            (Null, Null) | (Missing, Missing) => true,
            (Short(a), Short(b)) => a == b,
            (Int(a), Int(b)) => a == b,
            (Long(a), Long(b)) => a == b,
            (BigInteger(a), BigInteger(b)) => a == b,
            (Float(a), Float(b)) => (a.is_nan() && b.is_nan()) || a.to_bits() == b.to_bits(),
            (Double(a), Double(b)) => (a.is_nan() && b.is_nan()) || a.to_bits() == b.to_bits(),
            // DecimalNode.equals uses compareTo (scale-insensitive)
            (Decimal(a), Decimal(b)) => a.compare_to(b).is_eq(),
            (Pojo(a), Pojo(b)) => a.java_equals(b),
            _ => false,
        }
    }
}

impl std::fmt::Display for JsonNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_java_string())
    }
}

/// `NumberInput.parseAsInt`: trimmed, an optional sign, then `Integer.parseInt` for plain digits (the default when it
/// overflows) or `Double.parseDouble` narrowed for anything else.
pub fn parse_as_int(s: &str, default: i32) -> i32 {
    let s = s.trim_matches(|c: char| c <= ' ');
    if s.is_empty() {
        return default;
    }
    let s = s.strip_prefix('+').unwrap_or(s);
    let digits = s.strip_prefix('-').unwrap_or(s);
    if digits.bytes().all(|b| b.is_ascii_digit()) {
        // `Integer.parseInt` wants at least one digit; a bare sign fails like any malformed input.
        return s.parse::<i32>().unwrap_or(default);
    }
    crate::javanum::parse_double(s).map_or(default, |d| d as i32)
}
