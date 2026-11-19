//! swagger-core 1.x `Json.mapper().convertValue(node, X.class)` into the Swagger 2.0 classes: Jackson bean
//! deserialization (setters in document order, `x-` fields through `@JsonAnySetter`, unknown fields ignored)
//! plus the custom `ModelDeserializer`, `ParameterDeserializer`, `ResponseDeserializer`, `PathDeserializer`
//! and `SecurityDefinitionDeserializer`.

use super::models::*;
use super::property_builder::property_from_node;
use crate::java::{JResult, JavaException};
use crate::models::deserialize::{de_big_decimal, de_bool, de_int, de_object, de_string};
use indexmap::IndexMap;
use jackson::convert::node_to_object;
use jackson::{JacksonError, JavaObject, JsonNode};

fn wrap(e: JacksonError) -> JavaException {
    JavaException::new("java.lang.IllegalArgumentException", e.message)
}

fn s(v: &JsonNode) -> JResult<Option<String>> {
    de_string(v).map_err(wrap)
}

fn b(v: &JsonNode) -> JResult<Option<bool>> {
    de_bool(v).map_err(wrap)
}

fn i(v: &JsonNode) -> JResult<Option<i32>> {
    de_int(v).map_err(wrap)
}

fn mismatch(target: &str, v: &JsonNode) -> JavaException {
    JavaException::new("java.lang.IllegalArgumentException", format!("Cannot deserialize value of type `{target}` from {} value", v.node_class_name()))
}

fn expect_object(v: &JsonNode, class: &str) -> JResult<()> {
    if v.is_object() { Ok(()) } else { Err(mismatch(class, v)) }
}

fn fields(v: &JsonNode) -> Vec<(String, JsonNode)> {
    v.as_object().map(|o| o.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default()
}

/// `List<String>`
fn strings(v: &JsonNode) -> JResult<Option<Vec<String>>> {
    match v {
        JsonNode::Null | JsonNode::Missing => Ok(None),
        JsonNode::Array(a) => {
            let mut out = Vec::new();
            for e in a.iter() {
                match e {
                    JsonNode::Null => {}
                    JsonNode::Object(_) | JsonNode::Array(_) => return Err(mismatch("java.lang.String", e)),
                    _ => out.push(e.as_text()),
                }
            }
            Ok(Some(out))
        }
        _ => Err(mismatch("java.util.ArrayList<java.lang.String>", v)),
    }
}

/// `Map<String, Object>` (a `LinkedHashMap` of untyped values)
fn object_map(v: &JsonNode) -> JResult<Option<IndexMap<String, JavaObject>>> {
    match v {
        JsonNode::Null | JsonNode::Missing => Ok(None),
        JsonNode::Object(o) => Ok(Some(o.iter().map(|(k, v)| (k.clone(), node_to_object(v))).collect())),
        _ => Err(mismatch("java.util.LinkedHashMap<java.lang.String,java.lang.Object>", v)),
    }
}

/// `Map<String, String>`
fn string_map(v: &JsonNode) -> JResult<Option<IndexMap<String, Option<String>>>> {
    match v {
        JsonNode::Null | JsonNode::Missing => Ok(None),
        JsonNode::Object(o) => {
            let mut m = IndexMap::new();
            for (k, e) in o.iter() {
                m.insert(k.clone(), s(e)?);
            }
            Ok(Some(m))
        }
        _ => Err(mismatch("java.util.LinkedHashMap<java.lang.String,java.lang.String>", v)),
    }
}

/// A `Property`-typed field (swagger-core's `PropertyDeserializer`)
fn property(v: &JsonNode) -> JResult<Option<PropertyRef>> {
    match v {
        JsonNode::Null | JsonNode::Missing => Ok(None),
        _ => property_from_node(v),
    }
}

/// `Map<String, Property>`
pub fn property_map(v: &JsonNode) -> JResult<Option<IndexMap<String, Option<PropertyRef>>>> {
    match v {
        JsonNode::Null | JsonNode::Missing => Ok(None),
        JsonNode::Object(o) => {
            let mut m = IndexMap::new();
            for (k, e) in o.iter() {
                m.insert(k.clone(), property(e)?);
            }
            Ok(Some(m))
        }
        _ => Err(mismatch("java.util.LinkedHashMap<java.lang.String,io.swagger.models.properties.Property>", v)),
    }
}

/// `Json.mapper().convertValue(node, Xml.class)`
pub fn xml(v: &JsonNode) -> JResult<Option<Xml>> {
    if v.is_null() {
        return Ok(None);
    }
    expect_object(v, "io.swagger.models.Xml")?;
    let mut x = Xml::default();
    for (k, f) in fields(v) {
        match k.as_str() {
            "name" => x.name = s(&f)?,
            "namespace" => x.namespace = s(&f)?,
            "prefix" => x.prefix = s(&f)?,
            "attribute" => x.attribute = b(&f)?,
            "wrapped" => x.wrapped = b(&f)?,
            _ => {}
        }
    }
    Ok(Some(x))
}

/// `ExternalDocs` bean
pub fn external_docs(v: &JsonNode) -> JResult<Option<ExternalDocs>> {
    if v.is_null() {
        return Ok(None);
    }
    expect_object(v, "io.swagger.models.ExternalDocs")?;
    let mut d = ExternalDocs::default();
    for (k, f) in fields(v) {
        match k.as_str() {
            "description" => d.description = s(&f)?,
            "url" => d.url = s(&f)?,
            _ if k.starts_with("x-") => {
                d.vendor_extensions.put(k, node_to_object(&f));
            }
            _ => {}
        }
    }
    Ok(Some(d))
}

/// `BasicAuthDefinition` / `OAuth2Definition` beans (`SecurityDefinitionDeserializer` picks by `type`)
pub fn security_definition(kind: SecuritySchemeKind, v: &JsonNode) -> JResult<SecuritySchemeDefinition> {
    expect_object(v, "io.swagger.models.auth.SecuritySchemeDefinition")?;
    let mut d = SecuritySchemeDefinition::new(kind);
    for (k, f) in fields(v) {
        match (kind, k.as_str()) {
            (_, "type") => d.type_ = s(&f)?,
            (_, "description") => d.description = s(&f)?,
            (SecuritySchemeKind::OAuth2, "authorizationUrl") => d.authorization_url = s(&f)?,
            (SecuritySchemeKind::OAuth2, "tokenUrl") => d.token_url = s(&f)?,
            (SecuritySchemeKind::OAuth2, "flow") => d.flow = s(&f)?,
            (SecuritySchemeKind::OAuth2, "scopes") => d.scopes = string_map(&f)?,
            (SecuritySchemeKind::ApiKey, "name") => d.name = s(&f)?,
            (SecuritySchemeKind::ApiKey, "in") => d.in_ = s(&f)?.and_then(|x| In::for_value(&x)),
            _ if k.starts_with("x-") => {
                d.vendor_extensions.put(k, node_to_object(&f));
            }
            _ => {}
        }
    }
    Ok(d)
}

/// `SecurityDefinitionDeserializer`
pub fn security_scheme_definition(v: &JsonNode) -> JResult<Option<SecuritySchemeDefinition>> {
    let Some(t) = v.get("type") else { return Ok(None) };
    let kind = match t.as_text().as_str() {
        "basic" => SecuritySchemeKind::Basic,
        "apiKey" => SecuritySchemeKind::ApiKey,
        "oauth2" => SecuritySchemeKind::OAuth2,
        _ => return Ok(None),
    };
    security_definition(kind, v).map(Some)
}

/// The bean fields shared by `AbstractModel` subclasses; returns false for fields the class does not have.
fn abstract_model_field(m: &mut Model, k: &str, f: &JsonNode) -> JResult<bool> {
    match k {
        "externalDocs" => m.external_docs = external_docs(f)?,
        "title" => m.title = s(f)?,
        "xml" => m.xml = xml(f)?,
        "minimum" => m.minimum = de_big_decimal(f).map_err(wrap)?,
        "maximum" => m.maximum = de_big_decimal(f).map_err(wrap)?,
        "multipleOf" => m.multiple_of = de_big_decimal(f).map_err(wrap)?,
        "exclusiveMinimum" => m.exclusive_minimum = b(f)?,
        "exclusiveMaximum" => m.exclusive_maximum = b(f)?,
        "minLength" => m.min_length = i(f)?,
        "maxLength" => m.max_length = i(f)?,
        "pattern" => m.pattern = s(f)?,
        "reference" => m.reference = s(f)?,
        "properties" => {
            let props = property_map(f)?;
            if let Some(p) = props {
                for (k, v) in p {
                    m.add_property(&k, v);
                }
            }
        }
        "required" => {
            let r = strings(f)?;
            m.set_required(r);
        }
        _ if k.starts_with("x-") => {
            m.vendor_extensions.put(k, node_to_object(f));
        }
        _ => return Ok(false),
    }
    Ok(true)
}

/// `ModelDeserializer` (`Json.mapper().convertValue(node, Model.class)`)
pub fn model(v: &JsonNode) -> JResult<Option<ModelRef>> {
    match v {
        JsonNode::Null | JsonNode::Missing => return Ok(None),
        JsonNode::Boolean(x) => return Ok(Some(Model::new_boolean_value(*x).into_ref())),
        _ => {}
    }
    if let Some(r) = v.get("$ref") {
        // convertValue(sub, RefModel.class): the RefModel(String) constructor
        return match r {
            JsonNode::Null => Ok(None),
            JsonNode::Object(_) | JsonNode::Array(_) => Err(mismatch("io.swagger.models.RefModel", r)),
            _ => Ok(Some(Model::new_ref(&r.as_text(), None).into_ref())),
        };
    }
    if v.get("allOf").is_some() {
        expect_object(v, "io.swagger.models.ComposedModel")?;
        let mut m = Model::new(ModelKind::Composed);
        for (k, f) in fields(v) {
            match k.as_str() {
                "allOf" => {
                    let mut list = Vec::new();
                    match &f {
                        JsonNode::Null => {}
                        JsonNode::Array(a) => {
                            for e in a.iter() {
                                if let Some(x) = model(e)? {
                                    list.push(x);
                                }
                            }
                        }
                        other => return Err(mismatch("java.util.ArrayList<io.swagger.models.Model>", other)),
                    }
                    m.all_of = list;
                }
                "description" => m.description = s(&f)?,
                "example" => m.example = de_object(&f),
                "parent" | "child" | "interfaces" => {}
                _ => {
                    abstract_model_field(&mut m, &k, &f)?;
                }
            }
        }
        let all = m.all_of.clone();
        if !all.is_empty() {
            m.parent = Some(all[0].clone());
            if all.len() >= 2 {
                m.child = Some(all[all.len() - 1].clone());
                let interfaces: Vec<ModelRef> = all[1..all.len() - 1].iter().filter(|x| x.borrow().kind == ModelKind::Ref).cloned().collect();
                m.interfaces = Some(interfaces);
            } else {
                let c = Model::new(ModelKind::Impl).into_ref();
                m.child = Some(c.clone());
                m.all_of.push(c);
            }
        }
        return Ok(Some(m.into_ref()));
    }
    let is_array = v.get("type").is_some_and(|t| t.text_value() == Some("array")) || v.get("items").is_some();
    if let Some(t) = v.get("type") && !t.is_textual() && v.get("items").is_none() {
        return Err(JavaException::class_cast(&format!("com.fasterxml.jackson.databind.node.{}", t.node_class_name()), "com.fasterxml.jackson.databind.node.TextNode"));
    }
    if is_array {
        expect_object(v, "io.swagger.models.ArrayModel")?;
        let mut m = Model::new(ModelKind::Array);
        for (k, f) in fields(v) {
            match k.as_str() {
                "type" => m.type_ = s(&f)?,
                "description" => m.description = s(&f)?,
                "items" => m.items = property(&f)?,
                "uniqueItems" => m.unique_items = b(&f)?,
                "example" => m.example = de_object(&f),
                "minItems" => m.min_items = i(&f)?,
                "maxItems" => m.max_items = i(&f)?,
                _ => {
                    abstract_model_field(&mut m, &k, &f)?;
                }
            }
        }
        return Ok(Some(m.into_ref()));
    }
    expect_object(v, "io.swagger.models.ModelImpl")?;
    let mut m = Model::new(ModelKind::Impl);
    for (k, f) in fields(v) {
        match k.as_str() {
            "type" => m.type_ = s(&f)?,
            "format" => m.format = s(&f)?,
            "allowEmptyValue" => {
                if let Some(x) = b(&f)? {
                    m.allow_empty_value = Some(x);
                }
            }
            "uniqueItems" => m.unique_items = b(&f)?,
            "description" => m.description = s(&f)?,
            "example" => m.example = de_object(&f),
            "additionalProperties" => {
                let p = property(&f)?;
                m.set_additional_properties(p);
            }
            "discriminator" => m.discriminator = s(&f)?,
            "default" => m.default_value = s(&f)?,
            "enum" => m.enum_ = strings(&f)?,
            _ => {
                abstract_model_field(&mut m, &k, &f)?;
            }
        }
    }
    Ok(Some(m.into_ref()))
}

/// The `AbstractParameter` bean fields; returns false for unknown fields.
fn abstract_parameter_field(p: &mut Parameter, k: &str, f: &JsonNode) -> JResult<bool> {
    match k {
        "in" => p.in_ = s(f)?,
        "name" => p.name = s(f)?,
        "description" => p.description = s(f)?,
        "required" => p.required = b(f)?.unwrap_or(false),
        "pattern" => p.pattern = s(f)?,
        "readOnly" => p.read_only = b(f)?,
        _ => return Ok(false),
    }
    Ok(true)
}

/// `ParameterDeserializer` (`Json.mapper().convertValue(node, Parameter.class)`)
pub fn parameter(v: &JsonNode) -> JResult<Option<ParameterRef>> {
    if v.is_null() {
        return Ok(None);
    }
    if let Some(r) = v.get("$ref") {
        return match r {
            JsonNode::Null => Ok(None),
            JsonNode::Object(_) | JsonNode::Array(_) => Err(mismatch("io.swagger.models.parameters.RefParameter", r)),
            _ => Ok(Some(Parameter::new_ref(&r.as_text()).into_ref())),
        };
    }
    let Some(in_node) = v.get("in") else { return Ok(None) };
    let kind = match in_node.as_text().as_str() {
        "query" => ParameterKind::Query,
        "header" => ParameterKind::Header,
        "path" => ParameterKind::Path,
        "formData" => ParameterKind::Form,
        "body" => ParameterKind::Body,
        "cookie" => ParameterKind::Cookie,
        _ => return Ok(None),
    };
    expect_object(v, "io.swagger.models.parameters.Parameter")?;
    let mut p = Parameter::new(kind);
    for (k, f) in fields(v) {
        if abstract_parameter_field(&mut p, &k, &f)? {
            continue;
        }
        if kind == ParameterKind::Body {
            match k.as_str() {
                "schema" => p.schema = model(&f)?,
                "x-examples" => {
                    p.examples = string_map(&f)?.map(|m| m.into_iter().map(|(k, v)| (k, v.map(JavaObject::string).unwrap_or(JavaObject::Null))).collect());
                }
                _ if k.starts_with("x-") => p.set_vendor_extension(&k, node_to_object(&f)),
                _ => {}
            }
            continue;
        }
        match k.as_str() {
            "type" => {
                let t = s(&f)?;
                p.set_type(t);
            }
            "format" => p.format = s(&f)?,
            "collectionFormat" => p.collection_format = s(&f)?,
            "items" => p.items = property(&f)?,
            "exclusiveMaximum" => p.exclusive_maximum = b(&f)?,
            "maximum" => p.maximum = de_big_decimal(&f).map_err(wrap)?,
            "exclusiveMinimum" => p.exclusive_minimum = b(&f)?,
            "minimum" => p.minimum = de_big_decimal(&f).map_err(wrap)?,
            "x-example" => p.example = s(&f)?,
            "maxItems" => p.max_items = i(&f)?,
            "minItems" => p.min_items = i(&f)?,
            "allowEmptyValue" => p.allow_empty_value = b(&f)?,
            "enum" => {
                // setEnumValue(List<?>): toString() of each item
                p.enum_ = match &f {
                    JsonNode::Null => None,
                    JsonNode::Array(a) => Some(a.iter().map(|e| node_to_object(e).to_java_string()).collect()),
                    other => return Err(mismatch("java.util.ArrayList<java.lang.Object>", other)),
                };
            }
            "maxLength" => p.max_length = i(&f)?,
            "minLength" => p.min_length = i(&f)?,
            "uniqueItems" => p.unique_items = b(&f)?,
            "multipleOf" => {
                p.multiple_of = match &f {
                    JsonNode::Null => None,
                    n if n.is_number() => Some(node_to_object(n)),
                    JsonNode::Text(t) => Some(jackson::javanum::parse_double(t).map(JavaObject::Double).ok_or_else(|| mismatch("java.lang.Number", &f))?),
                    other => return Err(mismatch("java.lang.Number", other)),
                };
            }
            "default" => p.default_value = de_object(&f).map(|o| o.to_java_string()),
            _ if k.starts_with("x-") => p.set_vendor_extension(&k, node_to_object(&f)),
            _ => {}
        }
    }
    Ok(Some(p.into_ref()))
}

/// `ResponseDeserializer` (`Json.mapper().convertValue(node, Response.class)`)
pub fn response(v: &JsonNode) -> JResult<Option<ResponseRef>> {
    if v.is_null() {
        return Ok(None);
    }
    expect_object(v, "io.swagger.models.Response")?;
    let mut r = Response::new();
    if v.get("$ref").is_some() {
        r.is_ref = true;
    }
    for (k, f) in fields(v) {
        match k.as_str() {
            "description" => r.description = s(&f)?,
            "schema" => r.schema = model(&f)?,
            "examples" => r.examples = object_map(&f)?,
            "headers" => r.headers = property_map(&f)?,
            "$ref" if r.is_ref => {
                let x = s(&f)?;
                r.generic_ref = x.map(|x| GenericRef::new(RefType::Response, &x, None));
            }
            _ if k.starts_with("x-") => r.set_vendor_extension(&k, node_to_object(&f)),
            _ => {}
        }
    }
    Ok(Some(r.into_ref()))
}

/// `Operation` bean
pub fn operation(v: &JsonNode) -> JResult<Option<OperationRef>> {
    if v.is_null() {
        return Ok(None);
    }
    expect_object(v, "io.swagger.models.Operation")?;
    let mut o = Operation::new();
    for (k, f) in fields(v) {
        match k.as_str() {
            "tags" => o.tags = strings(&f)?,
            "summary" => o.summary = s(&f)?,
            "description" => o.description = s(&f)?,
            "operationId" => o.operation_id = s(&f)?,
            "schemes" => o.schemes = strings(&f)?.map(|l| l.iter().map(|x| Scheme::for_value(x)).collect()),
            "consumes" => o.consumes = strings(&f)?,
            "produces" => o.produces = strings(&f)?,
            "parameters" => {
                o.parameters = match &f {
                    JsonNode::Null => None,
                    JsonNode::Array(a) => {
                        let mut l = Vec::new();
                        for e in a.iter() {
                            if let Some(p) = parameter(e)? {
                                l.push(p);
                            }
                        }
                        Some(l)
                    }
                    other => return Err(mismatch("java.util.ArrayList<io.swagger.models.parameters.Parameter>", other)),
                };
            }
            "responses" => {
                o.responses = match &f {
                    JsonNode::Null => None,
                    JsonNode::Object(m) => {
                        let mut out = Responses::new();
                        for (code, rv) in m.iter() {
                            out.insert(code.clone(), response(rv)?);
                        }
                        Some(out)
                    }
                    other => return Err(mismatch("io.swagger.models.Responses", other)),
                };
            }
            "security" => {
                o.security = match &f {
                    JsonNode::Null => None,
                    JsonNode::Array(a) => {
                        let mut l = Vec::new();
                        for e in a.iter() {
                            let mut req = IndexMap::new();
                            for (name, scopes) in fields(e) {
                                req.insert(name, strings(&scopes)?.unwrap_or_default());
                            }
                            l.push(req);
                        }
                        Some(l)
                    }
                    other => return Err(mismatch("java.util.ArrayList", other)),
                };
            }
            "externalDocs" => o.external_docs = external_docs(&f)?,
            "deprecated" => o.deprecated = b(&f)?,
            _ if k.starts_with("x-") => o.set_vendor_extension(&k, node_to_object(&f)),
            _ => {}
        }
    }
    Ok(Some(o.into_ref()))
}

/// `PathDeserializer` (`Json.mapper().convertValue(node, Path.class)`)
pub fn path(v: &JsonNode) -> JResult<Option<PathRef>> {
    if v.is_null() {
        return Ok(None);
    }
    expect_object(v, "io.swagger.models.Path")?;
    let mut p = if v.get("$ref").is_some() { Path::new_ref(None) } else { Path::new() };
    for (k, f) in fields(v) {
        match k.as_str() {
            "get" => p.get = operation(&f)?,
            "put" => p.put = operation(&f)?,
            "post" => p.post = operation(&f)?,
            "head" => p.head = operation(&f)?,
            "delete" => p.delete = operation(&f)?,
            "patch" => p.patch = operation(&f)?,
            "options" => p.options = operation(&f)?,
            "parameters" => {
                p.parameters = match &f {
                    JsonNode::Null => None,
                    JsonNode::Array(a) => {
                        let mut l = Vec::new();
                        for e in a.iter() {
                            if let Some(x) = parameter(e)? {
                                l.push(x);
                            }
                        }
                        Some(l)
                    }
                    other => return Err(mismatch("java.util.ArrayList<io.swagger.models.parameters.Parameter>", other)),
                };
            }
            "$ref" if p.is_ref => {
                let x = s(&f)?;
                p.generic_ref = x.map(|x| GenericRef::new(RefType::Path, &x, None));
            }
            _ if k.starts_with("x-") => p.set_vendor_extension(&k, node_to_object(&f)),
            _ => {}
        }
    }
    Ok(Some(p.into_ref()))
}

/// The types `DeserializationUtils.deserialize` / `ResolverCache.loadRef` produce.
#[derive(Clone, Debug)]
pub enum Loaded {
    /// A schema, as `Model`.
    Model(ModelRef),
    /// A parameter, as `Parameter`.
    Parameter(ParameterRef),
    /// A response, as `Response`.
    Response(ResponseRef),
    /// A path item, as `Path`.
    Path(PathRef),
    /// A property, as `Property`.
    Property(PropertyRef),
}

/// The `Class<T>` argument of `loadRef` / `deserialize`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadType {
    /// `Model.class`.
    Model,
    /// `Parameter.class`.
    Parameter,
    /// `Response.class`.
    Response,
    /// `Path.class`.
    Path,
}

impl LoadType {
    /// `Class.toString()`
    pub fn class_string(&self) -> &'static str {
        match self {
            LoadType::Model => "interface io.swagger.models.Model",
            LoadType::Parameter => "interface io.swagger.models.parameters.Parameter",
            LoadType::Response => "class io.swagger.models.Response",
            LoadType::Path => "class io.swagger.models.Path",
        }
    }
}

/// `Json.mapper().convertValue(tree, type)`
pub fn convert(tree: &JsonNode, t: LoadType) -> JResult<Option<Loaded>> {
    Ok(match t {
        LoadType::Model => model(tree)?.map(Loaded::Model),
        LoadType::Parameter => parameter(tree)?.map(Loaded::Parameter),
        LoadType::Response => response(tree)?.map(Loaded::Response),
        LoadType::Path => path(tree)?.map(Loaded::Path),
    })
}

/// A `LinkedHashMap` copy of an untyped JSON object (`convertValue(node, Map<String, Object>)`)
pub fn untyped_map(v: &JsonNode) -> JResult<Option<IndexMap<String, JavaObject>>> {
    object_map(v)
}
