//! Serialization of trees and untyped Java values through a [`JsonGenerator`], and the
//! `convertValue` / `valueToTree` conversions between them.

use crate::JacksonError;
use crate::bigdecimal::BigDecimal;
use crate::generator::JsonGenerator;
use crate::node::{JsonNode, ObjectNode};
use crate::object::{JavaObject, OffsetDateTime};
use indexmap::IndexMap;
use num_bigint::BigInt;
use std::any::Any;
use std::sync::Arc;

/// The `ObjectMapper` serialization settings that affect untyped values.
#[derive(Clone, Copy)]
pub struct SerConfig<'a> {
    /// Identifies the mapper for [`crate::Pojo::serialize`].
    pub mapper: &'a dyn Any,
    /// `SerializationFeature.WRITE_NULL_MAP_VALUES` (Jackson default true; swagger mappers set false).
    pub write_null_map_values: bool,
    /// `SerializationFeature.WRITE_DATES_AS_TIMESTAMPS` (Jackson default true; swagger mappers set false).
    pub write_dates_as_timestamps: bool,
    /// `SerializationFeature.ORDER_MAP_ENTRIES_BY_KEYS` (default false): `Map` entries written in key order.
    pub order_map_entries_by_keys: bool,
}

impl SerConfig<'static> {
    /// The settings of Jackson's `InternalNodeMapper` (used by `JsonNode.toString()`): plain defaults.
    pub fn internal_node_mapper() -> SerConfig<'static> {
        SerConfig { mapper: &(), write_null_map_values: true, write_dates_as_timestamps: true, order_map_entries_by_keys: false }
    }

    /// swagger-core `Json.mapper()`-style settings (no Pojo mapper attached).
    pub fn swagger() -> SerConfig<'static> {
        SerConfig { mapper: &(), write_null_map_values: false, write_dates_as_timestamps: false, order_map_entries_by_keys: false }
    }
}

/// Serializes a tree node (as `JsonNode.serialize` does).
pub fn write_node(g: &mut dyn JsonGenerator, node: &JsonNode, cfg: &SerConfig) -> Result<(), JacksonError> {
    match node {
        JsonNode::Object(m) => {
            g.write_start_object()?;
            for (k, v) in m.iter() {
                g.write_field_name(k)?;
                write_node(g, v, cfg)?;
            }
            g.write_end_object()
        }
        JsonNode::Array(a) => {
            g.write_start_array()?;
            for v in a.iter() {
                write_node(g, v, cfg)?;
            }
            g.write_end_array()
        }
        JsonNode::Text(s) => g.write_string(s),
        JsonNode::Binary(b) => g.write_binary(b),
        JsonNode::Boolean(b) => g.write_boolean(*b),
        JsonNode::Null | JsonNode::Missing => g.write_null(),
        JsonNode::Short(v) => g.write_number_i16(*v),
        JsonNode::Int(v) => g.write_number_i32(*v),
        JsonNode::Long(v) => g.write_number_i64(*v),
        JsonNode::BigInteger(v) => g.write_number_big_integer(v),
        JsonNode::Float(v) => g.write_number_f32(*v),
        JsonNode::Double(v) => g.write_number_f64(*v),
        JsonNode::Decimal(v) => g.write_number_big_decimal(v),
        JsonNode::Pojo(o) => write_object(g, o, cfg),
    }
}

/// Serializes an untyped Java value with the given mapper settings.
pub fn write_object(g: &mut dyn JsonGenerator, v: &JavaObject, cfg: &SerConfig) -> Result<(), JacksonError> {
    match v {
        JavaObject::Null => g.write_null(),
        JavaObject::String(s) => g.write_string(s),
        JavaObject::Boolean(b) => g.write_boolean(*b),
        JavaObject::Short(x) => g.write_number_i16(*x),
        JavaObject::Integer(x) => g.write_number_i32(*x),
        JavaObject::Long(x) => g.write_number_i64(*x),
        JavaObject::BigInteger(x) => g.write_number_big_integer(x),
        JavaObject::Float(x) => g.write_number_f32(*x),
        JavaObject::Double(x) => g.write_number_f64(*x),
        JavaObject::BigDecimal(x) => g.write_number_big_decimal(x),
        JavaObject::Map(m) => {
            g.write_start_object()?;
            let mut entries: Vec<(&String, &JavaObject)> = m.iter().collect();
            if cfg.order_map_entries_by_keys {
                // MapSerializer._orderEntries: a TreeMap (String.compareTo, UTF-16 code unit order)
                entries.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
            }
            for (k, v) in entries {
                if v.is_null() && !cfg.write_null_map_values {
                    continue;
                }
                g.write_field_name(k)?;
                write_object(g, v, cfg)?;
            }
            g.write_end_object()
        }
        JavaObject::List(l) | JavaObject::Set(l) => {
            g.write_start_array()?;
            for v in l.iter() {
                write_object(g, v, cfg)?;
            }
            g.write_end_array()
        }
        JavaObject::Bytes(b) => g.write_binary(b),
        JavaObject::Date(ms) => {
            if cfg.write_dates_as_timestamps {
                g.write_number_i64(*ms)
            } else {
                g.write_string(&std_date_format(*ms))
            }
        }
        JavaObject::OffsetDateTime(o) => {
            if cfg.write_dates_as_timestamps {
                // OffsetDateTimeSerializer as timestamp: seconds.nanos as a decimal number
                let secs = BigDecimal::new(BigInt::from(o.epoch_second) * BigInt::from(1_000_000_000i64) + BigInt::from(o.nano), 9);
                g.write_number_big_decimal(&secs)
            } else {
                g.write_string(&iso_offset_date_time(o))
            }
        }
        JavaObject::Uuid(u) => g.write_string(&uuid_to_string(*u)),
        JavaObject::Node(n) => write_node(g, n, cfg),
        JavaObject::Pojo(p) => p.serialize(g, cfg.mapper),
    }
}

/// `StdDateFormat` ISO-8601 output in UTC: `yyyy-MM-dd'T'HH:mm:ss.SSS+00:00`.
pub fn std_date_format(ms: i64) -> String {
    let (y, mo, d, h, mi, s, milli) = civil_from_millis(ms);
    format!("{}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}+00:00", format_year(y), mo, d, h, mi, s, milli)
}

/// `yyyy-MM-dd` of a `java.util.Date` in UTC (swagger's `DateSchemaMixin` `@JsonFormat`).
pub fn date_only_format(ms: i64) -> String {
    let (y, mo, d, ..) = civil_from_millis(ms);
    format!("{}-{:02}-{:02}", format_year(y), mo, d)
}

fn format_year(y: i64) -> String {
    if y < 0 { format!("-{:04}", -y) } else { format!("{:04}", y) }
}

/// Days since the epoch → (year, month, day).
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// (year, month, day) → days since the epoch.
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let mp = if m > 2 { m - 3 } else { m + 9 } as u64;
    let doy = (153 * mp + 2) / 5 + d as u64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe as i64 - 719468
}

fn civil_from_millis(ms: i64) -> (i64, u32, u32, u32, u32, u32, u32) {
    let days = ms.div_euclid(86_400_000);
    let rem = ms.rem_euclid(86_400_000);
    let (y, mo, d) = civil_from_days(days);
    let h = (rem / 3_600_000) as u32;
    let mi = ((rem / 60_000) % 60) as u32;
    let s = ((rem / 1000) % 60) as u32;
    let milli = (rem % 1000) as u32;
    (y, mo, d, h, mi, s, milli)
}

fn offset_string(total: i32) -> String {
    if total == 0 {
        return "Z".to_string();
    }
    let sign = if total < 0 { '-' } else { '+' };
    let a = total.abs();
    let (h, m, s) = (a / 3600, (a / 60) % 60, a % 60);
    if s != 0 { format!("{sign}{h:02}:{m:02}:{s:02}") } else { format!("{sign}{h:02}:{m:02}") }
}

fn local_parts(o: &OffsetDateTime) -> (i64, u32, u32, u32, u32, u32) {
    let local = o.epoch_second + o.offset_seconds as i64;
    let days = local.div_euclid(86_400);
    let rem = local.rem_euclid(86_400);
    let (y, mo, d) = civil_from_days(days);
    (y, mo, d, (rem / 3600) as u32, ((rem / 60) % 60) as u32, (rem % 60) as u32)
}

/// `DateTimeFormatter.ISO_OFFSET_DATE_TIME.format(odt)` (seconds always, fraction trimmed).
pub fn iso_offset_date_time(o: &OffsetDateTime) -> String {
    let (y, mo, d, h, mi, s) = local_parts(o);
    let mut out = format!("{}-{:02}-{:02}T{:02}:{:02}:{:02}", format_year(y), mo, d, h, mi, s);
    if o.nano != 0 {
        let frac = format!("{:09}", o.nano);
        out.push('.');
        out.push_str(frac.trim_end_matches('0'));
    }
    out.push_str(&offset_string(o.offset_seconds));
    out
}

/// `OffsetDateTime.toString()` (LocalTime.toString: seconds omitted when zero, fraction in 3/6/9 groups).
pub fn offset_date_time_to_string(o: &OffsetDateTime) -> String {
    let (y, mo, d, h, mi, s) = local_parts(o);
    let mut out = format!("{}-{:02}-{:02}T{:02}:{:02}", format_year(y), mo, d, h, mi);
    if s > 0 || o.nano > 0 {
        out.push_str(&format!(":{s:02}"));
        if o.nano > 0 {
            if o.nano.is_multiple_of(1_000_000) {
                out.push_str(&format!(".{:03}", o.nano / 1_000_000));
            } else if o.nano.is_multiple_of(1000) {
                out.push_str(&format!(".{:06}", o.nano / 1000));
            } else {
                out.push_str(&format!(".{:09}", o.nano));
            }
        }
    }
    out.push_str(&offset_string(o.offset_seconds));
    out
}

/// `UUID.toString()`
pub fn uuid_to_string(u: u128) -> String {
    let h = format!("{u:032x}");
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

/// `UUID.fromString` (Java's lenient parser: five dash-separated hex groups).
pub fn uuid_from_string(s: &str) -> Option<u128> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 5 || s.len() > 36 {
        return None;
    }
    let widths = [32u32, 16, 16, 16, 48];
    let mut v: [u64; 5] = [0; 5];
    for (i, p) in parts.iter().enumerate() {
        if p.is_empty() || p.len() > 16 {
            return None;
        }
        let x = u64::from_str_radix(p, 16).ok()?;
        v[i] = x & if widths[i] == 64 { u64::MAX } else { (1u64 << widths[i]) - 1 };
    }
    let msb: u64 = (v[0] << 32) | (v[1] << 16) | v[2];
    let lsb: u64 = (v[3] << 48) | v[4];
    Some(((msb as u128) << 64) | lsb as u128)
}

/// `ObjectMapper.convertValue(node, Object.class)`.
pub fn node_to_object(node: &JsonNode) -> JavaObject {
    match node {
        JsonNode::Object(m) => {
            let mut out = IndexMap::new();
            for (k, v) in m.iter() {
                out.insert(k.clone(), node_to_object(v));
            }
            JavaObject::Map(Arc::new(out))
        }
        JsonNode::Array(a) => JavaObject::List(Arc::new(a.iter().map(node_to_object).collect())),
        JsonNode::Text(s) => JavaObject::String(s.clone()),
        JsonNode::Binary(b) => JavaObject::Bytes(b.clone()),
        JsonNode::Boolean(b) => JavaObject::Boolean(*b),
        JsonNode::Null | JsonNode::Missing => JavaObject::Null,
        JsonNode::Short(v) => JavaObject::Integer(*v as i32),
        JsonNode::Int(v) => JavaObject::Integer(*v),
        JsonNode::Long(v) => JavaObject::Long(*v),
        JsonNode::BigInteger(v) => JavaObject::BigInteger(v.clone()),
        JsonNode::Float(v) => JavaObject::Double(*v as f64),
        JsonNode::Double(v) => JavaObject::Double(*v),
        JsonNode::Decimal(v) => JavaObject::BigDecimal(v.clone()),
        JsonNode::Pojo(o) => (**o).clone(),
    }
}

/// Builds a tree from generator calls (`TokenBuffer` + `readTree`): `valueToTree`.
pub struct TreeGenerator {
    stack: Vec<Frame>,
    result: Option<JsonNode>,
    strip_decimals: bool,
}

enum Frame {
    Object(ObjectNode, Option<String>),
    Array(Vec<JsonNode>),
}

impl Default for TreeGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl TreeGenerator {
    /// An empty builder; the tree is ready once one root value has been written.
    pub fn new() -> TreeGenerator {
        TreeGenerator { stack: Vec::new(), result: None, strip_decimals: false }
    }

    /// A generator whose `BigDecimal`s become `DecimalNode`s with trailing zeros stripped, as
    /// `JsonNodeDeserializer` builds them when a tree is read from a token stream (`convertValue(x,
    /// JsonNode.class)`, `readTree(tokenBuffer)`; `JsonNodeFeature.STRIP_TRAILING_BIGDECIMAL_ZEROES`).
    pub fn with_stripped_decimals() -> TreeGenerator {
        TreeGenerator { stack: Vec::new(), result: None, strip_decimals: true }
    }

    /// The finished tree (`MissingNode` if nothing was written; Jackson would return null).
    pub fn into_node(self) -> JsonNode {
        self.result.unwrap_or(JsonNode::Missing)
    }

    fn value(&mut self, v: JsonNode) -> Result<(), JacksonError> {
        match self.stack.last_mut() {
            None => self.result = Some(v),
            Some(Frame::Array(a)) => a.push(v),
            Some(Frame::Object(m, name)) => {
                let k = name.take().ok_or_else(|| JacksonError { class: "com.fasterxml.jackson.core.JsonGenerationException".into(), message: "Can not write a value, expecting field name".into() })?;
                m.insert(k, v);
            }
        }
        Ok(())
    }
}

impl JsonGenerator for TreeGenerator {
    fn write_start_object(&mut self) -> Result<(), JacksonError> {
        self.stack.push(Frame::Object(ObjectNode::new(), None));
        Ok(())
    }
    fn write_end_object(&mut self) -> Result<(), JacksonError> {
        match self.stack.pop() {
            Some(Frame::Object(m, _)) => self.value(JsonNode::Object(Arc::new(m))),
            _ => Err(JacksonError { class: "com.fasterxml.jackson.core.JsonGenerationException".into(), message: "Current context not Object".into() }),
        }
    }
    fn write_start_array(&mut self) -> Result<(), JacksonError> {
        self.stack.push(Frame::Array(Vec::new()));
        Ok(())
    }
    fn write_end_array(&mut self) -> Result<(), JacksonError> {
        match self.stack.pop() {
            Some(Frame::Array(a)) => self.value(JsonNode::Array(Arc::new(a))),
            _ => Err(JacksonError { class: "com.fasterxml.jackson.core.JsonGenerationException".into(), message: "Current context not Array".into() }),
        }
    }
    fn write_field_name(&mut self, name: &str) -> Result<(), JacksonError> {
        match self.stack.last_mut() {
            Some(Frame::Object(_, n)) => {
                *n = Some(name.to_string());
                Ok(())
            }
            _ => Err(JacksonError { class: "com.fasterxml.jackson.core.JsonGenerationException".into(), message: "Can not write a field name, expecting a value".into() }),
        }
    }
    fn write_string(&mut self, value: &str) -> Result<(), JacksonError> {
        self.value(JsonNode::Text(value.into()))
    }
    fn write_boolean(&mut self, value: bool) -> Result<(), JacksonError> {
        self.value(JsonNode::Boolean(value))
    }
    fn write_null(&mut self) -> Result<(), JacksonError> {
        self.value(JsonNode::Null)
    }
    fn write_number_i16(&mut self, value: i16) -> Result<(), JacksonError> {
        self.value(JsonNode::Int(value as i32))
    }
    fn write_number_i32(&mut self, value: i32) -> Result<(), JacksonError> {
        self.value(JsonNode::Int(value))
    }
    fn write_number_i64(&mut self, value: i64) -> Result<(), JacksonError> {
        self.value(JsonNode::Long(value))
    }
    fn write_number_big_integer(&mut self, value: &BigInt) -> Result<(), JacksonError> {
        self.value(JsonNode::BigInteger(Arc::new(value.clone())))
    }
    fn write_number_f32(&mut self, value: f32) -> Result<(), JacksonError> {
        self.value(JsonNode::Float(value))
    }
    fn write_number_f64(&mut self, value: f64) -> Result<(), JacksonError> {
        self.value(JsonNode::Double(value))
    }
    fn write_number_big_decimal(&mut self, value: &BigDecimal) -> Result<(), JacksonError> {
        let v = if self.strip_decimals { value.strip_trailing_zeros() } else { value.clone() };
        self.value(JsonNode::Decimal(Arc::new(v)))
    }
    fn write_binary(&mut self, value: &[u8]) -> Result<(), JacksonError> {
        self.value(JsonNode::Binary(value.into()))
    }
}

/// `ObjectMapper.valueToTree(obj)` / `convertValue(obj, JsonNode.class)`.
pub fn object_to_node(v: &JavaObject, cfg: &SerConfig) -> Result<JsonNode, JacksonError> {
    if let JavaObject::Node(n) = v {
        return Ok(n.clone());
    }
    let mut g = TreeGenerator::new();
    write_object(&mut g, v, cfg)?;
    Ok(match g.result {
        Some(n) => n,
        None => JsonNode::Null,
    })
}

/// Serializes an untyped value to compact JSON text (`writeValueAsString`).
pub fn object_to_json_string(v: &JavaObject, cfg: &SerConfig, big_decimal_as_plain: bool) -> Result<String, JacksonError> {
    let mut w = crate::json::JsonWriter::new(crate::json::WriteFeatures { big_decimal_as_plain, ..Default::default() });
    write_object(&mut w, v, cfg)?;
    Ok(w.into_string())
}

/// Serializes a tree to compact (or pretty) JSON text.
pub fn node_to_json_string(n: &JsonNode, pretty: bool, big_decimal_as_plain: bool) -> String {
    let mut w = crate::json::JsonWriter::new(crate::json::WriteFeatures { pretty, big_decimal_as_plain, ..Default::default() });
    let _ = write_node(&mut w, n, &SerConfig::swagger());
    w.into_string()
}
