//! `io.swagger.models.properties.PropertyBuilder`, swagger-core 1.x's `PropertyDeserializer`
//! (`Json.mapper().convertValue(node, Property.class)`) and `PropertyModelConverter.modelToProperty`.

use super::models::*;
use crate::java::{JMap, JResult, JavaException, MapKind};
use jackson::convert::node_to_object;
use jackson::{BigDecimal, JavaObject, JsonNode};
use std::collections::HashMap;

/// `PropertyBuilder.PropertyId`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PropertyId {
    /// `PropertyId.ENUM`: the `enum` argument.
    Enum,
    /// `PropertyId.TITLE`: the `title` argument.
    Title,
    /// `PropertyId.DESCRIPTION`: the `description` argument.
    Description,
    /// `PropertyId.DEFAULT`: the `default` argument.
    Default,
    /// `PropertyId.PATTERN`: the `pattern` argument.
    Pattern,
    /// `PropertyId.DISCRIMINATOR`: the `discriminator` argument.
    Discriminator,
    /// `PropertyId.MIN_ITEMS`: the `minItems` argument.
    MinItems,
    /// `PropertyId.MAX_ITEMS`: the `maxItems` argument.
    MaxItems,
    /// `PropertyId.MIN_PROPERTIES`: the `minProperties` argument.
    MinProperties,
    /// `PropertyId.MAX_PROPERTIES`: the `maxProperties` argument.
    MaxProperties,
    /// `PropertyId.MIN_LENGTH`: the `minLength` argument.
    MinLength,
    /// `PropertyId.MAX_LENGTH`: the `maxLength` argument.
    MaxLength,
    /// `PropertyId.MINIMUM`: the `minimum` argument.
    Minimum,
    /// `PropertyId.MAXIMUM`: the `maximum` argument.
    Maximum,
    /// `PropertyId.EXCLUSIVE_MINIMUM`: the `exclusiveMinimum` argument.
    ExclusiveMinimum,
    /// `PropertyId.EXCLUSIVE_MAXIMUM`: the `exclusiveMaximum` argument.
    ExclusiveMaximum,
    /// `PropertyId.UNIQUE_ITEMS`: the `uniqueItems` argument.
    UniqueItems,
    /// `PropertyId.EXAMPLE`: the `example` argument.
    Example,
    /// `PropertyId.TYPE`: the `type` argument.
    Type,
    /// `PropertyId.FORMAT`: the `format` argument.
    Format,
    /// `PropertyId.READ_ONLY`: the `readOnly` argument.
    ReadOnly,
    /// `PropertyId.REQUIRED`: the `required` argument.
    Required,
    /// `PropertyId.VENDOR_EXTENSIONS`: the `vendorExtensions` argument.
    VendorExtensions,
    /// `PropertyId.ALLOW_EMPTY_VALUE`: the `allowEmptyValue` argument.
    AllowEmptyValue,
    /// `PropertyId.MULTIPLE_OF`: the `multipleOf` argument.
    MultipleOf,
    /// `PropertyId.ADDITIONAL_PROPERTIES`: the `additionalProperties` argument.
    AdditionalProperties,
}

impl PropertyId {
    /// `PropertyId.getPropertyName()`: the schema keyword this id stands for, such as `minItems`.
    pub fn property_name(&self) -> &'static str {
        match self {
            PropertyId::Enum => "enum",
            PropertyId::Title => "title",
            PropertyId::Description => "description",
            PropertyId::Default => "default",
            PropertyId::Pattern => "pattern",
            PropertyId::Discriminator => "discriminator",
            PropertyId::MinItems => "minItems",
            PropertyId::MaxItems => "maxItems",
            PropertyId::MinProperties => "minProperties",
            PropertyId::MaxProperties => "maxProperties",
            PropertyId::MinLength => "minLength",
            PropertyId::MaxLength => "maxLength",
            PropertyId::Minimum => "minimum",
            PropertyId::Maximum => "maximum",
            PropertyId::ExclusiveMinimum => "exclusiveMinimum",
            PropertyId::ExclusiveMaximum => "exclusiveMaximum",
            PropertyId::UniqueItems => "uniqueItems",
            PropertyId::Example => "example",
            PropertyId::Type => "type",
            PropertyId::Format => "format",
            PropertyId::ReadOnly => "readOnly",
            PropertyId::Required => "required",
            PropertyId::VendorExtensions => "vendorExtensions",
            PropertyId::AllowEmptyValue => "allowEmptyValue",
            PropertyId::MultipleOf => "multipleOf",
            PropertyId::AdditionalProperties => "additionalProperties",
        }
    }
}

/// A value of the `Map<PropertyId, Object>` argument map.
#[derive(Clone, Debug)]
pub enum Arg {
    /// A key present with a null value (it still applies).
    Null,
    /// A `String` value.
    Str(String),
    /// An `Integer` value.
    Int(i32),
    /// A `BigDecimal` value.
    Decimal(BigDecimal),
    /// A `Boolean` value.
    Bool(bool),
    /// A `List<String>` value (the `enum` values).
    List(Vec<String>),
    /// A vendor-extension map value.
    Extensions(JMap<JavaObject>),
    /// Any other object (an example, or a typed default from `ModelImpl.getDefaultValue()`)
    Object(JavaObject),
}

impl Arg {
    /// [`Arg::Str`], or [`Arg::Null`] for `None`.
    pub fn opt_str(v: Option<String>) -> Arg {
        v.map(Arg::Str).unwrap_or(Arg::Null)
    }

    /// [`Arg::Int`], or [`Arg::Null`] for `None`.
    pub fn opt_int(v: Option<i32>) -> Arg {
        v.map(Arg::Int).unwrap_or(Arg::Null)
    }

    /// [`Arg::Decimal`], or [`Arg::Null`] for `None`.
    pub fn opt_decimal(v: Option<BigDecimal>) -> Arg {
        v.map(Arg::Decimal).unwrap_or(Arg::Null)
    }

    /// [`Arg::Bool`], or [`Arg::Null`] for `None`.
    pub fn opt_bool(v: Option<bool>) -> Arg {
        v.map(Arg::Bool).unwrap_or(Arg::Null)
    }

    /// [`Arg::List`], or [`Arg::Null`] for `None`.
    pub fn opt_list(v: Option<Vec<String>>) -> Arg {
        v.map(Arg::List).unwrap_or(Arg::Null)
    }

    /// An arbitrary object: [`Arg::Null`] for `None` or a Java null, [`Arg::Str`] for a string, otherwise [`Arg::Object`].
    pub fn opt_object(v: Option<JavaObject>) -> Arg {
        match v {
            None | Some(JavaObject::Null) => Arg::Null,
            Some(JavaObject::String(s)) => Arg::Str(s.to_string()),
            Some(o) => Arg::Object(o),
        }
    }

    fn java_class(&self) -> &'static str {
        match self {
            Arg::Null => "null",
            Arg::Str(_) => "java.lang.String",
            Arg::Int(_) => "java.lang.Integer",
            Arg::Decimal(_) => "java.math.BigDecimal",
            Arg::Bool(_) => "java.lang.Boolean",
            Arg::List(_) => "java.util.ArrayList",
            Arg::Extensions(_) => "java.util.LinkedHashMap",
            Arg::Object(o) => java_class_of(o),
        }
    }

    /// `(String) value`
    fn as_string(&self) -> JResult<Option<String>> {
        match self {
            Arg::Null => Ok(None),
            Arg::Str(s) => Ok(Some(s.clone())),
            other => Err(bootstrap_class_cast(other.java_class(), "java.lang.String")),
        }
    }

    /// The value as an `Object` (for `setExample(Object)`)
    fn as_object(&self) -> Option<JavaObject> {
        match self {
            Arg::Null => None,
            Arg::Str(s) => Some(JavaObject::string(s.as_str())),
            Arg::Int(i) => Some(JavaObject::Integer(*i)),
            Arg::Decimal(d) => Some(JavaObject::BigDecimal(d.clone().into())),
            Arg::Bool(b) => Some(JavaObject::Boolean(*b)),
            Arg::List(l) => Some(JavaObject::List(std::sync::Arc::new(l.iter().map(|s| JavaObject::string(s.as_str())).collect()))),
            Arg::Extensions(_) => None,
            Arg::Object(o) => Some(o.clone()),
        }
    }

    fn as_int(&self) -> Option<i32> {
        match self {
            Arg::Int(i) => Some(*i),
            _ => None,
        }
    }

    fn as_decimal(&self) -> Option<BigDecimal> {
        match self {
            Arg::Decimal(d) => Some(d.clone()),
            Arg::Object(JavaObject::BigDecimal(d)) => Some((**d).clone()),
            _ => None,
        }
    }

    fn as_bool(&self) -> Option<bool> {
        match self {
            Arg::Bool(b) => Some(*b),
            _ => None,
        }
    }

    fn as_list(&self) -> Option<Vec<String>> {
        match self {
            Arg::List(l) => Some(l.clone()),
            _ => None,
        }
    }
}

fn java_class_of(o: &JavaObject) -> &'static str {
    match o {
        JavaObject::Null => "null",
        JavaObject::String(_) => "java.lang.String",
        JavaObject::Boolean(_) => "java.lang.Boolean",
        JavaObject::Short(_) => "java.lang.Short",
        JavaObject::Integer(_) => "java.lang.Integer",
        JavaObject::Long(_) => "java.lang.Long",
        JavaObject::BigInteger(_) => "java.math.BigInteger",
        JavaObject::Float(_) => "java.lang.Float",
        JavaObject::Double(_) => "java.lang.Double",
        JavaObject::BigDecimal(_) => "java.math.BigDecimal",
        JavaObject::Map(_) => "java.util.LinkedHashMap",
        JavaObject::List(_) => "java.util.ArrayList",
        _ => "java.lang.Object",
    }
}

/// A `ClassCastException` between two JDK classes (as HotSpot words it for `java.base` classes).
pub fn bootstrap_class_cast(from: &str, to: &str) -> JavaException {
    JavaException::new(
        "java.lang.ClassCastException",
        format!("class {from} cannot be cast to class {to} ({from} and {to} are in module java.base of loader 'bootstrap')"),
    )
}

/// `Map<PropertyId, Object>`: `containsKey` matters (a present key with a null value still applies).
#[derive(Clone, Debug, Default)]
pub struct Args(pub HashMap<PropertyId, Arg>);

impl Args {
    /// An empty argument map.
    pub fn new() -> Args {
        Args(HashMap::new())
    }

    /// `map.put(k, v)`: sets the argument for `k`, replacing any earlier one.
    pub fn put(&mut self, k: PropertyId, v: Arg) {
        self.0.insert(k, v);
    }

    fn get(&self, k: PropertyId) -> Option<&Arg> {
        self.0.get(&k)
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// `PropertyBuilder.Processor`, in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Processor {
    Boolean,
    ByteArray,
    Binary,
    Date,
    DateTime,
    Int,
    Long,
    Float,
    Double,
    Integer,
    Decimal,
    File,
    Reference,
    Email,
    Uuid,
    Object,
    Untyped,
    Array,
    Map,
    String,
}

const PROCESSORS: [Processor; 20] = [
    Processor::Boolean,
    Processor::ByteArray,
    Processor::Binary,
    Processor::Date,
    Processor::DateTime,
    Processor::Int,
    Processor::Long,
    Processor::Float,
    Processor::Double,
    Processor::Integer,
    Processor::Decimal,
    Processor::File,
    Processor::Reference,
    Processor::Email,
    Processor::Uuid,
    Processor::Object,
    Processor::Untyped,
    Processor::Array,
    Processor::Map,
    Processor::String,
];

impl Processor {
    fn is_type(&self, t: Option<&str>, f: Option<&str>) -> bool {
        match self {
            Processor::Boolean => t == Some("boolean"),
            Processor::ByteArray => t == Some("string") && f == Some("byte"),
            Processor::Binary => t == Some("string") && f == Some("binary"),
            Processor::Date => t == Some("string") && f == Some("date"),
            Processor::DateTime => t == Some("string") && f == Some("date-time"),
            Processor::Int => t == Some("integer") && f == Some("int32"),
            Processor::Long => t == Some("integer") && f == Some("int64"),
            Processor::Float => t == Some("number") && f == Some("float"),
            Processor::Double => t == Some("number") && f == Some("double"),
            Processor::Integer => t == Some("integer"),
            Processor::Decimal => t == Some("number"),
            Processor::File => t.is_some_and(|t| t.to_lowercase() == "file"),
            Processor::Reference => t == Some("ref"),
            Processor::Email => t == Some("string") && f == Some("email"),
            Processor::Uuid => t == Some("string") && f == Some("uuid"),
            Processor::Object => t == Some("object"),
            Processor::Untyped => t.is_none(),
            Processor::Array => t == Some("array"),
            Processor::Map => t == Some("object") && f.is_none(),
            Processor::String => t == Some("string"),
        }
    }

    /// `isType(Property)`: `type.isInstance(property)`
    fn is_instance(&self, k: PropertyKind) -> bool {
        use PropertyKind as K;
        match self {
            Processor::Boolean => k == K::Boolean,
            Processor::ByteArray => k == K::ByteArray,
            Processor::Binary => k == K::Binary,
            Processor::Date => k == K::Date,
            Processor::DateTime => k == K::DateTime,
            Processor::Int => k == K::Integer,
            Processor::Long => k == K::Long,
            Processor::Float => k == K::Float,
            Processor::Double => k == K::Double,
            Processor::Integer => k.is_base_integer(),
            Processor::Decimal => k.is_decimal(),
            Processor::File => k == K::File,
            Processor::Reference => k == K::Ref,
            Processor::Email => k == K::Email,
            Processor::Uuid => k == K::Uuid,
            Processor::Object => k == K::Object,
            Processor::Untyped => k == K::Untyped,
            Processor::Array => k == K::Array,
            Processor::Map => k == K::Map,
            Processor::String => k.is_string_property(),
        }
    }

    fn create(&self) -> Property {
        let k = match self {
            Processor::Boolean => PropertyKind::Boolean,
            Processor::ByteArray => PropertyKind::ByteArray,
            Processor::Binary => PropertyKind::Binary,
            Processor::Date => PropertyKind::Date,
            Processor::DateTime => PropertyKind::DateTime,
            Processor::Int => PropertyKind::Integer,
            Processor::Long => PropertyKind::Long,
            Processor::Float => PropertyKind::Float,
            Processor::Double => PropertyKind::Double,
            Processor::Integer => PropertyKind::BaseInteger,
            Processor::Decimal => PropertyKind::Decimal,
            Processor::File => PropertyKind::File,
            Processor::Reference => PropertyKind::Ref,
            Processor::Email => PropertyKind::Email,
            Processor::Uuid => PropertyKind::Uuid,
            Processor::Object => PropertyKind::Object,
            Processor::Untyped => PropertyKind::Untyped,
            Processor::Array => PropertyKind::Array,
            Processor::Map => PropertyKind::Map,
            Processor::String => PropertyKind::String,
        };
        Property::new(k)
    }

    fn from_type(t: Option<&str>, f: Option<&str>) -> Option<Processor> {
        PROCESSORS.into_iter().find(|p| p.is_type(t, f))
    }

    fn from_property(k: PropertyKind) -> Option<Processor> {
        PROCESSORS.into_iter().find(|p| p.is_instance(k))
    }

    /// `Processor.merge(property, args)`, including the per-processor overrides.
    fn merge(&self, p: &mut Property, args: &Args) -> JResult<()> {
        base_merge(p, args)?;
        match self {
            Processor::Boolean => {
                if let Some(v) = args.get(PropertyId::Default) {
                    p.default_value = v.as_string()?.map(|s| JavaObject::Boolean(s.eq_ignore_ascii_case("true")));
                }
            }
            Processor::ByteArray => {
                merge_string(p, args)?;
                p.enum_strings = None;
                p.pattern = Some("^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$".into());
            }
            Processor::Int | Processor::Long | Processor::Float | Processor::Double => {
                merge_numeric(p, args);
                if let Some(v) = args.get(PropertyId::Default) {
                    match v.as_string()? {
                        Some(s) => {
                            // `_default(String)`: a parse failure leaves the default unchanged
                            let parsed = match self {
                                Processor::Int => java_parse_int(&s).map(JavaObject::Integer),
                                Processor::Long => java_parse_long(&s).map(JavaObject::Long),
                                Processor::Float => parse_float(&s).map(JavaObject::Float),
                                _ => jackson::javanum::parse_double(&s).map(JavaObject::Double),
                            };
                            if parsed.is_some() {
                                p.default_value = parsed;
                            }
                        }
                        None => p.default_value = None,
                    }
                }
            }
            Processor::Integer | Processor::Decimal => merge_numeric(p, args),
            Processor::Email | Processor::String => merge_string(p, args)?,
            Processor::Uuid => {
                if let Some(v) = args.get(PropertyId::Default) {
                    p.default_string = v.as_string()?;
                }
                if let Some(v) = args.get(PropertyId::MinLength) {
                    p.min_length = v.as_int();
                }
                if let Some(v) = args.get(PropertyId::MaxLength) {
                    p.max_length = v.as_int();
                }
                if let Some(v) = args.get(PropertyId::Pattern) {
                    p.pattern = v.as_string()?;
                }
            }
            Processor::Array => {
                if let Some(v) = args.get(PropertyId::MinItems) {
                    p.min_items = v.as_int();
                }
                if let Some(v) = args.get(PropertyId::MaxItems) {
                    p.max_items = v.as_int();
                }
                if let Some(v) = args.get(PropertyId::UniqueItems) {
                    p.set_unique_items(v.as_bool());
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// The base `Processor.merge`.
fn base_merge(p: &mut Property, args: &Args) -> JResult<()> {
    if let Some(v) = args.get(PropertyId::ReadOnly) {
        p.set_read_only(v.as_bool());
    }
    if p.format.is_none() {
        p.format = match args.get(PropertyId::Format) {
            Some(v) => v.as_string()?,
            None => None,
        };
    }
    if let Some(v) = args.get(PropertyId::AllowEmptyValue) {
        p.set_allow_empty_value(v.as_bool());
    }
    if let Some(v) = args.get(PropertyId::Title) {
        p.title = v.as_string()?;
    }
    if let Some(v) = args.get(PropertyId::Description) {
        p.description = v.as_string()?;
    }
    if let Some(v) = args.get(PropertyId::Example) {
        p.set_example(v.as_object());
    }
    if let Some(v) = args.get(PropertyId::VendorExtensions) {
        match v {
            Arg::Extensions(m) => p.set_vendor_extension_map(m),
            Arg::Null => return Err(JavaException::without_message("java.lang.NullPointerException")),
            _ => {}
        }
    }
    if let Some(Some(values)) = args.get(PropertyId::Enum).map(|v| v.as_list()) {
        let add = |p: &mut Property, v: JavaObject| {
            let l = p.enum_values.get_or_insert_with(Vec::new);
            if !l.iter().any(|x| x.java_equals(&v)) {
                l.push(v);
            }
        };
        let add_str = |p: &mut Property, v: &str| {
            let l = p.enum_strings.get_or_insert_with(Vec::new);
            if !l.iter().any(|x| x == v) {
                l.push(v.to_string());
            }
        };
        for value in &values {
            match p.kind {
                PropertyKind::Boolean => add(p, JavaObject::Boolean(value.eq_ignore_ascii_case("true"))),
                PropertyKind::Integer => {
                    if let Some(i) = java_parse_int(value) {
                        add(p, JavaObject::Integer(i));
                    }
                }
                PropertyKind::Long => {
                    if let Some(i) = java_parse_long(value) {
                        add(p, JavaObject::Long(i));
                    }
                }
                PropertyKind::Double => {
                    if let Some(d) = jackson::javanum::parse_double(value) {
                        add(p, JavaObject::Double(d));
                    }
                }
                PropertyKind::Float => {
                    if let Some(f) = parse_float(value) {
                        add(p, JavaObject::Float(f));
                    }
                }
                PropertyKind::Date | PropertyKind::DateTime | PropertyKind::Uuid => add_str(p, value),
                _ => {}
            }
        }
    }
    Ok(())
}

fn merge_numeric(p: &mut Property, args: &Args) {
    if let Some(Some(v)) = args.get(PropertyId::Minimum).map(|v| v.as_decimal()) {
        p.minimum = Some(v);
    }
    if let Some(Some(v)) = args.get(PropertyId::Maximum).map(|v| v.as_decimal()) {
        p.maximum = Some(v);
    }
    if let Some(v) = args.get(PropertyId::ExclusiveMinimum) {
        p.exclusive_minimum = v.as_bool();
    }
    if let Some(v) = args.get(PropertyId::ExclusiveMaximum) {
        p.exclusive_maximum = v.as_bool();
    }
    if let Some(Some(v)) = args.get(PropertyId::MultipleOf).map(|v| v.as_decimal()) {
        p.multiple_of = Some(v);
    }
}

fn merge_string(p: &mut Property, args: &Args) -> JResult<()> {
    if let Some(v) = args.get(PropertyId::Default) {
        p.default_string = v.as_string()?;
    }
    if let Some(v) = args.get(PropertyId::MinLength) {
        p.min_length = v.as_int();
    }
    if let Some(v) = args.get(PropertyId::MaxLength) {
        p.max_length = v.as_int();
    }
    if let Some(v) = args.get(PropertyId::Pattern) {
        p.pattern = v.as_string()?;
    }
    if let Some(v) = args.get(PropertyId::Enum) {
        p.enum_strings = v.as_list();
    }
    Ok(())
}

/// `PropertyBuilder.build(type, format, args)`
pub fn build(t: Option<&str>, f: Option<&str>, args: &Args) -> JResult<Option<Property>> {
    let Some(processor) = Processor::from_type(t, f) else { return Ok(None) };
    let mut fixed = args.clone();
    if let Some(f) = f {
        fixed.put(PropertyId::Format, Arg::Str(f.to_string()));
    }
    let mut p = processor.create();
    processor.merge(&mut p, &fixed)?;
    Ok(Some(p))
}

/// `PropertyBuilder.merge(property, args)`
pub fn merge(p: &mut Property, args: &Args) -> JResult<()> {
    if args.is_empty() {
        return Ok(());
    }
    if let Some(processor) = Processor::from_property(p.kind) {
        processor.merge(p, args)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------
// PropertyDeserializer

fn detail(node: &JsonNode, id: PropertyId) -> Option<&JsonNode> {
    node.get(id.property_name())
}

fn get_string(node: &JsonNode, id: PropertyId) -> Option<String> {
    detail(node, id).map(|n| n.as_text())
}

fn get_integer(node: &JsonNode, id: PropertyId) -> Option<i32> {
    detail(node, id).map(|n| n.int_value())
}

/// `new BigDecimal(detailNode.toString())`
fn get_big_decimal(node: &JsonNode, id: PropertyId) -> JResult<Option<BigDecimal>> {
    match detail(node, id) {
        None => Ok(None),
        Some(n) => {
            let s = n.to_java_string();
            BigDecimal::parse(&s).map(Some).map_err(|_| number_format_bigdecimal(&s))
        }
    }
}

/// The `NumberFormatException` of `new BigDecimal(String)`
pub fn number_format_bigdecimal(s: &str) -> JavaException {
    JavaException::new("java.lang.NumberFormatException", format!("Character {} is neither a decimal digit number, decimal point, nor \"e\" notation exponential mark.", s.chars().find(|c| !(c.is_ascii_digit() || *c == '.' || *c == '-' || *c == '+' || *c == 'e' || *c == 'E')).unwrap_or(' ')))
}

fn get_boolean(node: &JsonNode, id: PropertyId) -> Option<bool> {
    detail(node, id).map(|n| n.boolean_value())
}

fn get_enum(node: &JsonNode, id: PropertyId) -> Option<Vec<String>> {
    let mut out = Vec::new();
    if let Some(d) = detail(node, id) && let Some(a) = d.as_array() {
        for c in a {
            if c.is_textual() || c.is_number() {
                out.push(c.as_text());
            }
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

fn get_enum_checked(node: &JsonNode, id: PropertyId) -> JResult<Option<Vec<String>>> {
    if let Some(d) = detail(node, id) && !d.is_array() {
        return Err(JavaException::class_cast(&format!("com.fasterxml.jackson.databind.node.{}", d.node_class_name()), "com.fasterxml.jackson.databind.node.ArrayNode"));
    }
    Ok(get_enum(node, id))
}

/// `getVendorExtensions(node)`: a `HashMap` of the `x-` fields (so its iteration order is Java's hash order).
pub fn get_vendor_extensions(node: &JsonNode) -> JMap<JavaObject> {
    let m = JMap::with_kind(MapKind::Hash);
    if let Some(o) = node.as_object() {
        for (k, v) in o.iter() {
            if k.starts_with("x-") {
                m.put(k.clone(), node_to_object(v));
            }
        }
    }
    m
}

fn get_required(node: &JsonNode, id: PropertyId) -> JResult<Vec<String>> {
    match detail(node, id) {
        None => Ok(Vec::new()),
        Some(d) => match d.as_array() {
            Some(a) => Ok(a.iter().map(|n| n.as_text()).collect()),
            None => Err(JavaException::new("java.lang.RuntimeException", "Required property should be a list")),
        },
    }
}

/// `PropertyDeserializer.getXml(node)`
pub fn get_xml(node: &JsonNode) -> JResult<Option<Xml>> {
    let Some(o) = node.as_object() else { return Ok(None) };
    let Some(x) = o.get("xml") else { return Ok(None) };
    if !x.is_object() {
        return Err(JavaException::class_cast(&format!("com.fasterxml.jackson.databind.node.{}", x.node_class_name()), "com.fasterxml.jackson.databind.node.ObjectNode"));
    }
    let mut xml = Xml::default();
    if let Some(n) = x.get("name") {
        xml.name = Some(n.as_text());
    }
    if let Some(n) = x.get("namespace") {
        xml.namespace = Some(n.as_text());
    }
    if let Some(n) = x.get("prefix") {
        xml.prefix = Some(n.as_text());
    }
    if let Some(n) = x.get("attribute") {
        xml.attribute = Some(n.as_boolean());
    }
    if let Some(n) = x.get("wrapped") {
        xml.wrapped = Some(n.as_boolean());
    }
    Ok(Some(xml))
}

/// `argsFromNode(node)`
fn args_from_node(node: &JsonNode) -> JResult<Args> {
    let mut a = Args::new();
    a.put(PropertyId::Type, Arg::opt_str(get_string(node, PropertyId::Type)));
    a.put(PropertyId::Format, Arg::opt_str(get_string(node, PropertyId::Format)));
    a.put(PropertyId::Description, Arg::opt_str(get_string(node, PropertyId::Description)));
    a.put(PropertyId::Example, Arg::opt_str(get_string(node, PropertyId::Example)));
    a.put(PropertyId::Enum, Arg::opt_list(get_enum_checked(node, PropertyId::Enum)?));
    a.put(PropertyId::Title, Arg::opt_str(get_string(node, PropertyId::Title)));
    a.put(PropertyId::Default, Arg::opt_str(get_string(node, PropertyId::Default)));
    a.put(PropertyId::Pattern, Arg::opt_str(get_string(node, PropertyId::Pattern)));
    a.put(PropertyId::Discriminator, Arg::opt_str(get_string(node, PropertyId::Discriminator)));
    a.put(PropertyId::MinItems, Arg::opt_int(get_integer(node, PropertyId::MinItems)));
    a.put(PropertyId::MaxItems, Arg::opt_int(get_integer(node, PropertyId::MaxItems)));
    a.put(PropertyId::MinProperties, Arg::opt_int(get_integer(node, PropertyId::MinProperties)));
    a.put(PropertyId::MaxProperties, Arg::opt_int(get_integer(node, PropertyId::MaxProperties)));
    a.put(PropertyId::MinLength, Arg::opt_int(get_integer(node, PropertyId::MinLength)));
    a.put(PropertyId::MaxLength, Arg::opt_int(get_integer(node, PropertyId::MaxLength)));
    a.put(PropertyId::Minimum, Arg::opt_decimal(get_big_decimal(node, PropertyId::Minimum)?));
    a.put(PropertyId::Maximum, Arg::opt_decimal(get_big_decimal(node, PropertyId::Maximum)?));
    a.put(PropertyId::MultipleOf, Arg::opt_decimal(get_big_decimal(node, PropertyId::MultipleOf)?));
    a.put(PropertyId::ExclusiveMinimum, Arg::opt_bool(get_boolean(node, PropertyId::ExclusiveMinimum)));
    a.put(PropertyId::ExclusiveMaximum, Arg::opt_bool(get_boolean(node, PropertyId::ExclusiveMaximum)));
    a.put(PropertyId::UniqueItems, Arg::opt_bool(get_boolean(node, PropertyId::UniqueItems)));
    a.put(PropertyId::ReadOnly, Arg::opt_bool(get_boolean(node, PropertyId::ReadOnly)));
    a.put(PropertyId::VendorExtensions, Arg::Extensions(get_vendor_extensions(node)));
    Ok(a)
}

fn node_example(n: Option<&JsonNode>) -> Option<JavaObject> {
    n.map(|n| JavaObject::Node(n.clone()))
}

/// `PropertyDeserializer.propertyFromNode(node)` (`Json.mapper().convertValue(node, Property.class)`);
/// `Ok(None)` where Java returns null.
pub fn property_from_node(node: &JsonNode) -> JResult<Option<PropertyRef>> {
    if let JsonNode::Boolean(b) = node {
        return Ok(Some(Property::new_boolean_value(*b).into_ref()));
    }
    let ty = get_string(node, PropertyId::Type);
    let title = get_string(node, PropertyId::Title);
    let format = get_string(node, PropertyId::Format);
    let read_only = get_boolean(node, PropertyId::ReadOnly);
    let mut description = get_string(node, PropertyId::Description);
    let xml = get_xml(node)?;
    if let Some(r) = node.get("$ref") {
        let mut p = Property::new_ref(&r.as_text(), None);
        p.description = description;
        p.title = title;
        p.set_read_only(read_only);
        p.xml = xml;
        return Ok(Some(p.into_ref()));
    }
    if ty.as_deref() == Some("object") || node.get("properties").is_some() || node.get("allOf").is_some() {
        let example = detail(node, PropertyId::Example);
        let ap = node.get("additionalProperties");
        if ap.is_none_or(|a| !a.is_object()) {
            if let Some(JsonNode::Boolean(b)) = ap {
                let mut m = Property::new(PropertyKind::Map);
                m.additional_properties = Some(Property::new_boolean_value(*b).into_ref());
                m.description = description;
                m.title = title;
                m.xml = xml;
                m.set_example(node_example(example));
                m.min_properties = get_integer(node, PropertyId::MinProperties);
                m.max_properties = get_integer(node, PropertyId::MaxProperties);
                m.set_vendor_extension_map(&get_vendor_extensions(node));
                m.set_read_only(read_only);
                return Ok(Some(m.into_ref()));
            }
            let all_of = node.get("allOf");
            let props_node = node.get("properties");
            let mut detail_type: Option<&str> = None;
            let properties: JMap<PropertyRef> = JMap::with_kind(MapKind::Linked);
            if let Some(pn) = props_node {
                if let Some(o) = pn.as_object() {
                    for (k, v) in o.iter() {
                        match property_from_node(v)? {
                            Some(p) => {
                                properties.put(k.clone(), p);
                            }
                            None => {
                                if k == "type" && v.as_text() == "array" {
                                    detail_type = Some("array");
                                }
                                if k == "description" && v.is_textual() {
                                    description = Some(v.as_text());
                                }
                            }
                        }
                    }
                }
            } else if let Some(all_of) = all_of {
                let mut cp = Property::new(PropertyKind::Composed);
                cp.description = description.clone();
                cp.title = title;
                cp.set_example(node_example(example));
                cp.description = description;
                cp.vendor_extensions = get_vendor_extensions(node);
                let mut list = Vec::new();
                for inner in iter_elements(all_of) {
                    list.push(property_from_node(inner)?);
                }
                cp.all_of = Some(list);
                return Ok(Some(cp.into_ref()));
            }
            if detail_type == Some("array") {
                let mut ap = Property::new(PropertyKind::Array);
                ap.description = description.clone();
                ap.title = title;
                ap.xml = xml;
                ap.set_example(node_example(example));
                merge(&mut ap, &args_from_node(props_node.unwrap())?)?;
                ap.description = description;
                if properties.len() == 1 {
                    ap.items = properties.values().into_iter().next();
                }
                ap.set_vendor_extension_map(&get_vendor_extensions(node));
                return Ok(Some(ap.into_ref()));
            }
            let mut op = Property::new(PropertyKind::Object);
            op.properties = Some(properties);
            op.description = description;
            op.title = title;
            op.xml = xml;
            op.set_read_only(read_only);
            op.set_example(node_example(example));
            op.set_vendor_extension_map(&get_vendor_extensions(node));
            let required = get_required(node, PropertyId::Required)?;
            op.set_required_properties(&required);
            return Ok(Some(op.into_ref()));
        }
        if let Some(items) = property_from_node(ap.unwrap())? {
            let mut m = Property::new(PropertyKind::Map);
            m.additional_properties = Some(items);
            m.description = description.clone();
            m.title = title.clone();
            m.xml = xml.clone();
            m.set_example(node_example(example));
            m.min_properties = get_integer(node, PropertyId::MinProperties);
            m.max_properties = get_integer(node, PropertyId::MaxProperties);
            m.set_vendor_extension_map(&get_vendor_extensions(node));
            m.set_read_only(read_only);
            return Ok(Some(m.into_ref()));
        }
    }
    if ty.as_deref() == Some("array") && let Some(items_node) = node.get("items") {
        let sub = property_from_node(items_node)?;
        let mut ap = Property::new(PropertyKind::Array);
        ap.items = sub;
        ap.description = description;
        ap.title = title;
        ap.xml = xml;
        ap.set_read_only(read_only);
        ap.min_items = get_integer(node, PropertyId::MinItems);
        ap.max_items = get_integer(node, PropertyId::MaxItems);
        ap.set_unique_items(get_boolean(node, PropertyId::UniqueItems));
        ap.set_example(node_example(detail(node, PropertyId::Example)));
        ap.set_vendor_extension_map(&get_vendor_extensions(node));
        return Ok(Some(ap.into_ref()));
    }
    let args = args_from_node(node)?;
    match build(ty.as_deref(), format.as_deref(), &args)? {
        None => Ok(None),
        Some(mut p) => {
            p.xml = xml;
            p.description = description;
            Ok(Some(p.into_ref()))
        }
    }
}

fn iter_elements(n: &JsonNode) -> Vec<&JsonNode> {
    match n {
        JsonNode::Array(a) => a.iter().collect(),
        JsonNode::Object(o) => o.iter().map(|(_, v)| v).collect(),
        _ => Vec::new(),
    }
}

// ---------------------------------------------------------------------------------------------------------
// PropertyModelConverter

fn args_from_model(m: &Model) -> Args {
    let mut a = Args::new();
    a.put(PropertyId::Description, Arg::opt_str(m.get_description()));
    a.put(PropertyId::Example, Arg::opt_object(m.get_example()));
    a.put(PropertyId::Enum, Arg::opt_list(m.enum_.clone()));
    a.put(PropertyId::Title, Arg::opt_str(m.title.clone()));
    a.put(PropertyId::Default, Arg::opt_object(m.get_default_value()));
    a.put(PropertyId::Discriminator, Arg::opt_str(m.discriminator.clone()));
    a.put(PropertyId::UniqueItems, Arg::opt_bool(m.unique_items));
    a.put(PropertyId::VendorExtensions, Arg::Extensions(m.vendor_extensions.clone()));
    a.put(PropertyId::Pattern, Arg::opt_str(m.pattern.clone()));
    a.put(PropertyId::Maximum, Arg::opt_decimal(m.maximum.clone()));
    a.put(PropertyId::Minimum, Arg::opt_decimal(m.minimum.clone()));
    a.put(PropertyId::ExclusiveMaximum, Arg::opt_bool(m.exclusive_maximum));
    a.put(PropertyId::ExclusiveMinimum, Arg::opt_bool(m.exclusive_minimum));
    a.put(PropertyId::MultipleOf, Arg::opt_decimal(m.multiple_of.clone()));
    a.put(PropertyId::MinLength, Arg::opt_int(m.min_length));
    a.put(PropertyId::MaxLength, Arg::opt_int(m.max_length));
    a
}

/// `new PropertyModelConverter().modelToProperty(model)`: the property shares the model's property map
/// and items, as in Java.
pub fn model_to_property(model: &ModelRef) -> JResult<Option<PropertyRef>> {
    let m = model.borrow();
    match m.kind {
        ModelKind::Impl => {
            if m.additional_properties.is_none() {
                let Some(mut p) = build(m.type_.as_deref(), m.format.as_deref(), &args_from_model(&m))? else { return Ok(None) };
                if p.kind == PropertyKind::Object {
                    p.properties = m.properties.clone();
                    p.set_example(m.get_example());
                } else if p.kind.is_string_property() {
                    p.pattern = m.pattern.clone();
                    p.max_length = m.max_length;
                    p.min_length = m.min_length;
                } else if p.kind.is_numeric() {
                    p.maximum = m.maximum.clone();
                    p.minimum = m.minimum.clone();
                    p.multiple_of = m.multiple_of.clone();
                    p.exclusive_minimum = m.exclusive_minimum;
                    p.exclusive_maximum = m.exclusive_maximum;
                }
                Ok(Some(p.into_ref()))
            } else {
                let mut p = Property::new(PropertyKind::Map);
                p.type_ = m.type_.clone();
                p.set_allow_empty_value(m.allow_empty_value);
                // `setDefault((String) m.getDefaultValue())`
                match m.get_default_value() {
                    None | Some(JavaObject::String(_)) => {}
                    Some(o) => return Err(bootstrap_class_cast(java_class_of(&o), "java.lang.String")),
                }
                p.description = m.get_description();
                p.set_example(m.get_example());
                p.format = m.format.clone();
                p.name = m.name.clone();
                p.title = m.title.clone();
                if let (Some(req), Some(name)) = (m.get_required(), &m.name) && req.iter().any(|r| r == name) {
                    p.required = true;
                }
                p.xml = m.xml.clone();
                p.vendor_extensions = m.vendor_extensions.clone();
                p.additional_properties = m.additional_properties.clone();
                Ok(Some(p.into_ref()))
            }
        }
        ModelKind::Array => {
            let mut p = Property::new(PropertyKind::Array);
            p.items = m.items.clone();
            p.set_example(m.get_example());
            p.max_items = m.max_items;
            p.min_items = m.min_items;
            p.description = m.get_description();
            p.title = m.title.clone();
            p.set_unique_items(m.unique_items);
            p.xml = m.xml.clone();
            Ok(Some(p.into_ref()))
        }
        ModelKind::Ref => Ok(Some(Property::new_ref(&m.get_ref().unwrap_or_default(), m.ref_format()).into_ref())),
        ModelKind::BooleanValue => Ok(None),
        ModelKind::Composed => {
            let mut op = Property::new(PropertyKind::Object);
            op.description = m.get_description();
            op.title = m.title.clone();
            op.set_example(m.get_example());
            let props: JMap<PropertyRef> = JMap::with_kind(MapKind::Tree);
            let mut has_props = false;
            let required: JMap<()> = JMap::with_kind(MapKind::Hash);
            for item in m.all_of.clone() {
                let Some(ip) = model_to_property(&item)? else { continue };
                let ipb = ip.borrow();
                if ipb.kind == PropertyKind::Ref {
                    props.put(ipb.simple_ref().unwrap_or_default(), ip.clone());
                    has_props = true;
                } else if ipb.kind == PropertyKind::Object {
                    if let Some(pp) = &ipb.properties {
                        for (k, v) in pp.entries() {
                            props.put(k, v);
                            has_props = true;
                        }
                    }
                    if let Some(req) = ipb.required_properties() {
                        for r in req {
                            required.put(r, ());
                        }
                    }
                }
            }
            if has_props {
                op.properties = Some(props);
            }
            if !required.is_empty() {
                op.set_required_properties(&required.keys());
            }
            for (k, v) in m.vendor_extensions.entries() {
                op.set_vendor_extension(&k, v);
            }
            Ok(Some(op.into_ref()))
        }
    }
}
