//! Jackson serialization of the models: shared helpers, the custom serializers swagger-core registers
//! (`SchemaSerializer`, `Schema31Serializer`, `MediaTypeSerializer`, `ExampleSerializer`, `PathsSerializer`,
//! `ApiResponsesSerializer`, `CallbackSerializer`) and the `Json.pretty()`-style entry points.

use super::*;
use crate::java::JMap;
use jackson::convert::{SerConfig, write_object as jackson_write_object};
use jackson::json::{JsonWriter, WriteFeatures};
use jackson::{JacksonError, JavaObject, JsonGenerator};

/// The configured `ObjectMapper` doing the serialization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mapper {
    /// `Json.mapper()` / `Yaml.mapper()` (OpenAPI 3.0 mixins, `SchemaSerializer`)
    Json30,
    /// `Json31.mapper()` / `Yaml31.mapper()` (3.1 mixins, `Schema31Serializer`)
    Json31,
    /// `Json31.converterMapper()` (`SchemaConverterMixin`, no custom serializers)
    Converter,
}

pub(crate) fn ser_config(m: &Mapper) -> SerConfig<'_> {
    SerConfig { mapper: m, write_null_map_values: false, write_dates_as_timestamps: false, order_map_entries_by_keys: features().order_map_entries_by_keys }
}

/// Features switched on a copy of a swagger-core mapper (`mapper.copy().configure(feature, true)`); both are
/// off on the swagger-core mappers themselves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MapperFeatures {
    /// `MapperFeature.SORT_PROPERTIES_ALPHABETICALLY`: bean properties without an explicit
    /// `@JsonPropertyOrder` are written in name order (the `@JsonAnyGetter` entries stay last).
    pub sort_properties_alphabetically: bool,
    /// `SerializationFeature.ORDER_MAP_ENTRIES_BY_KEYS`: `Map` values (including any-getter maps) are written
    /// in key order.
    pub order_map_entries_by_keys: bool,
}

thread_local! {
    static FEATURES: std::cell::Cell<MapperFeatures> = const { std::cell::Cell::new(MapperFeatures { sort_properties_alphabetically: false, order_map_entries_by_keys: false }) };
}

/// The features of the serialization running on this thread.
pub(crate) fn features() -> MapperFeatures {
    FEATURES.with(|f| f.get())
}

/// Runs `f` with `features` in effect on this thread (restored afterwards).
fn with_features<T>(features: MapperFeatures, f: impl FnOnce() -> T) -> T {
    struct Restore(MapperFeatures);
    impl Drop for Restore {
        fn drop(&mut self) {
            FEATURES.with(|f| f.set(self.0));
        }
    }
    let _restore = Restore(FEATURES.with(|c| c.replace(features)));
    f()
}

/// `String.compareTo` (UTF-16 code unit order), the natural order of a `TreeMap<String, ?>`.
fn java_string_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// Map entries in the order `MapSerializer` writes them: insertion order, or key order with
/// `ORDER_MAP_ENTRIES_BY_KEYS` (a map holding a null key is left as is: `TreeMap` cannot take it and
/// serialization fails on the null key anyway).
pub(crate) fn ordered_entries<V>(mut entries: Vec<(String, V)>) -> Vec<(String, V)> {
    if features().order_map_entries_by_keys && !entries.iter().any(|(k, _)| k == crate::java::collections::NULL_KEY) {
        entries.sort_by(|a, b| java_string_cmp(&a.0, &b.0));
    }
    entries
}

/// One recorded generator call.
enum Event {
    StartObject,
    EndObject,
    StartArray,
    EndArray,
    FieldName(String),
    String(String),
    Boolean(bool),
    Null,
    I16(i16),
    I32(i32),
    I64(i64),
    BigInteger(num_bigint::BigInt),
    F32(f32),
    F64(f64),
    BigDecimal(jackson::BigDecimal),
    Binary(Vec<u8>),
}

/// Records a bean's properties (one group of events per top-level field) so they can be written in
/// `SORT_PROPERTIES_ALPHABETICALLY` order.
#[derive(Default)]
struct PropertyRecorder {
    /// (property name, events, written by the any-getter)
    fields: Vec<(String, Vec<Event>, bool)>,
    depth: usize,
    in_any_getter: bool,
}

impl PropertyRecorder {
    fn push(&mut self, e: Event) -> Result<(), JacksonError> {
        match (&e, self.depth) {
            (Event::FieldName(n), 0) => {
                self.fields.push((n.clone(), vec![], self.in_any_getter));
                return Ok(());
            }
            (Event::StartObject | Event::StartArray, _) => self.depth += 1,
            (Event::EndObject | Event::EndArray, _) => self.depth = self.depth.saturating_sub(1),
            _ => {}
        }
        match self.fields.last_mut() {
            Some(f) => {
                f.1.push(e);
                Ok(())
            }
            None => Err(JacksonError { class: "com.fasterxml.jackson.core.JsonGenerationException".into(), message: "Can not write a value, expecting field name".into() }),
        }
    }

    /// Regular properties in name order, then the any-getter entries as written.
    fn replay_sorted(self, g: &mut dyn JsonGenerator) -> Result<(), JacksonError> {
        let (mut regular, any): (Vec<_>, Vec<_>) = self.fields.into_iter().partition(|f| !f.2);
        regular.sort_by(|a, b| java_string_cmp(&a.0, &b.0));
        for (name, events, _) in regular.into_iter().chain(any) {
            g.write_field_name(&name)?;
            for e in events {
                match e {
                    Event::StartObject => g.write_start_object()?,
                    Event::EndObject => g.write_end_object()?,
                    Event::StartArray => g.write_start_array()?,
                    Event::EndArray => g.write_end_array()?,
                    Event::FieldName(n) => g.write_field_name(&n)?,
                    Event::String(s) => g.write_string(&s)?,
                    Event::Boolean(b) => g.write_boolean(b)?,
                    Event::Null => g.write_null()?,
                    Event::I16(v) => g.write_number_i16(v)?,
                    Event::I32(v) => g.write_number_i32(v)?,
                    Event::I64(v) => g.write_number_i64(v)?,
                    Event::BigInteger(v) => g.write_number_big_integer(&v)?,
                    Event::F32(v) => g.write_number_f32(v)?,
                    Event::F64(v) => g.write_number_f64(v)?,
                    Event::BigDecimal(v) => g.write_number_big_decimal(&v)?,
                    Event::Binary(v) => g.write_binary(&v)?,
                }
            }
        }
        Ok(())
    }
}

impl JsonGenerator for PropertyRecorder {
    fn write_start_object(&mut self) -> Result<(), JacksonError> {
        self.push(Event::StartObject)
    }
    fn write_end_object(&mut self) -> Result<(), JacksonError> {
        self.push(Event::EndObject)
    }
    fn write_start_array(&mut self) -> Result<(), JacksonError> {
        self.push(Event::StartArray)
    }
    fn write_end_array(&mut self) -> Result<(), JacksonError> {
        self.push(Event::EndArray)
    }
    fn write_field_name(&mut self, name: &str) -> Result<(), JacksonError> {
        self.push(Event::FieldName(name.to_string()))
    }
    fn write_string(&mut self, value: &str) -> Result<(), JacksonError> {
        self.push(Event::String(value.to_string()))
    }
    fn write_boolean(&mut self, value: bool) -> Result<(), JacksonError> {
        self.push(Event::Boolean(value))
    }
    fn write_null(&mut self) -> Result<(), JacksonError> {
        self.push(Event::Null)
    }
    fn write_number_i16(&mut self, value: i16) -> Result<(), JacksonError> {
        self.push(Event::I16(value))
    }
    fn write_number_i32(&mut self, value: i32) -> Result<(), JacksonError> {
        self.push(Event::I32(value))
    }
    fn write_number_i64(&mut self, value: i64) -> Result<(), JacksonError> {
        self.push(Event::I64(value))
    }
    fn write_number_big_integer(&mut self, value: &num_bigint::BigInt) -> Result<(), JacksonError> {
        self.push(Event::BigInteger(value.clone()))
    }
    fn write_number_f32(&mut self, value: f32) -> Result<(), JacksonError> {
        self.push(Event::F32(value))
    }
    fn write_number_f64(&mut self, value: f64) -> Result<(), JacksonError> {
        self.push(Event::F64(value))
    }
    fn write_number_big_decimal(&mut self, value: &jackson::BigDecimal) -> Result<(), JacksonError> {
        self.push(Event::BigDecimal(value.clone()))
    }
    fn write_binary(&mut self, value: &[u8]) -> Result<(), JacksonError> {
        self.push(Event::Binary(value.to_vec()))
    }
    fn mark_any_getter(&mut self) {
        if self.depth == 0 {
            self.in_any_getter = true;
        }
    }
}

/// A bean serializer's properties (`fields` writes them in declaration order), reordered alphabetically when
/// `SORT_PROPERTIES_ALPHABETICALLY` is on.
pub(crate) fn write_bean_properties(
    g: &mut dyn JsonGenerator,
    fields: impl FnOnce(&mut dyn JsonGenerator) -> Result<(), JacksonError>,
) -> Result<(), JacksonError> {
    if !features().sort_properties_alphabetically {
        return fields(g);
    }
    let mut r = PropertyRecorder::default();
    fields(&mut r)?;
    r.replay_sorted(g)
}

/// An `Object`-typed property value.
pub(crate) fn write_object(g: &mut dyn JsonGenerator, v: &JavaObject, m: Mapper) -> Result<(), JacksonError> {
    jackson_write_object(g, v, &ser_config(&m))
}

/// An `Object` property annotated `@JsonInclude(value = NON_NULL, content = ALWAYS)` (`example`, `value`):
/// null map entries are written.
pub(crate) fn write_object_content_always(g: &mut dyn JsonGenerator, v: &JavaObject, m: Mapper) -> Result<(), JacksonError> {
    let mut cfg = ser_config(&m);
    cfg.write_null_map_values = true;
    jackson_write_object(g, v, &cfg)
}

/// A `Map<String, V>` property (`MapSerializer`; null values cannot occur in a `JMap<V>` except
/// `JavaObject::Null`, which `WRITE_NULL_MAP_VALUES=false` drops).
pub(crate) fn write_map<V: Clone + 'static>(
    g: &mut dyn JsonGenerator,
    map: &JMap<V>,
    _m: Mapper,
    mut f: impl FnMut(&mut dyn JsonGenerator, &V) -> Result<(), JacksonError>,
) -> Result<(), JacksonError> {
    g.write_start_object()?;
    for (k, v) in ordered_entries(map.entries()) {
        if k == crate::java::collections::NULL_KEY {
            return Err(JacksonError {
                class: "com.fasterxml.jackson.databind.JsonMappingException".into(),
                message: "Null key for a Map not allowed in JSON (use a converting NullKeySerializer?)".into(),
            });
        }
        if let Some(o) = (&v as &dyn std::any::Any).downcast_ref::<JavaObject>() && o.is_null() {
            continue;
        }
        g.write_field_name(&k)?;
        f(g, &v)?;
    }
    g.write_end_object()
}

/// `@JsonAnyGetter` extensions: entries written inline as fields.
pub(crate) fn write_any_getter(g: &mut dyn JsonGenerator, map: &JMap<JavaObject>, m: Mapper, suppress_nulls: bool) -> Result<(), JacksonError> {
    g.mark_any_getter();
    for (k, v) in ordered_entries(map.entries()) {
        if v.is_null() && suppress_nulls {
            continue;
        }
        g.write_field_name(&k)?;
        write_object(g, &v, m)?;
    }
    Ok(())
}

/// `Schema31Mixin.TypeSerializer`: one type → string, several → array (an empty set writes nothing, which
/// makes Jackson fail on the dangling field name).
pub(crate) fn write_types(g: &mut dyn JsonGenerator, types: &JSet) -> Result<(), JacksonError> {
    let items = types.items();
    if items.len() == 1 {
        g.write_field_name("type")?;
        g.write_string(&items[0])?;
    } else if items.len() > 1 {
        g.write_field_name("type")?;
        g.write_start_array()?;
        for t in items {
            g.write_string(&t)?;
        }
        g.write_end_array()?;
    }
    Ok(())
}

macro_rules! plain_bean {
    ($($name:ident),* $(,)?) => {
        $(
            impl $name {
                /// Bean serialization (`{` + properties + `}`).
                pub fn serialize(&self, g: &mut dyn JsonGenerator, m: Mapper) -> Result<(), JacksonError> {
                    g.write_start_object()?;
                    write_bean_properties(g, |g| self.serialize_fields(g, m))?;
                    g.write_end_object()
                }
            }
        )*
    };
}

plain_bean!(
    OpenAPI,
    Components,
    ExternalDocumentation,
    Operation,
    PathItem,
    Header,
    Contact,
    Info,
    License,
    Link,
    LinkParameter,
    Discriminator,
    Encoding,
    EncodingProperty,
    XML,
    Parameter,
    RequestBody,
    ApiResponse,
    OAuthFlow,
    OAuthFlows,
    SecurityScheme,
    Server,
    ServerVariable,
    Tag,
);

impl Schema {
    /// `SchemaSerializer` (3.0) / `Schema31Serializer` (3.1) / plain bean (converter).
    pub fn serialize(&self, g: &mut dyn JsonGenerator, m: Mapper) -> Result<(), JacksonError> {
        match m {
            Mapper::Json30 => {
                if let Some(r) = self.get_ref().filter(|r| !r.trim().is_empty()) {
                    g.write_start_object()?;
                    g.write_field_name("$ref")?;
                    g.write_string(&r)?;
                    return g.write_end_object();
                }
                self.serialize_with_null_flags(g, m)
            }
            Mapper::Json31 => {
                if let Some(b) = self.get_boolean_schema_value() {
                    return g.write_boolean(b);
                }
                self.serialize_with_null_flags(g, m)
            }
            Mapper::Converter => {
                g.write_start_object()?;
                write_bean_properties(g, |g| self.serialize_fields(g, m))?;
                g.write_end_object()
            }
        }
    }

    fn serialize_with_null_flags(&self, g: &mut dyn JsonGenerator, m: Mapper) -> Result<(), JacksonError> {
        let (null_example, null_default) = {
            let d = self.read();
            (d.example_set_flag && d.example.is_none(), d.default_set_flag && d.default_.is_none())
        };
        g.write_start_object()?;
        if m == Mapper::Json31 {
            // Schema31Mixin: @JsonPropertyOrder({type, format, if, then, else}, alphabetic = true) — the
            // order is fixed whatever the mapper's SORT_PROPERTIES_ALPHABETICALLY
            self.serialize_fields(g, m)?;
        } else {
            write_bean_properties(g, |g| self.serialize_fields(g, m))?;
        }
        if null_example {
            g.write_field_name("example")?;
            g.write_null()?;
        }
        if null_default {
            g.write_field_name("default")?;
            g.write_null()?;
        }
        g.write_end_object()
    }
}

impl MediaType {
    /// `MediaTypeSerializer` (not registered on the converter mapper).
    pub fn serialize(&self, g: &mut dyn JsonGenerator, m: Mapper) -> Result<(), JacksonError> {
        let null_example = { let d = self.read(); d.example_set_flag && d.example.is_none() };
        g.write_start_object()?;
        write_bean_properties(g, |g| self.serialize_fields(g, m))?;
        if null_example && m != Mapper::Converter {
            g.write_field_name("example")?;
            g.write_null()?;
        }
        g.write_end_object()
    }
}

impl Example {
    /// `ExampleSerializer` (not registered on the converter mapper).
    pub fn serialize(&self, g: &mut dyn JsonGenerator, m: Mapper) -> Result<(), JacksonError> {
        let null_value = { let d = self.read(); d.value_set_flag && d.value.is_none() };
        g.write_start_object()?;
        write_bean_properties(g, |g| self.serialize_fields(g, m))?;
        if null_value && m != Mapper::Converter {
            g.write_field_name("value")?;
            g.write_null()?;
        }
        g.write_end_object()
    }
}

impl Paths {
    /// `PathsSerializer`: entries then extensions when there are extensions, else the plain map.
    pub fn serialize_with_extensions(&self, g: &mut dyn JsonGenerator, m: Mapper) -> Result<(), JacksonError> {
        match self.get_extensions().filter(|e| !e.is_empty()) {
            Some(ext) => {
                g.write_start_object()?;
                for (k, v) in self.entries() {
                    g.write_field_name(&k)?;
                    v.serialize(g, m)?;
                }
                for (k, v) in ext.entries() {
                    g.write_field_name(&k)?;
                    write_object(g, &v, m)?;
                }
                g.write_end_object()
            }
            None => self.serialize(g, m),
        }
    }
}

impl ApiResponses {
    /// `ApiResponsesSerializer`
    pub fn serialize_with_extensions(&self, g: &mut dyn JsonGenerator, m: Mapper) -> Result<(), JacksonError> {
        match self.get_extensions().filter(|e| !e.is_empty()) {
            Some(ext) => {
                g.write_start_object()?;
                for (k, v) in self.entries() {
                    g.write_field_name(&k)?;
                    v.serialize(g, m)?;
                }
                for (k, v) in ext.entries() {
                    g.write_field_name(&k)?;
                    write_object(g, &v, m)?;
                }
                g.write_end_object()
            }
            None => self.serialize(g, m),
        }
    }
}

impl Callback {
    /// `CallbackSerializer` (used for `Components.callbacks` / `Operation.callbacks` values).
    pub fn serialize_callback(&self, g: &mut dyn JsonGenerator, m: Mapper) -> Result<(), JacksonError> {
        let ext = self.get_extensions().filter(|e| !e.is_empty());
        let r = self.get_ref().filter(|r| !r.trim().is_empty());
        if let Some(ext) = ext {
            g.write_start_object()?;
            match r {
                None => {
                    for (k, v) in self.entries() {
                        g.write_field_name(&k)?;
                        v.serialize(g, m)?;
                    }
                }
                Some(r) => {
                    g.write_field_name("$ref")?;
                    g.write_string(&r)?;
                }
            }
            for (k, v) in ext.entries() {
                g.write_field_name(&k)?;
                write_object(g, &v, m)?;
            }
            g.write_end_object()
        } else if let Some(r) = r {
            g.write_start_object()?;
            g.write_field_name("$ref")?;
            g.write_string(&r)?;
            g.write_end_object()
        } else {
            self.serialize(g, m)
        }
    }
}

/// A model that can be serialized with a swagger-core mapper.
pub trait Serialize {
    /// Writes this model to `g` the way mapper `m` (`Json30`, `Json31` or `Converter`) would.
    fn serialize_with(&self, g: &mut dyn JsonGenerator, m: Mapper) -> Result<(), JacksonError>;
}

macro_rules! serialize_impl {
    ($($name:ident),* $(,)?) => {
        $(impl Serialize for $name {
            fn serialize_with(&self, g: &mut dyn JsonGenerator, m: Mapper) -> Result<(), JacksonError> {
                self.serialize(g, m)
            }
        })*
    };
}

serialize_impl!(
    OpenAPI, Components, ExternalDocumentation, Operation, PathItem, Header, Contact, Info, License, Link, LinkParameter, Discriminator,
    Encoding, EncodingProperty, XML, Parameter, RequestBody, ApiResponse, OAuthFlow, OAuthFlows, SecurityScheme, Server, ServerVariable,
    Tag, Schema, MediaType, Example, Paths, Callback, Content, ApiResponses, Scopes, SecurityRequirement, ServerVariables,
);

/// A `Map<String, Model>` (e.g. `Yaml.pretty(components.getSchemas())`): Jackson's `MapSerializer`, entries in
/// the map's iteration order.
impl<V: Serialize + Clone> Serialize for JMap<V> {
    fn serialize_with(&self, g: &mut dyn JsonGenerator, m: Mapper) -> Result<(), JacksonError> {
        g.write_start_object()?;
        for (k, v) in self.entries() {
            g.write_field_name(&k)?;
            v.serialize_with(g, m)?;
        }
        g.write_end_object()
    }
}

/// An untyped `Object` given to the mapper directly (e.g. `Json.mapper().writeValueAsString(schema.getExample())`).
impl Serialize for JavaObject {
    fn serialize_with(&self, g: &mut dyn JsonGenerator, m: Mapper) -> Result<(), JacksonError> {
        write_object(g, self, m)
    }
}

fn write_to_string(v: &dyn Serialize, m: Mapper, pretty: bool) -> Result<String, JacksonError> {
    let mut w = JsonWriter::new(WriteFeatures { pretty, big_decimal_as_plain: true, quote_non_numeric_numbers: true });
    v.serialize_with(&mut w, m)?;
    Ok(w.into_string())
}

/// A copy of the JSON mapper `m` with `features` switched on: `writer(new DefaultPrettyPrinter())` when
/// `pretty`, then `writeValueAsString(v)`.
pub fn json_string_with(v: &dyn Serialize, m: Mapper, features: MapperFeatures, pretty: bool) -> Result<String, JacksonError> {
    with_features(features, || write_to_string(v, m, pretty))
}

/// `Json.mapper().writeValueAsString(v)`
pub fn json_string(v: &dyn Serialize) -> Result<String, JacksonError> {
    write_to_string(v, Mapper::Json30, false)
}

/// `Json.pretty(v)` (Java returns null on failure)
pub fn json_pretty(v: &dyn Serialize) -> Option<String> {
    write_to_string(v, Mapper::Json30, true).ok()
}

/// `Json31.mapper().writeValueAsString(v)`
pub fn json31_string(v: &dyn Serialize) -> Result<String, JacksonError> {
    write_to_string(v, Mapper::Json31, false)
}

/// `Json31.pretty(v)`
pub fn json31_pretty(v: &dyn Serialize) -> Option<String> {
    write_to_string(v, Mapper::Json31, true).ok()
}

fn write_yaml(v: &dyn Serialize, m: Mapper) -> Result<String, JacksonError> {
    let mut w = jackson::yaml::YamlWriter::new(jackson::yaml::YamlGeneratorFeatures::swagger());
    v.serialize_with(&mut w, m)?;
    w.into_string()
}

/// `Yaml.mapper().writeValueAsString(v)`
pub fn yaml_string(v: &dyn Serialize) -> Result<String, JacksonError> {
    write_yaml(v, Mapper::Json30)
}

/// `Yaml.pretty(v)` (Java returns null on failure)
pub fn yaml_pretty(v: &dyn Serialize) -> Option<String> {
    write_yaml(v, Mapper::Json30).ok()
}

/// `Yaml31.mapper().writeValueAsString(v)`
pub fn yaml31_string(v: &dyn Serialize) -> Result<String, JacksonError> {
    write_yaml(v, Mapper::Json31)
}

/// `Yaml31.pretty(v)`
pub fn yaml31_pretty(v: &dyn Serialize) -> Option<String> {
    write_yaml(v, Mapper::Json31).ok()
}

/// `mapper.valueToTree(v)` / `convertValue(v, JsonNode.class)`
pub fn value_to_tree(v: &dyn Serialize, m: Mapper) -> Result<jackson::JsonNode, JacksonError> {
    let mut g = jackson::convert::TreeGenerator::new();
    v.serialize_with(&mut g, m)?;
    Ok(g.into_node())
}
