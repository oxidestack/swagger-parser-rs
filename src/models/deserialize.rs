//! Jackson deserialization of the models (`mapper.convertValue(node, X.class)` / `readValue`), including the
//! custom deserializers swagger-core registers: `ModelDeserializer` / `Model31Deserializer` (Schema),
//! `ParameterDeserializer`, `SecuritySchemeDeserializer`, `ApiResponsesDeserializer`, `PathsDeserializer`,
//! `CallbackDeserializer` and the style-enum deserializers.

use super::*;
use crate::java::{JList, JMap, JSet};
use jackson::convert::node_to_object;
use jackson::{BigDecimal, JacksonError, JavaObject, JsonNode};

/// Types `convertValue` can produce.
pub trait FromTree: Sized {
    /// `mapper.convertValue(node, T.class)` with mapper `m`; `Err` when the tree does not fit the type.
    fn from_tree(node: &JsonNode, m: Mapper) -> Result<Self, JacksonError>;

    /// As `convertValue`, where the deserializer may return `null` (custom deserializers do).
    fn from_tree_opt(node: &JsonNode, m: Mapper) -> Result<Option<Self>, JacksonError> {
        Self::from_tree(node, m).map(Some)
    }
}

fn mismatched(target: &str, node: &JsonNode) -> JacksonError {
    let (what, token) = match node {
        JsonNode::Object(_) => ("Object value", "JsonToken.START_OBJECT"),
        JsonNode::Array(_) => ("Array value", "JsonToken.START_ARRAY"),
        JsonNode::Text(_) => ("String value", "JsonToken.VALUE_STRING"),
        JsonNode::Boolean(true) => ("Boolean value", "JsonToken.VALUE_TRUE"),
        JsonNode::Boolean(false) => ("Boolean value", "JsonToken.VALUE_FALSE"),
        n if n.is_integral_number() => ("Integer value", "JsonToken.VALUE_NUMBER_INT"),
        n if n.is_number() => ("Floating-point value", "JsonToken.VALUE_NUMBER_FLOAT"),
        _ => ("Embedded Object value", "JsonToken.VALUE_EMBEDDED_OBJECT"),
    };
    JacksonError {
        class: "com.fasterxml.jackson.databind.exc.MismatchedInputException".into(),
        message: format!("Cannot deserialize value of type `{target}` from {what} (token `{token}`)"),
    }
}

pub(crate) fn expect_object(node: &JsonNode, class: &str) -> Result<(), JacksonError> {
    if node.is_object() {
        return Ok(());
    }
    Err(mismatched(&format!("io.swagger.v3.oas.models.{class}"), node))
}

pub(crate) fn de_string(v: &JsonNode) -> Result<Option<String>, JacksonError> {
    match v {
        JsonNode::Null | JsonNode::Missing => Ok(None),
        JsonNode::Object(_) | JsonNode::Array(_) => Err(mismatched("java.lang.String", v)),
        _ => Ok(Some(v.as_text())),
    }
}

pub(crate) fn de_bool(v: &JsonNode) -> Result<Option<bool>, JacksonError> {
    match v {
        JsonNode::Null | JsonNode::Missing => Ok(None),
        JsonNode::Boolean(b) => Ok(Some(*b)),
        JsonNode::Text(s) => {
            let t = s.trim();
            if t == "true" || t == "True" || t == "TRUE" {
                Ok(Some(true))
            } else if t == "false" || t == "False" || t == "FALSE" {
                Ok(Some(false))
            } else if t.is_empty() {
                Ok(None)
            } else {
                Err(JacksonError {
                    class: "com.fasterxml.jackson.databind.exc.InvalidFormatException".into(),
                    message: format!("Cannot deserialize value of type `java.lang.Boolean` from String \"{s}\": only \"true\" or \"false\" recognized"),
                })
            }
        }
        n if n.is_integral_number() => Ok(Some(n.long_value() != 0)),
        _ => Err(mismatched("java.lang.Boolean", v)),
    }
}

pub(crate) fn de_int(v: &JsonNode) -> Result<Option<i32>, JacksonError> {
    match v {
        JsonNode::Null | JsonNode::Missing => Ok(None),
        n if n.is_number() => {
            if n.is_integral_number() && !n.can_convert_to_int() {
                return Err(JacksonError {
                    class: "com.fasterxml.jackson.core.exc.InputCoercionException".into(),
                    message: format!("Numeric value ({}) out of range of int (-2147483648 - 2147483647)", n.as_text()),
                });
            }
            Ok(Some(n.int_value()))
        }
        JsonNode::Text(s) => match s.trim().parse::<i32>() {
            Ok(i) => Ok(Some(i)),
            Err(_) if s.trim().is_empty() => Ok(None),
            Err(_) => Err(JacksonError {
                class: "com.fasterxml.jackson.databind.exc.InvalidFormatException".into(),
                message: format!("Cannot deserialize value of type `java.lang.Integer` from String \"{s}\": not a valid `java.lang.Integer` value"),
            }),
        },
        _ => Err(mismatched("java.lang.Integer", v)),
    }
}

pub(crate) fn de_big_decimal(v: &JsonNode) -> Result<Option<BigDecimal>, JacksonError> {
    match v {
        JsonNode::Null | JsonNode::Missing => Ok(None),
        n if n.is_number() => Ok(Some(n.decimal_value())),
        JsonNode::Text(s) => match BigDecimal::parse(s.trim()) {
            Ok(b) => Ok(Some(b)),
            Err(_) if s.trim().is_empty() => Ok(None),
            Err(_) => Err(JacksonError {
                class: "com.fasterxml.jackson.databind.exc.InvalidFormatException".into(),
                message: format!("Cannot deserialize value of type `java.math.BigDecimal` from String \"{s}\": not a valid representation"),
            }),
        },
        _ => Err(mismatched("java.math.BigDecimal", v)),
    }
}

/// Untyped (`Object`) property.
pub(crate) fn de_object(v: &JsonNode) -> Option<JavaObject> {
    match node_to_object(v) {
        JavaObject::Null => None,
        o => Some(o),
    }
}

pub(crate) fn de_enum<E>(v: &JsonNode, from: fn(&str) -> Option<E>, name: &str) -> Result<Option<E>, JacksonError> {
    match v {
        JsonNode::Null | JsonNode::Missing => Ok(None),
        _ => {
            let s = v.as_text();
            match from(&s) {
                Some(e) => Ok(Some(e)),
                None => Err(JacksonError {
                    class: "java.lang.RuntimeException".into(),
                    message: format!("Can not deserialize value of type {name} from String \"{s}\": value not one of declared Enum instance names"),
                }),
            }
        }
    }
}

pub(crate) fn de_list<T: Clone>(v: &JsonNode, mut f: impl FnMut(&JsonNode) -> Result<Option<T>, JacksonError>) -> Result<Option<JList<T>>, JacksonError> {
    match v {
        JsonNode::Null | JsonNode::Missing => Ok(None),
        JsonNode::Array(a) => {
            let l = JList::new();
            for e in a.iter() {
                if let Some(x) = f(e)? {
                    l.add(x);
                }
            }
            Ok(Some(l))
        }
        _ => Err(mismatched("java.util.ArrayList", v)),
    }
}

pub(crate) fn de_map<T: Clone>(v: &JsonNode, mut f: impl FnMut(&JsonNode) -> Result<Option<T>, JacksonError>) -> Result<Option<JMap<T>>, JacksonError> {
    match v {
        JsonNode::Null | JsonNode::Missing => Ok(None),
        JsonNode::Object(o) => {
            let m = JMap::new();
            for (k, e) in o.iter() {
                if let Some(x) = f(e)? {
                    m.put(k.clone(), x);
                }
            }
            Ok(Some(m))
        }
        _ => Err(mismatched("java.util.LinkedHashMap", v)),
    }
}

pub(crate) fn de_string_set(v: &JsonNode) -> Result<Option<JSet>, JacksonError> {
    Ok(de_list(v, de_string)?.map(|l| JSet::from_iter(crate::java::MapKind::Linked, l.to_vec())))
}

/// 3.1 `type` via the bean path (`setTypes(Set<String>)`)
pub(crate) fn de_types31(v: &JsonNode) -> Result<Option<JSet>, JacksonError> {
    match v {
        JsonNode::Text(s) => Ok(Some(JSet::from_iter(crate::java::MapKind::Linked, [s.to_string()]))),
        _ => de_string_set(v),
    }
}

pub(crate) fn de_model<T: FromTree>(v: &JsonNode, m: Mapper) -> Result<Option<T>, JacksonError> {
    match v {
        JsonNode::Null | JsonNode::Missing => Ok(None),
        _ => T::from_tree_opt(v, m),
    }
}

/// `setAdditionalProperties(Object)` through the bean path: Boolean or an error for anything else.
pub(crate) fn de_additional_properties(v: &JsonNode, _m: Mapper) -> Result<Option<AdditionalProperties>, JacksonError> {
    match v {
        JsonNode::Null | JsonNode::Missing => Ok(None),
        JsonNode::Boolean(b) => Ok(Some(AdditionalProperties::Bool(*b))),
        _ => Err(JacksonError {
            class: "com.fasterxml.jackson.databind.JsonMappingException".into(),
            message: "additionalProperties must be either a Boolean or a Schema instance".into(),
        }),
    }
}

impl FromTree for Schema {
    fn from_tree(node: &JsonNode, m: Mapper) -> Result<Self, JacksonError> {
        match m {
            Mapper::Json31 => model31_deserialize(node),
            _ => model_deserialize(node),
        }
    }

    fn from_tree_opt(node: &JsonNode, m: Mapper) -> Result<Option<Self>, JacksonError> {
        schema_from_tree_nullable(node, m)
    }
}

pub(crate) fn bean_schema(kind: SchemaKind, node: &JsonNode, m: Mapper) -> Result<Schema, JacksonError> {
    expect_object(node, &format!("media.{}", kind.simple_name()))?;
    let s = Schema::of(kind);
    s.deserialize_fields(node, m)?;
    Ok(s)
}

/// `Json31.jsonSchemaAsMap(node)`
fn json_schema_as_map(node: &JsonNode) -> Option<JMap<JavaObject>> {
    match node_to_object(node) {
        JavaObject::Map(m) => Some(JMap::from_entries(crate::java::MapKind::Linked, m.iter().map(|(k, v)| (k.clone(), v.clone())))),
        _ => None,
    }
}

/// `ModelDeserializer.deserialize` (OpenAPI 3.0)
pub fn model_deserialize(node: &JsonNode) -> Result<Schema, JacksonError> {
    if let JsonNode::Boolean(b) = node {
        let s = Schema::new();
        s.set_boolean_schema_value(*b);
        return Ok(s);
    }
    for field in ["allOf", "anyOf", "oneOf"] {
        if node.get(field).is_some() {
            return bean_schema(SchemaKind::Composed, node, Mapper::Json30);
        }
    }
    if let Some(type_node) = node.get("type") {
        return deserialize_schema_with_type(node, type_node).map(|o| o.unwrap_or_else(Schema::new_null_marker));
    }
    if let Some(r) = node.get("$ref") {
        let s = Schema::new();
        s.set_ref(r.as_text());
        return Ok(s);
    }
    deserialize_arbitrary_or_object_schema(node, false)
}

fn deserialize_arbitrary_or_object_schema(node: &JsonNode, _always_object: bool) -> Result<Schema, JacksonError> {
    let mut node = node.clone();
    let schema = match node.get("additionalProperties").cloned() {
        Some(JsonNode::Boolean(b)) => {
            node.as_object_mut().unwrap().shift_remove("additionalProperties");
            let s = bean_schema(if b { SchemaKind::Map } else { SchemaKind::Object }, &node, Mapper::Json30)?;
            s.set_additional_properties(AdditionalProperties::Bool(b));
            s
        }
        Some(ap) => {
            let inner = Schema::from_tree(&ap, Mapper::Json30)?;
            node.as_object_mut().unwrap().shift_remove("additionalProperties");
            let s = bean_schema(SchemaKind::Map, &node, Mapper::Json30)?;
            s.set_additional_properties(AdditionalProperties::Schema(inner));
            s
        }
        None => bean_schema(SchemaKind::Object, &node, Mapper::Json30)?,
    };
    schema.set_json_schema(json_schema_as_map(&node));
    Ok(schema)
}

fn deserialize_schema_with_type(node: &JsonNode, type_node: &JsonNode) -> Result<Option<Schema>, JacksonError> {
    let ty = match type_node.text_value() {
        Some(t) => t.to_string(),
        None => {
            return Err(JacksonError {
                class: "java.lang.ClassCastException".into(),
                message: format!(
                    "class {} cannot be cast to class com.fasterxml.jackson.databind.node.TextNode ({} and com.fasterxml.jackson.databind.node.TextNode are in unnamed module of loader 'app')",
                    crate::parser::deserializer::node_class_name(type_node),
                    crate::parser::deserializer::node_class_name(type_node)
                ),
            });
        }
    };
    let format = node.get("format").and_then(|f| f.text_value()).unwrap_or("").to_string();
    Ok(Some(match ty.as_str() {
        "array" => bean_schema(SchemaKind::Array, node, Mapper::Json30)?,
        "integer" => {
            let s = bean_schema(SchemaKind::Integer, node, Mapper::Json30)?;
            if format.trim().is_empty() {
                s.set_format(None::<String>);
            }
            s
        }
        "number" => bean_schema(SchemaKind::Number, node, Mapper::Json30)?,
        "boolean" => bean_schema(SchemaKind::Boolean, node, Mapper::Json30)?,
        "string" => {
            let kind = match format.as_str() {
                "date" => SchemaKind::Date,
                "date-time" => SchemaKind::DateTime,
                "email" => SchemaKind::Email,
                "password" => SchemaKind::Password,
                "uuid" => SchemaKind::Uuid,
                _ => SchemaKind::String,
            };
            bean_schema(kind, node, Mapper::Json30)?
        }
        "object" => deserialize_arbitrary_or_object_schema(node, true)?,
        _ => return Ok(None),
    }))
}

/// `Model31Deserializer` → `deserializeJsonSchema`
pub fn model31_deserialize(node: &JsonNode) -> Result<Schema, JacksonError> {
    if let JsonNode::Boolean(b) = node {
        let s = Schema::new();
        s.set_boolean_schema_value(*b);
        return Ok(s);
    }
    let ap = node.get("additionalProperties").cloned();
    let ty = node.get("type").cloned();
    if ty.is_none() && ap.is_none() {
        return bean_schema(SchemaKind::Json, node, Mapper::Json31);
    }
    let mut n = node.clone();
    if let Some(o) = n.as_object_mut() {
        o.shift_remove("type");
        o.shift_remove("additionalProperties");
    }
    let schema = bean_schema(SchemaKind::Json, &n, Mapper::Json31)?;
    match &ty {
        Some(JsonNode::Text(t)) => schema.set_types(JSet::from_iter(crate::java::MapKind::Linked, [t.to_string()])),
        Some(JsonNode::Array(a)) => {
            let set = JSet::new();
            for e in a.iter() {
                if let Some(t) = e.text_value() {
                    set.add(t);
                } else {
                    set.add("null");
                }
            }
            schema.set_types(set);
        }
        _ => {}
    }
    if let Some(ap) = ap {
        match ap {
            JsonNode::Boolean(b) => schema.set_additional_properties(AdditionalProperties::Bool(b)),
            other => match model31_deserialize(&other) {
                Ok(inner) => schema.set_additional_properties(AdditionalProperties::Schema(inner)),
                Err(_) => {
                    if let Some(b) = de_bool(&other)? {
                        schema.set_additional_properties(AdditionalProperties::Bool(b));
                    }
                }
            },
        }
    }
    Ok(schema)
}

impl Schema {
    /// Java's `ModelDeserializer` returns `null` for an unknown `type`; a null schema is not representable as
    /// a handle, so an empty `Schema` stands in (callers that need the distinction use
    /// [`schema_from_tree_nullable`]).
    pub(crate) fn new_null_marker() -> Schema {
        Schema::new()
    }
}

/// `Json.mapper().convertValue(node, Schema.class)` keeping Java's `null` for unknown types.
pub fn schema_from_tree_nullable(node: &JsonNode, m: Mapper) -> Result<Option<Schema>, JacksonError> {
    if m == Mapper::Json31 {
        return model31_deserialize(node).map(Some);
    }
    if let JsonNode::Boolean(_) = node {
        return model_deserialize(node).map(Some);
    }
    for field in ["allOf", "anyOf", "oneOf"] {
        if node.get(field).is_some() {
            return bean_schema(SchemaKind::Composed, node, Mapper::Json30).map(Some);
        }
    }
    if let Some(t) = node.get("type") {
        return deserialize_schema_with_type(node, t);
    }
    model_deserialize(node).map(Some)
}

impl FromTree for Parameter {
    /// `ParameterDeserializer`
    fn from_tree(node: &JsonNode, m: Mapper) -> Result<Self, JacksonError> {
        Parameter::from_tree_nullable(node, m).map(|p| p.unwrap_or_default())
    }

    fn from_tree_opt(node: &JsonNode, m: Mapper) -> Result<Option<Self>, JacksonError> {
        Parameter::from_tree_nullable(node, m)
    }
}

impl Parameter {
    /// `ParameterDeserializer` (null when neither `$ref` nor a known `in` is present)
    pub fn from_tree_nullable(node: &JsonNode, m: Mapper) -> Result<Option<Parameter>, JacksonError> {
        if let Some(sub) = node.get("$ref") {
            let p = Parameter::new();
            p.set_ref(sub.as_text());
            if m == Mapper::Json31 && let Some(d) = node.get("description") {
                p.set_description(d.as_text());
            }
            return Ok(Some(p));
        }
        let Some(in_node) = node.get("in") else { return Ok(None) };
        let kind = match in_node.as_text().as_str() {
            "query" => ParameterKind::Query,
            "header" => ParameterKind::Header,
            "path" => ParameterKind::Path,
            "cookie" => ParameterKind::Cookie,
            _ => return Ok(None),
        };
        let p = Parameter::of(kind);
        p.deserialize_fields(node, m)?;
        Ok(Some(p))
    }
}

impl FromTree for SecurityScheme {
    fn from_tree(node: &JsonNode, m: Mapper) -> Result<Self, JacksonError> {
        Ok(SecurityScheme::from_tree_nullable(node, m)?.unwrap_or_else(SecurityScheme::new))
    }

    fn from_tree_opt(node: &JsonNode, m: Mapper) -> Result<Option<Self>, JacksonError> {
        SecurityScheme::from_tree_nullable(node, m)
    }
}

impl SecurityScheme {
    /// `SecuritySchemeDeserializer`
    pub fn from_tree_nullable(node: &JsonNode, m: Mapper) -> Result<Option<SecurityScheme>, JacksonError> {
        let Some(t) = node.get("type") else { return Ok(None) };
        let ty = t.as_text();
        let Some(kind) = SecuritySchemeType::from_str_value(&ty) else {
            return Err(JacksonError { class: "com.fasterxml.jackson.core.JsonParseException".into(), message: format!("SecurityScheme type {ty} not allowed") });
        };
        let text = |f: &str| node.get(f).map(|n| n.as_text());
        let r = SecurityScheme::new();
        r.set_description(text("description"));
        r.set_type(kind);
        match kind {
            SecuritySchemeType::Http => {
                r.set_scheme(text("scheme"));
                r.set_bearer_format(text("bearerFormat"));
            }
            SecuritySchemeType::ApiKey => {
                r.set_name(text("name"));
                r.set_in(text("in").as_deref().and_then(SecuritySchemeIn::from_str_value));
            }
            SecuritySchemeType::OpenIdConnect => r.set_open_id_connect_url(text("openIdConnectUrl")),
            SecuritySchemeType::Oauth2 => r.set_flows(match node.get("flows") {
                Some(f) => de_model::<OAuthFlows>(f, m)?,
                None => None,
            }),
            SecuritySchemeType::MutualTls => {}
        }
        for (k, v) in node.as_object().into_iter().flat_map(|o| o.iter()) {
            if k.starts_with("x-") {
                r.add_extension(k, node_to_object(v));
            }
        }
        Ok(Some(r))
    }
}

macro_rules! map_from_tree {
    ($name:ident, $vt:ty, $conv:expr, ext) => {
        impl FromTree for $name {
            /// `PathsDeserializer` / `ApiResponsesDeserializer`: `x-` keys become extensions
            fn from_tree(node: &JsonNode, m: Mapper) -> Result<Self, JacksonError> {
                let obj = match node.as_object() {
                    Some(o) => o,
                    None => return Err(mismatched(concat!("io.swagger.v3.oas.models.", stringify!($name)), node)),
                };
                let result = $name::new();
                let ext: JMap<JavaObject> = JMap::new();
                for (k, v) in obj.iter() {
                    if k.starts_with("x-") {
                        ext.put(k.clone(), node_to_object(v));
                    } else {
                        let conv: fn(&JsonNode, Mapper) -> Result<Option<$vt>, JacksonError> = $conv;
                        if let Some(x) = conv(v, m)? {
                            result.put(k.clone(), x);
                        }
                    }
                }
                if !ext.is_empty() {
                    result.0.write().extensions = Some(ext);
                }
                Ok(result)
            }
        }
    };
    ($name:ident, $vt:ty, $conv:expr) => {
        impl FromTree for $name {
            /// plain `MapDeserializer`
            fn from_tree(node: &JsonNode, m: Mapper) -> Result<Self, JacksonError> {
                let obj = match node.as_object() {
                    Some(o) => o,
                    None => return Err(mismatched(concat!("io.swagger.v3.oas.models.", stringify!($name)), node)),
                };
                let result = $name::new();
                for (k, v) in obj.iter() {
                    let conv: fn(&JsonNode, Mapper) -> Result<Option<$vt>, JacksonError> = $conv;
                    if let Some(x) = conv(v, m)? {
                        result.put(k.clone(), x);
                    }
                }
                Ok(result)
            }
        }
    };
}

map_from_tree!(Paths, PathItem, |v, m| de_model::<PathItem>(v, m), ext);
map_from_tree!(ApiResponses, ApiResponse, |v, m| de_model::<ApiResponse>(v, m), ext);
map_from_tree!(Content, MediaType, |v, m| de_model::<MediaType>(v, m));
map_from_tree!(Scopes, String, |v, _m| de_string(v));
map_from_tree!(ServerVariables, ServerVariable, |v, m| de_model::<ServerVariable>(v, m));
map_from_tree!(SecurityRequirement, JList<String>, |v, _m| de_list(v, de_string));

impl FromTree for Callback {
    /// `CallbackDeserializer` (note `$ref` goes through `$ref()`, which prefixes bare names)
    fn from_tree(node: &JsonNode, m: Mapper) -> Result<Self, JacksonError> {
        let obj = match node.as_object() {
            Some(o) => o,
            None => return Err(mismatched("io.swagger.v3.oas.models.callbacks.Callback", node)),
        };
        let result = Callback::new();
        let ext: JMap<JavaObject> = JMap::new();
        for (k, v) in obj.iter() {
            if k.starts_with("x-") {
                ext.put(k.clone(), node_to_object(v));
            } else if k == "$ref" {
                result.set_ref(v.as_text());
            } else if let Some(p) = de_model::<PathItem>(v, m)? {
                result.put(k.clone(), p);
            }
        }
        if !ext.is_empty() {
            result.0.write().extensions = Some(ext);
        }
        Ok(result)
    }
}

/// `List<T>` elements of `enum` / `examples`, typed by the schema subclass's `T`.
pub(crate) fn de_schema_typed(e: &JsonNode, kind: SchemaKind) -> Result<Option<JavaObject>, JacksonError> {
    if e.is_null() {
        // null elements are kept by Jackson; represented as JavaObject::Null
        return Ok(Some(JavaObject::Null));
    }
    match kind {
        SchemaKind::Number => de_big_decimal(e).map(|o| o.map(|b| JavaObject::BigDecimal(b.into()))),
        SchemaKind::Integer => match e {
            n if n.is_number() => Ok(n.number_value().map(|v| match v {
                JavaObject::Short(s) => JavaObject::Integer(s as i32),
                JavaObject::Float(f) => JavaObject::Double(f as f64),
                other => other,
            })),
            JsonNode::Text(s) => {
                match jackson::javanum::parse_double(s) {
                    Some(_) if s.contains(['.', 'e', 'E']) => Ok(Some(JavaObject::Double(jackson::javanum::parse_double(s).unwrap()))),
                    _ => match s.trim().parse::<i64>() {
                        Ok(l) if l >= i32::MIN as i64 && l <= i32::MAX as i64 => Ok(Some(JavaObject::Integer(l as i32))),
                        Ok(l) => Ok(Some(JavaObject::Long(l))),
                        Err(_) => Err(JacksonError {
                            class: "com.fasterxml.jackson.databind.exc.InvalidFormatException".into(),
                            message: format!("Cannot deserialize value of type `java.lang.Number` from String \"{s}\": not a valid number"),
                        }),
                    },
                }
            }
            JsonNode::Boolean(_) => Err(mismatched("java.lang.Number", e)),
            _ => Err(mismatched("java.lang.Number", e)),
        },
        SchemaKind::String | SchemaKind::Email | SchemaKind::Password | SchemaKind::File => de_string(e).map(|o| o.map(JavaObject::string)),
        SchemaKind::Boolean => de_bool(e).map(|o| o.map(JavaObject::Boolean)),
        SchemaKind::Uuid => match de_string(e)? {
            None => Ok(None),
            Some(s) => match jackson::convert::uuid_from_string(&s) {
                Some(u) => Ok(Some(JavaObject::Uuid(u))),
                None => Err(JacksonError {
                    class: "com.fasterxml.jackson.databind.exc.InvalidFormatException".into(),
                    message: format!("Cannot deserialize value of type `java.util.UUID` from String \"{s}\": UUID has to be represented by standard 36-char representation"),
                }),
            },
        },
        SchemaKind::Date => match e {
            n if n.is_integral_number() => Ok(Some(JavaObject::Date(n.long_value()))),
            JsonNode::Text(s) => match crate::models::schema::parse_date_utc(s.get(..10).unwrap_or(s)) {
                Some(ms) => Ok(Some(JavaObject::Date(ms))),
                None => Err(JacksonError {
                    class: "com.fasterxml.jackson.databind.exc.InvalidFormatException".into(),
                    message: format!("Cannot deserialize value of type `java.util.Date` from String \"{s}\""),
                }),
            },
            _ => Err(mismatched("java.util.Date", e)),
        },
        SchemaKind::DateTime => match e {
            JsonNode::Text(s) => match crate::models::schema::parse_offset_date_time(s) {
                Some(o) => Ok(Some(JavaObject::OffsetDateTime(o))),
                None => Err(JacksonError {
                    class: "com.fasterxml.jackson.databind.exc.InvalidFormatException".into(),
                    message: format!("Cannot deserialize value of type `java.time.OffsetDateTime` from String \"{s}\""),
                }),
            },
            _ => Err(mismatched("java.time.OffsetDateTime", e)),
        },
        SchemaKind::ByteArray | SchemaKind::Binary => match e {
            JsonNode::Text(s) => match jackson::base64::decode_mime(s) {
                Ok(b) => Ok(Some(JavaObject::Bytes(b.into()))),
                Err(m) => Err(JacksonError { class: "com.fasterxml.jackson.databind.exc.InvalidFormatException".into(), message: m }),
            },
            _ => Err(mismatched("byte[]", e)),
        },
        _ => Ok(de_object(e).or(Some(JavaObject::Null))),
    }
}
