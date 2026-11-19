//! `io.swagger.parser.util.SwaggerDeserializer` (swagger-parser 1.0.76): a Swagger 2.0 JSON tree to the
//! [`Swagger`] model, with the same validation messages.

use super::bean;
use super::models::*;
use super::property_builder::{self, Arg, Args, PropertyId};
use crate::java::{JMap, JResult, MapKind};
use indexmap::{IndexMap, IndexSet};
use jackson::convert::node_to_object;
use jackson::{BigDecimal, JavaObject, JsonNode};
use regex::Regex;
use std::collections::HashSet;
use std::sync::OnceLock;

const ROOT_KEYS: &[&str] = &["swagger", "info", "host", "basePath", "schemes", "consumes", "produces", "paths", "definitions", "parameters", "responses", "securityDefinitions", "security", "tags", "externalDocs"];
const EXTERNAL_DOCS_KEYS: &[&str] = &["description", "url"];
const SCHEMA_KEYS: &[&str] = &[
    "discriminator", "example", "$ref", "format", "title", "description", "default", "multipleOf", "maximum", "exclusiveMaximum", "minimum", "exclusiveMinimum", "maxLength", "minLength", "pattern", "maxItems", "minItems",
    "uniqueItems", "maxProperties", "minProperties", "required", "enum", "type", "items", "allOf", "properties", "additionalProperties", "xml", "readOnly", "allowEmptyValue",
];
const INFO_KEYS: &[&str] = &["title", "description", "termsOfService", "contact", "license", "version"];
const TAG_KEYS: &[&str] = &["description", "name", "externalDocs"];
const RESPONSE_KEYS: &[&str] = &["description", "schema", "headers", "examples"];
const CONTACT_KEYS: &[&str] = &["name", "url", "email"];
const LICENSE_KEYS: &[&str] = &["name", "url"];
const REF_MODEL_KEYS: &[&str] = &["$ref"];
const PATH_KEYS: &[&str] = &["$ref", "get", "put", "post", "delete", "head", "patch", "options", "parameters"];
const OPERATION_KEYS: &[&str] = &["scheme", "tags", "summary", "description", "externalDocs", "operationId", "consumes", "produces", "parameters", "responses", "schemes", "deprecated", "security"];
const PARAMETER_KEYS: &[&str] = &[
    "name", "in", "description", "required", "type", "format", "allowEmptyValue", "items", "collectionFormat", "default", "maximum", "exclusiveMaximum", "minimum", "exclusiveMinimum", "maxLength", "minLength",
    "pattern", "maxItems", "minItems", "uniqueItems", "enum", "multipleOf", "readOnly", "allowEmptyValue",
];
const BODY_PARAMETER_KEYS: &[&str] = &["name", "in", "description", "required", "schema"];
const SECURITY_SCHEME_KEYS: &[&str] = &["type", "name", "in", "description", "flow", "authorizationUrl", "tokenUrl", "scopes"];

type Location = (String, String);

/// `SwaggerDeserializer.ParseResult`
#[derive(Clone, Debug, Default)]
pub struct ParseResult {
    /// `isValid()`: `false` once a required field is missing or a node has the wrong type.
    pub valid: bool,
    extra: IndexSet<Location>,
    unsupported: IndexSet<Location>,
    invalid_type: IndexMap<Location, String>,
    warnings: Vec<Location>,
    missing: Vec<Location>,
    unique: Vec<Location>,
}

impl ParseResult {
    /// An empty, valid result.
    pub fn new() -> ParseResult {
        ParseResult { valid: true, ..Default::default() }
    }

    fn loc(location: &str, key: &str) -> Location {
        (location.to_string(), key.to_string())
    }

    /// Records `location.key` as an unsupported attribute ("is unsupported").
    pub fn unsupported(&mut self, location: &str, key: &str) {
        self.unsupported.insert(Self::loc(location, key));
    }

    /// Records `location.key` as an unexpected attribute ("is unexpected").
    pub fn extra(&mut self, location: &str, key: &str) {
        self.extra.insert(Self::loc(location, key));
    }

    /// Records `location.key` as a repeated value ("is repeated").
    pub fn unique(&mut self, location: &str, key: &str) {
        self.unique.push(Self::loc(location, key));
    }

    /// Records `location.key` as a missing required attribute ("is missing").
    pub fn missing(&mut self, location: &str, key: &str) {
        self.missing.push(Self::loc(location, key));
    }

    /// Records a free-form warning about `location.key`.
    pub fn warning(&mut self, location: &str, key: &str) {
        self.warnings.push(Self::loc(location, key));
    }

    /// Records `location.key` as not of the `expected` type ("is not of type `expected`").
    pub fn invalid_type(&mut self, location: &str, key: &str, expected: &str) {
        self.invalid_type.insert(Self::loc(location, key), expected.to_string());
    }

    /// Marks the result invalid.
    pub fn invalid(&mut self) {
        self.valid = false;
    }

    /// `getMessages()`
    pub fn get_messages(&self) -> Vec<String> {
        fn prefix(l: &str) -> String {
            if l.is_empty() { String::new() } else { format!("{l}.") }
        }
        let mut out = Vec::new();
        for (l, k) in &self.extra {
            out.push(format!("attribute {}{} is unexpected", prefix(l), k));
        }
        for ((l, k), t) in &self.invalid_type {
            out.push(format!("attribute {}{} is not of type `{}`", prefix(l), k, t));
        }
        for (l, k) in &self.missing {
            out.push(format!("attribute {}{} is missing", prefix(l), k));
        }
        for (l, k) in &self.warnings {
            out.push(format!("attribute {}{}", prefix(l), k));
        }
        for (l, k) in &self.unique {
            out.push(format!("attribute {}{} is repeated", prefix(l), k));
        }
        for (l, k) in &self.unsupported {
            out.push(format!("attribute {}{} is unsupported", prefix(l), k));
        }
        out
    }
}

fn keys(n: &JsonNode) -> Vec<String> {
    n.as_object().map(|o| o.iter().map(|(k, _)| k.clone()).collect()).unwrap_or_default()
}

fn extension(n: &JsonNode) -> JavaObject {
    node_to_object(n)
}

fn path_param_pattern() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\{(.+?)\}").unwrap())
}

/// `io.swagger.parser.util.SwaggerDeserializer`
#[derive(Default)]
pub struct SwaggerDeserializer {
    operation_ids: HashSet<String>,
}

impl SwaggerDeserializer {
    /// A deserializer with no operation ids seen yet.
    pub fn new() -> SwaggerDeserializer {
        SwaggerDeserializer::default()
    }

    /// `deserialize(rootNode)`
    pub fn deserialize(&mut self, root: &JsonNode) -> JResult<SwaggerDeserializationResult> {
        let mut r = ParseResult::new();
        let swagger = self.parse_root(root, &mut r)?;
        Ok(SwaggerDeserializationResult { swagger: swagger.map(shared), messages: Some(r.get_messages()) })
    }

    /// `parseRoot(node, result)`: the whole document; `None` when the root is not an object.
    pub fn parse_root(&mut self, node: &JsonNode, result: &mut ParseResult) -> JResult<Option<Swagger>> {
        let location = "";
        if !node.is_object() {
            result.invalid_type("", "", "object");
            result.invalid();
            return Ok(None);
        }
        let on = node;
        let mut swagger = Swagger { swagger: self.get_string("swagger", on, true, location, result, false), ..Swagger::default() };
        if let Some(obj) = self.get_object("info", on, true, "", result) {
            swagger.info = Some(self.info(obj, "info", result));
        }
        swagger.host = self.get_string("host", on, false, location, result, false);
        swagger.base_path = self.get_string("basePath", on, false, location, result, false);
        if let Some(array) = self.get_array("schemes", on, false, location, result) {
            for n in array {
                if let Some(s) = self.get_string_node(n, &format!("{location}.schemes"), result)
                    && let Some(scheme) = Scheme::for_value(&s)
                {
                    let l = swagger.schemes.get_or_insert_with(Vec::new);
                    if !l.contains(&Some(scheme)) {
                        l.push(Some(scheme));
                    }
                }
            }
        }
        if let Some(array) = self.get_array("consumes", on, false, location, result) {
            for n in array {
                if let Some(s) = self.get_string_node(n, &format!("{location}.consumes"), result) {
                    let l = swagger.consumes.get_or_insert_with(Vec::new);
                    if !l.contains(&s) {
                        l.push(s);
                    }
                }
            }
        }
        if let Some(array) = self.get_array("produces", on, false, location, result) {
            for n in array {
                if let Some(s) = self.get_string_node(n, &format!("{location}.produces"), result) {
                    let l = swagger.produces.get_or_insert_with(Vec::new);
                    if !l.contains(&s) {
                        l.push(s);
                    }
                }
            }
        }
        let obj = self.get_object("paths", on, true, location, result);
        swagger.paths = self.paths(obj, "paths", result)?;
        let obj = self.get_object("definitions", on, false, location, result);
        swagger.definitions = self.definitions(obj, "definitions", result)?;
        if let Some(obj) = self.get_object("parameters", on, false, location, result) {
            let mut parameters = IndexMap::new();
            for key in keys(obj) {
                let param_node = obj.get(&key).unwrap();
                if param_node.is_object() {
                    let parameter = self.parameter(Some(param_node), location, result)?;
                    // Java dereferences the result (a null parameter throws)
                    let Some(parameter) = parameter else {
                        return Err(crate::java::JavaException::without_message("java.lang.NullPointerException"));
                    };
                    {
                        let p = parameter.borrow();
                        if p.in_.as_deref().is_some_and(|i| i.eq_ignore_ascii_case("path")) && !p.required {
                            let name = p.name.clone().unwrap_or_else(|| "null".into());
                            result.warning(&format!("{location}.'{name}'"), &format!(" For path parameter '{name}' the required value should be true"));
                        }
                    }
                    parameters.insert(key, parameter);
                }
            }
            swagger.parameters = Some(parameters);
        }
        let obj = self.get_object("responses", on, false, location, result);
        swagger.responses = self.responses(obj, "responses", result)?;
        let obj = self.get_object("securityDefinitions", on, false, location, result);
        swagger.security_definitions = self.security_definitions(obj, "securityDefinitions", result)?;
        let array = self.get_array("security", on, false, location, result);
        swagger.security = self.security_requirements(array, "security", result);
        let array = self.get_array("tags", on, false, location, result);
        if let Some(tags) = self.tags(array, "tags", result) {
            // `tags(list)` sets the list as is
            swagger.tags = Some(tags);
        } else {
            swagger.tags = None;
        }
        let obj = self.get_object("externalDocs", on, false, location, result);
        swagger.external_docs = self.external_docs(obj, "externalDocs", result);
        for key in keys(on) {
            if key.starts_with("x-") {
                swagger.vendor_extension(&key, extension(on.get(&key).unwrap()));
            } else if !ROOT_KEYS.contains(&key.as_str()) {
                result.extra(location, &key);
            }
        }
        Ok(Some(swagger))
    }

    /// `paths(obj, location, result)`: the `paths` object; `None` when absent. A `$ref` path item fails with a `NullPointerException`, as in Java.
    pub fn paths(&mut self, obj: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> JResult<Option<IndexMap<String, PathRef>>> {
        let Some(obj) = obj else { return Ok(None) };
        let mut output = IndexMap::new();
        for path_name in keys(obj) {
            let path_value = obj.get(&path_name).unwrap();
            if path_name.starts_with("x-") {
                result.unsupported(location, &path_name);
            } else if !path_value.is_object() {
                result.invalid_type(location, &path_name, "object");
            } else {
                let path_obj = self.path(path_value, &format!("{location}.'{path_name}'"), result)?;
                let each_part: Vec<String> = path_param_pattern().find_iter(&path_name).map(|m| m.as_str().to_string()).collect();
                // a null path (a `$ref` object) makes Java throw a NullPointerException here
                let Some(path_obj) = path_obj else {
                    return Err(crate::java::JavaException::without_message("java.lang.NullPointerException"));
                };
                let operations = Self::all_operations_in_a_path(&path_obj.borrow());
                for op in &operations {
                    for p in op.borrow().parameters.clone().unwrap_or_default() {
                        let p = p.borrow();
                        if p.in_.as_deref().is_some_and(|i| i.eq_ignore_ascii_case("path")) && !p.required {
                            let name = p.name.clone().unwrap_or_else(|| "null".into());
                            result.warning(&format!("{location}.'{name}'"), &format!(" For path parameter '{name}' the required value should be true"));
                        }
                    }
                }
                for part in each_part {
                    let path_param = &part[1..part.len() - 1];
                    let defined_in_path_level = Self::is_path_param_defined(path_param, path_obj.borrow().parameters.as_deref());
                    if !defined_in_path_level {
                        for op in &operations {
                            if !Self::is_path_param_defined(path_param, op.borrow().parameters.as_deref()) {
                                result.warning(&format!("{location}.'{path_name}'"), &format!(" Declared path parameter {path_param} needs to be defined as a path parameter in path or operation level"));
                                break;
                            }
                        }
                    }
                }
                output.insert(path_name, path_obj);
            }
        }
        Ok(Some(output))
    }

    fn is_path_param_defined(path_param: &str, parameters: Option<&[ParameterRef]>) -> bool {
        let Some(parameters) = parameters else { return false };
        for p in parameters {
            let p = p.borrow();
            if p.kind == ParameterKind::Ref || (p.name.as_deref() == Some(path_param) && p.in_.as_deref() == Some("path")) {
                return true;
            }
        }
        false
    }

    fn all_operations_in_a_path(p: &Path) -> Vec<OperationRef> {
        [&p.get, &p.put, &p.post, &p.patch, &p.delete, &p.options, &p.head].into_iter().flatten().cloned().collect()
    }

    /// `path(obj, location, result)`: one path item; a [`Path::new_ref`] for a string `$ref`, `None` for any other `$ref`.
    pub fn path(&mut self, obj: &JsonNode, location: &str, result: &mut ParseResult) -> JResult<Option<PathRef>> {
        if let Some(r) = obj.get("$ref") {
            if r.is_textual() {
                return Ok(Some(Path::new_ref(r.text_value()).into_ref()));
            }
            if r.is_object() {
                for key in keys(r) {
                    result.extra(location, &key);
                }
            }
            return Ok(None);
        }
        let mut path = Path::new();
        let parameters = self.get_array("parameters", obj, false, location, result);
        path.parameters = Some(self.parameters(parameters, location, result)?);
        for (m, slot) in [("get", 0), ("put", 1), ("post", 2), ("head", 3), ("delete", 4), ("patch", 5), ("options", 6)] {
            if let Some(on) = self.get_object(m, obj, false, location, result)
                && let Some(op) = self.operation(Some(on), &format!("{location}({m})"), result)?
            {
                let op = Some(op);
                match slot {
                    0 => path.get = op,
                    1 => path.put = op,
                    2 => path.post = op,
                    3 => path.head = op,
                    4 => path.delete = op,
                    5 => path.patch = op,
                    _ => path.options = op,
                }
            }
        }
        for key in keys(obj) {
            if key.starts_with("x-") {
                path.set_vendor_extension(&key, extension(obj.get(&key).unwrap()));
            } else if !PATH_KEYS.contains(&key.as_str()) {
                result.extra(location, &key);
            }
        }
        Ok(Some(path.into_ref()))
    }

    /// `operation(obj, location, result)`: one operation; `None` when `obj` is absent.
    pub fn operation(&mut self, obj: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> JResult<Option<OperationRef>> {
        let Some(obj) = obj else { return Ok(None) };
        let mut output = Operation::new();
        let array = self.get_array("tags", obj, false, location, result);
        if let Some(tags) = Self::tag_strings(array) {
            output.tags = Some(tags);
        }
        output.summary = self.get_string("summary", obj, false, location, result, false);
        output.description = self.get_string("description", obj, false, location, result, false);
        let external_docs = self.get_object("externalDocs", obj, false, location, result);
        output.external_docs = self.external_docs(external_docs, location, result);
        output.operation_id = self.get_string("operationId", obj, false, location, result, true);
        if let Some(array) = self.get_array("consumes", obj, false, location, result) {
            if array.is_empty() {
                output.consumes = Some(Vec::new());
            } else {
                for n in array {
                    if let Some(s) = self.get_string_node(n, &format!("{location}.consumes"), result) {
                        output.consumes.get_or_insert_with(Vec::new).push(s);
                    }
                }
            }
        }
        if let Some(array) = self.get_array("produces", obj, false, location, result) {
            if array.is_empty() {
                output.produces = Some(Vec::new());
            } else {
                for n in array {
                    if let Some(s) = self.get_string_node(n, &format!("{location}.produces"), result) {
                        output.produces.get_or_insert_with(Vec::new).push(s);
                    }
                }
            }
        }
        let parameters = self.get_array("parameters", obj, false, location, result);
        output.parameters = Some(self.parameters(parameters, location, result)?);
        let responses = self.get_object("responses", obj, true, &format!("{location}.responses"), result);
        let responses_object = self.responses(responses, &format!("{location}.responses"), result)?;
        if responses_object.as_ref().is_some_and(|r| r.is_empty()) {
            result.missing(location, "responses");
        }
        output.responses = responses_object;
        if let Some(array) = self.get_array("schemes", obj, false, location, result) {
            for n in array {
                if let Some(s) = self.get_string_node(n, &format!("{location}.schemes"), result)
                    && let Some(scheme) = Scheme::for_value(&s)
                {
                    let l = output.schemes.get_or_insert_with(Vec::new);
                    if !l.contains(&Some(scheme)) {
                        l.push(Some(scheme));
                    }
                }
            }
        }
        if let Some(d) = self.get_boolean("deprecated", obj, false, location, result) {
            output.deprecated = Some(d);
        }
        let array = self.get_array("security", obj, false, location, result);
        if let Some(security) = self.security_requirements(array, location, result) {
            let mut ss = Vec::new();
            for s in security {
                ss.push(s.requirements);
            }
            output.security = Some(ss);
        }
        for key in keys(obj) {
            if key.starts_with("x-") {
                output.set_vendor_extension(&key, extension(obj.get(&key).unwrap()));
            } else if !OPERATION_KEYS.contains(&key.as_str()) {
                result.extra(location, &key);
            }
        }
        Ok(Some(output.into_ref()))
    }

    /// `getBoolean(key, node, required, location, result)`: a boolean field, or a string compared case-insensitively to `true`; `None` when absent or of another type (a missing required key invalidates the result).
    pub fn get_boolean(&self, key: &str, node: &JsonNode, required: bool, location: &str, result: &mut ParseResult) -> Option<bool> {
        match node.get(key) {
            Some(v) => match v {
                JsonNode::Boolean(b) => Some(*b),
                JsonNode::Text(s) => Some(s.eq_ignore_ascii_case("true")),
                _ => None,
            },
            None => {
                if required {
                    result.missing(location, key);
                    result.invalid();
                }
                None
            }
        }
    }

    /// `parameters(obj, location, result)`: the parameter objects of an array, skipping non-objects and unparseable entries; empty when absent.
    pub fn parameters(&mut self, obj: Option<&Vec<JsonNode>>, location: &str, result: &mut ParseResult) -> JResult<Vec<ParameterRef>> {
        let mut output = Vec::new();
        let Some(obj) = obj else { return Ok(output) };
        for item in obj {
            if item.is_object() && let Some(p) = self.parameter(Some(item), location, result)? {
                output.push(p);
            }
        }
        Ok(output)
    }

    /// `parameter(obj, location, result)`: one parameter, typed by its `in`; a [`Parameter::new_ref`] for a string `$ref`, `None` when `in` is missing or unknown.
    pub fn parameter(&mut self, obj: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> JResult<Option<ParameterRef>> {
        let Some(obj) = obj else { return Ok(None) };
        if let Some(r) = obj.get("$ref") {
            if r.is_textual() {
                return Ok(Some(Parameter::new_ref(&r.as_text()).into_ref()));
            }
            result.invalid_type(location, "$ref", "string");
            return Ok(None);
        }
        let l = match obj.get("name") {
            Some(n) => n.as_text(),
            None => "['unknown']".to_string(),
        };
        let location = format!("{location}.[{l}]");
        let location = location.as_str();
        let Some(value) = self.get_string("in", obj, true, location, result, false) else { return Ok(None) };
        let ty = self.get_string("type", obj, false, location, result, false);
        let format = self.get_string("format", obj, false, location, result, false);
        let kind = match value.as_str() {
            "query" => Some(ParameterKind::Query),
            "header" => Some(ParameterKind::Header),
            "path" => Some(ParameterKind::Path),
            "formData" => Some(ParameterKind::Form),
            _ => None,
        };
        let mut output: Option<Parameter> = None;
        if let Some(kind) = kind {
            let mut sp = Parameter::new(kind);
            let param_type = self.get_string("type", obj, true, location, result, false);
            let mut map = Args::new();
            map.put(PropertyId::Type, Arg::opt_str(ty.clone()));
            map.put(PropertyId::Format, Arg::opt_str(format.clone()));
            let default_value = self.parameter_default(obj, param_type.as_deref(), location, result);
            map.put(PropertyId::Default, Arg::opt_str(default_value.clone()));
            sp.set_default(default_value);
            if let Some(bd) = self.get_big_decimal("maximum", obj, false, location, result)? {
                map.put(PropertyId::Maximum, Arg::Decimal(bd.clone()));
                sp.maximum = Some(bd);
            }
            if let Some(bl) = self.get_boolean("exclusiveMaximum", obj, false, location, result) {
                map.put(PropertyId::ExclusiveMaximum, Arg::Bool(bl));
                sp.exclusive_maximum = Some(bl);
            }
            if let Some(bd) = self.get_big_decimal("minimum", obj, false, location, result)? {
                map.put(PropertyId::Minimum, Arg::Decimal(bd.clone()));
                sp.minimum = Some(bd);
            }
            if let Some(bl) = self.get_boolean("exclusiveMinimum", obj, false, location, result) {
                map.put(PropertyId::ExclusiveMinimum, Arg::Bool(bl));
                sp.exclusive_minimum = Some(bl);
            }
            let max_length = self.get_integer("maxLength", obj, false, location, result);
            map.put(PropertyId::MaxLength, Arg::opt_int(max_length));
            sp.max_length = max_length;
            let min_length = self.get_integer("minLength", obj, false, location, result);
            map.put(PropertyId::MinLength, Arg::opt_int(min_length));
            sp.min_length = min_length;
            let pat = self.get_string("pattern", obj, false, location, result, false);
            map.put(PropertyId::Pattern, Arg::opt_str(pat.clone()));
            sp.pattern = pat;
            let iv = self.get_integer("maxItems", obj, false, location, result);
            map.put(PropertyId::MaxItems, Arg::opt_int(iv));
            sp.max_items = iv;
            let iv = self.get_integer("minItems", obj, false, location, result);
            map.put(PropertyId::MinItems, Arg::opt_int(iv));
            sp.min_items = iv;
            let iv = self.get_integer("minLength", obj, false, location, result);
            map.put(PropertyId::MinLength, Arg::opt_int(iv));
            sp.min_length = iv;
            let iv = self.get_integer("maxLength", obj, false, location, result);
            map.put(PropertyId::MaxLength, Arg::opt_int(iv));
            sp.max_length = iv;
            if let Some(bd) = self.get_big_decimal("multipleOf", obj, false, location, result)? {
                map.put(PropertyId::MultipleOf, Arg::Decimal(bd.clone()));
                sp.multiple_of = Some(JavaObject::Double(bd.to_f64()));
            }
            let unique_items = self.get_boolean("uniqueItems", obj, false, location, result);
            map.put(PropertyId::UniqueItems, Arg::opt_bool(unique_items));
            sp.unique_items = unique_items;
            if let Some(an) = self.get_array("enum", obj, false, location, result) {
                let mut e = Vec::new();
                for n in an {
                    if n.is_value_node() {
                        e.push(n.as_text());
                    } else {
                        result.invalid_type(location, "enum", "value");
                    }
                }
                sp.enum_ = Some(e.clone());
                map.put(PropertyId::Enum, Arg::List(e));
            }
            if let Some(bl) = self.get_boolean("readOnly", obj, false, location, result) {
                map.put(PropertyId::ReadOnly, Arg::Bool(bl));
                sp.read_only = Some(bl);
            }
            if let Some(bl) = self.get_boolean("allowEmptyValue", obj, false, location, result) {
                map.put(PropertyId::AllowEmptyValue, Arg::Bool(bl));
                sp.allow_empty_value = Some(bl);
            }
            if let Some(prop) = property_builder::build(ty.as_deref(), format.as_deref(), &map)? {
                sp.set_property(&prop);
                if let Some(items) = self.get_object("items", obj, false, location, result) {
                    let inner = property_builder::property_from_node(items)?;
                    sp.items = inner;
                }
            }
            for key in keys(obj) {
                if key.starts_with("x-") {
                    sp.set_vendor_extension(&key, extension(obj.get(&key).unwrap()));
                } else if !PARAMETER_KEYS.contains(&key.as_str()) {
                    result.extra(location, &key);
                }
            }
            sp.collection_format = self.get_string("collectionFormat", obj, false, location, result, false);
            output = Some(sp);
        } else if value == "body" {
            let mut bp = Parameter::new(ParameterKind::Body);
            if let Some(node) = obj.get("schema") && node.is_object() {
                bp.schema = self.definition(Some(node), location, result)?;
            }
            if let Some(examples) = self.get_object("examples", obj, false, location, result) {
                bp.examples = bean::untyped_map(examples)?;
            }
            if let Some(pat) = self.get_string("pattern", obj, false, location, result, false) {
                bp.pattern = Some(pat);
            }
            if let Some(bl) = self.get_boolean("readOnly", obj, false, location, result) {
                bp.read_only = Some(bl);
            }
            for key in keys(obj) {
                if key.starts_with("x-") {
                    bp.set_vendor_extension(&key, extension(obj.get(&key).unwrap()));
                } else if !BODY_PARAMETER_KEYS.contains(&key.as_str()) {
                    result.extra(location, &key);
                }
            }
            output = Some(bp);
        }
        let Some(mut output) = output else { return Ok(None) };
        output.name = self.get_string("name", obj, true, location, result, false);
        output.description = self.get_string("description", obj, false, location, result, false);
        if let Some(required) = self.get_boolean("required", obj, false, location, result) {
            output.required = required;
        }
        Ok(Some(output.into_ref()))
    }

    fn parameter_default(&mut self, node: &JsonNode, ty: Option<&str>, location: &str, result: &mut ParseResult) -> Option<String> {
        if ty == Some("array") {
            self.get_array("default", node, false, location, result).map(|a| JsonNode::array(a.clone()).to_java_string())
        } else {
            self.get_string("default", node, false, location, result, false)
        }
    }

    /// `definitions(node, location, result)`: definition name to its schema; `None` when absent.
    pub fn definitions(&mut self, node: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> JResult<Option<IndexMap<String, ModelRef>>> {
        let Some(node) = node else { return Ok(None) };
        let mut output = IndexMap::new();
        for name in keys(node) {
            let schema = node.get(&name).unwrap();
            if schema.is_object() {
                if let Some(model) = self.definition(Some(schema), &format!("{location}.{name}"), result)? {
                    output.insert(name, model);
                }
            } else {
                result.invalid_type(location, &name, "object");
            }
        }
        Ok(Some(output))
    }

    /// `definition(node, location, result)`
    pub fn definition(&mut self, node: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> JResult<Option<ModelRef>> {
        let Some(node) = node else {
            result.missing(location, "empty schema");
            return Ok(None);
        };
        if let JsonNode::Boolean(b) = node {
            return Ok(Some(Model::new_boolean_value(*b).into_ref()));
        }
        if node.get("$ref").is_some() {
            return self.ref_model(node, location, result);
        }
        if node.get("allOf").is_some() {
            return self.all_of_model(node, location, result);
        }
        let ty = self.get_string("type", node, false, location, result, false);
        let items_key = node.get("items");
        let mut model;
        if ty.as_deref() != Some("array") && items_key.is_none() {
            let mut imp = Model::new(ModelKind::Impl);
            imp.type_ = ty;
            let ap = node.get("additionalProperties");
            match ap {
                Some(a) if a.is_object() => {
                    let p = property_builder::property_from_node(a)?;
                    imp.set_additional_properties(p);
                }
                Some(JsonNode::Boolean(b)) => imp.set_additional_properties(Some(Property::new_boolean_value(*b).into_ref())),
                _ => {}
            }
            imp.default_value = self.get_string("default", node, false, location, result, false);
            imp.format = self.get_string("format", node, false, location, result, false);
            imp.discriminator = self.get_string("discriminator", node, false, location, result, false);
            if let Some(bp) = self.get_boolean("uniqueItems", node, false, location, result) {
                imp.unique_items = Some(bp);
            }
            imp.minimum = self.get_big_decimal("minimum", node, false, location, result)?;
            imp.maximum = self.get_big_decimal("maximum", node, false, location, result)?;
            if let Some(bp) = self.get_boolean("exclusiveMaximum", node, false, location, result) {
                imp.exclusive_maximum = Some(bp);
            }
            if let Some(bp) = self.get_boolean("exclusiveMinimum", node, false, location, result) {
                imp.exclusive_minimum = Some(bp);
            }
            imp.pattern = self.get_string("pattern", node, false, location, result, false);
            if let Some(m) = self.get_big_decimal("maximum", node, false, location, result)? {
                imp.maximum = Some(m);
            }
            if let Some(m) = self.get_big_decimal("minimum", node, false, location, result)? {
                imp.minimum = Some(m);
            }
            if let Some(v) = self.get_integer("minLength", node, false, location, result) {
                imp.min_length = Some(v);
            }
            if let Some(v) = self.get_integer("maxLength", node, false, location, result) {
                imp.max_length = Some(v);
            }
            if let Some(v) = self.get_big_decimal("multipleOf", node, false, location, result)? {
                imp.multiple_of = Some(v);
            }
            if node.get("enum").is_some() && let Some(array) = self.get_array("enum", node, false, location, result) {
                for n in array {
                    if n.is_value_node() {
                        imp.enum_.get_or_insert_with(Vec::new).push(n.as_text());
                    } else {
                        result.invalid_type(location, "enum", "value");
                    }
                }
            }
            if let Some(xml) = node.get("xml") {
                imp.xml = bean::xml(xml)?;
            }
            let external_docs = self.get_object("externalDocs", node, false, location, result);
            imp.external_docs = self.external_docs(external_docs, location, result);
            self.add_properties(location, node, result, &mut imp)?;
            if let Some(required) = self.get_array("required", node, false, location, result) {
                let mut req = Vec::new();
                for n in required {
                    if let JsonNode::Text(t) = n {
                        req.push(t.to_string());
                    } else {
                        result.invalid_type(location, "required", "string");
                    }
                }
                if !req.is_empty() {
                    imp.set_required(Some(req));
                }
            }
            for key in keys(node) {
                if key.starts_with("x-") {
                    imp.set_vendor_extension(&key, extension(node.get(&key).unwrap()));
                } else if !SCHEMA_KEYS.contains(&key.as_str()) {
                    result.extra(location, &key);
                }
            }
            model = imp;
        } else {
            let mut am = Model::new(ModelKind::Array);
            let property_node = self.get_object("properties", node, false, location, result);
            let properties = self.properties(property_node, location, result)?;
            if let Some(p) = properties {
                for (k, v) in p {
                    am.add_property(&k, v);
                }
            }
            let items_node = self.get_object("items", node, false, location, result);
            if let Some(items) = self.property(items_node, location, result)? {
                am.items = Some(items);
            }
            am.max_items = self.get_integer("maxItems", node, false, location, result);
            am.min_items = self.get_integer("minItems", node, false, location, result);
            if let Some(u) = self.get_boolean("uniqueItems", node, false, location, result) {
                am.unique_items = Some(u);
            }
            if let Some(xml) = node.get("xml") {
                am.xml = bean::xml(xml)?;
            }
            for key in keys(node) {
                if key.starts_with("x-") {
                    am.set_vendor_extension(&key, extension(node.get(&key).unwrap()));
                }
            }
            model = am;
        }
        if let Some(example_node) = node.get("example") {
            model.set_example(Some(node_to_object(example_node)));
        }
        model.set_description(self.get_string("description", node, false, location, result, false));
        model.title = self.get_string("title", node, false, location, result, false);
        Ok(Some(model.into_ref()))
    }

    fn all_of_model(&mut self, node: &JsonNode, location: &str, result: &mut ParseResult) -> JResult<Option<ModelRef>> {
        let sub = node.get("$ref");
        let all_of = node.get("allOf");
        if let Some(sub) = sub {
            if sub.is_object() {
                return self.ref_model(sub, location, result);
            }
            result.invalid_type(location, "$ref", "object");
            return Ok(None);
        }
        let Some(all_of) = all_of else { return Ok(None) };
        let mut model: Option<Model> = None;
        if let Some(parts) = all_of.as_array() {
            let mut m = Model::new(ModelKind::Composed);
            for (pos, part) in parts.iter().enumerate() {
                if part.is_object() {
                    if let Some(segment) = self.definition(Some(part), location, result)? {
                        m.all_of.push(segment);
                    }
                } else {
                    result.invalid_type(location, &format!("allOf[{pos}]"), "object");
                }
            }
            let mut child = None;
            let mut interfaces = Vec::new();
            for x in &m.all_of {
                match x.borrow().kind {
                    ModelKind::Ref => interfaces.push(x.clone()),
                    ModelKind::Impl => child = Some(x.clone()),
                    _ => {}
                }
            }
            m.interfaces = Some(interfaces);
            if let Some(c) = child {
                m.child = Some(c);
            }
            model = Some(m);
        } else {
            result.invalid_type(location, "allOf", "array");
        }
        // Java continues with a null model here and throws a NullPointerException at the first use.
        let Some(mut model) = model else {
            return Err(crate::java::JavaException::without_message("java.lang.NullPointerException"));
        };
        for key in keys(node) {
            if key.starts_with("x-") {
                model.set_vendor_extension(&key, extension(node.get(&key).unwrap()));
            } else if !SCHEMA_KEYS.contains(&key.as_str()) {
                result.extra(location, &key);
            } else {
                model.title = self.get_string("title", node, false, location, result, false);
                model.description = self.get_string("description", node, false, location, result, false);
            }
        }
        self.add_properties(location, node, result, &mut model)?;
        if let Some(required) = self.get_array("required", node, false, location, result) {
            let mut req = Vec::new();
            for n in required {
                if let JsonNode::Text(t) = n {
                    req.push(t.to_string());
                } else {
                    result.invalid_type(location, "required", "string");
                }
            }
            if !req.is_empty() {
                model.set_required(Some(req));
            }
        }
        Ok(Some(model.into_ref()))
    }

    fn add_properties(&mut self, location: &str, node: &JsonNode, result: &mut ParseResult, model: &mut Model) -> JResult<()> {
        let Some(properties) = self.get_object("properties", node, false, location, result) else { return Ok(()) };
        for name in keys(properties) {
            let pn = properties.get(&name).unwrap();
            if pn.is_object() {
                let property = self.property(Some(pn), location, result)?;
                if let Some(p) = &property {
                    let p = p.borrow();
                    if p.type_.as_deref() == Some("array") && (p.kind != PropertyKind::Array || p.items.is_none()) {
                        result.missing(location, "items");
                    }
                }
                model.add_property(&name, property);
            } else {
                result.invalid_type(location, "properties", "object");
            }
        }
        Ok(())
    }

    /// `properties(node, location, result)`: property name to its schema (`None` values for unparseable ones); `None` when absent.
    pub fn properties(&mut self, node: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> JResult<Option<IndexMap<String, Option<PropertyRef>>>> {
        let Some(node) = node else { return Ok(None) };
        let mut output = IndexMap::new();
        for name in keys(node) {
            let pn = node.get(&name).unwrap();
            if pn.is_object() {
                let p = self.property(Some(pn), location, result)?;
                output.insert(name, p);
            } else {
                result.invalid_type(location, &name, "object");
            }
        }
        Ok(Some(output))
    }

    /// `property(node, location, result)`: infers `type` from `enum` values, then `convertValue(Property)`
    pub fn property(&mut self, node: Option<&JsonNode>, _location: &str, _result: &mut ParseResult) -> JResult<Option<PropertyRef>> {
        let Some(node) = node else {
            // Json.mapper().convertValue(null, Property.class)
            return Ok(None);
        };
        if let JsonNode::Boolean(b) = node {
            return Ok(Some(Property::new_boolean_value(*b).into_ref()));
        }
        let mut node = node.clone();
        if node.get("type").is_none() && let Some(JsonNode::Array(a)) = node.get("enum").cloned() {
            let t = Self::infer_type_from_array(&a);
            if let Some(o) = node.as_object_mut() {
                o.insert("type".to_string(), JsonNode::text(t));
            }
        }
        property_builder::property_from_node(&node)
    }

    /// `inferTypeFromArray` (looks at the first element only, as the Java loop does)
    pub fn infer_type_from_array(an: &[JsonNode]) -> String {
        if an.is_empty() {
            return "string".into();
        }
        let mut t: Option<&str> = None;
        for _ in 0..an.len() {
            let e = &an[0];
            if e.is_boolean() {
                t = Some(if t.is_none() || t == Some("boolean") { "boolean" } else { "string" });
            } else if e.is_number() {
                t = Some(if t.is_none() || t == Some("number") { "number" } else { "string" });
            } else {
                t = Some("string");
            }
        }
        t.unwrap().to_string()
    }

    fn ref_model(&mut self, node: &JsonNode, location: &str, result: &mut ParseResult) -> JResult<Option<ModelRef>> {
        if !node.is_object() {
            result.invalid_type(location, "$ref", "object");
            return Ok(None);
        }
        let r = node.get("$ref").unwrap();
        let JsonNode::Text(t) = r else {
            return Err(crate::java::JavaException::class_cast(
                &format!("com.fasterxml.jackson.databind.node.{}", r.node_class_name()),
                "com.fasterxml.jackson.databind.node.TextNode",
            ));
        };
        let mut output = Model::new(ModelKind::Ref);
        output.set_ref(t);
        for key in keys(node) {
            if !REF_MODEL_KEYS.contains(&key.as_str()) {
                result.extra(location, &key);
            }
        }
        Ok(Some(output.into_ref()))
    }

    /// `responses(obj, location, result)`: status code to response; `x-` keys are dropped; `None` when absent.
    pub fn responses(&mut self, node: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> JResult<Option<Responses>> {
        let Some(node) = node else { return Ok(None) };
        let mut output = Responses::new();
        for key in keys(node) {
            if key.starts_with("x-") {
                // Responses.addVendorExtension: kept on the Responses object, which nothing reads back
                continue;
            }
            let obj = self.get_object(&key, node, false, &format!("{location}.responses"), result);
            let response = self.response(obj, &format!("{location}.{key}"), result)?;
            output.insert(key, response);
        }
        Ok(Some(output))
    }

    /// `response(node, location, result)`: one response; a [`Response::new_ref`] for a string `$ref`, `None` when absent or for a non-string `$ref`.
    pub fn response(&mut self, node: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> JResult<Option<ResponseRef>> {
        let Some(node) = node else { return Ok(None) };
        let mut output = Response::new();
        if let Some(r) = node.get("$ref") {
            if r.is_textual() {
                return Ok(Some(Response::new_ref(&r.as_text()).into_ref()));
            }
            result.invalid_type(location, "$ref", "string");
            return Ok(None);
        }
        output.description = self.get_string("description", node, true, location, result, false);
        if let Some(schema) = self.get_object("schema", node, false, location, result) {
            if let Some(schema_ref) = schema.get("$ref") {
                if let JsonNode::Text(t) = schema_ref {
                    output.schema = Some(Model::new_ref(t, None).into_ref());
                } else {
                    result.invalid_type(location, "$ref", "string");
                }
            } else {
                output.schema = self.definition(Some(schema), &format!("{location}.schema"), result)?;
            }
        }
        if let Some(headers) = self.get_object("headers", node, false, location, result) {
            output.headers = bean::property_map(headers)?;
        }
        if let Some(examples) = self.get_object("examples", node, false, location, result) {
            output.examples = bean::untyped_map(examples)?;
        }
        for key in keys(node) {
            if key.starts_with("x-") {
                output.set_vendor_extension(&key, extension(node.get(&key).unwrap()));
            } else if !RESPONSE_KEYS.contains(&key.as_str()) {
                result.extra(location, &key);
            }
        }
        Ok(Some(output.into_ref()))
    }

    /// `info(node, location, result)`: the `info` object; `title` and `version` are required.
    pub fn info(&mut self, node: &JsonNode, location: &str, result: &mut ParseResult) -> Info {
        let mut info = Info {
            title: self.get_string("title", node, true, location, result, false),
            description: self.get_string("description", node, false, location, result, false),
            terms_of_service: self.get_string("termsOfService", node, false, location, result, false),
            ..Info::default()
        };
        let obj = self.get_object("contact", node, false, "contact", result);
        info.contact = self.contact(obj, location, result);
        let obj = self.get_object("license", node, false, location, result);
        info.license = self.license(obj, location, result);
        info.version = self.get_string("version", node, true, location, result, false);
        for key in keys(node) {
            if key.starts_with("x-") {
                info.vendor_extensions.put(key.clone(), extension(node.get(&key).unwrap()));
            } else if !INFO_KEYS.contains(&key.as_str()) {
                result.extra(location, &key);
            }
        }
        info
    }

    fn license(&mut self, node: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> Option<License> {
        let node = node?;
        let license = License {
            name: self.get_string("name", node, true, location, result, false),
            url: self.get_string("url", node, false, location, result, false),
            ..License::default()
        };
        for key in keys(node) {
            if key.starts_with("x-") {
                license.vendor_extensions.put(key.clone(), extension(node.get(&key).unwrap()));
            } else if !LICENSE_KEYS.contains(&key.as_str()) {
                result.extra(&format!("{location}.license"), &key);
            }
        }
        Some(license)
    }

    fn contact(&mut self, node: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> Option<Contact> {
        let node = node?;
        let contact = Contact {
            name: self.get_string("name", node, false, &format!("{location}.name"), result, false),
            url: self.get_string("url", node, false, &format!("{location}.url"), result, false),
            email: self.get_string("email", node, false, &format!("{location}.email"), result, false),
            ..Contact::default()
        };
        for key in keys(node) {
            if key.starts_with("x-") {
                contact.vendor_extensions.put(key.clone(), extension(node.get(&key).unwrap()));
            } else if !CONTACT_KEYS.contains(&key.as_str()) {
                result.extra(&format!("{location}.contact"), &key);
            }
        }
        Some(contact)
    }

    /// `securityDefinitions(node, location, result)`: scheme name to security scheme; `None` when absent.
    pub fn security_definitions(&mut self, node: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> JResult<Option<IndexMap<String, SecuritySchemeDefinition>>> {
        let Some(node) = node else { return Ok(None) };
        let mut output = IndexMap::new();
        for key in keys(node) {
            let obj = self.get_object(&key, node, false, location, result);
            if let Some(def) = self.security_definition(obj, &format!("{location}.{key}"), result)? {
                output.insert(key, def);
            }
        }
        Ok(Some(output))
    }

    fn security_definition(&mut self, node: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> JResult<Option<SecuritySchemeDefinition>> {
        let Some(node) = node else { return Ok(None) };
        let mut output: Option<SecuritySchemeDefinition> = None;
        if let Some(t) = self.get_string("type", node, true, location, result, false) {
            if t == "basic" {
                output = Some(bean::security_definition(SecuritySchemeKind::Basic, node)?);
            } else if t != "apiKey" {
                if t == "oauth2" {
                    let mut d = bean::security_definition(SecuritySchemeKind::OAuth2, node)?;
                    if let Some(desc) = node.get("description") {
                        d.description = desc.text_value().map(str::to_string);
                    }
                    output = Some(d);
                } else {
                    result.invalid_type(&format!("{location}.type"), "type", "basic|apiKey|oauth2");
                }
            } else {
                let position = self.get_string("in", node, true, location, result, false);
                let name = self.get_string("name", node, true, location, result, false);
                if let Some(name) = &name
                    && (position.as_deref() == Some("header") || position.as_deref() == Some("query"))
                    && let Some(in_) = In::for_value(position.as_deref().unwrap())
                {
                    let mut d = SecuritySchemeDefinition::new(SecuritySchemeKind::ApiKey);
                    d.name = Some(name.clone());
                    d.in_ = Some(in_);
                    d.description = self.get_string("description", node, false, location, result, false);
                    output = Some(d);
                }
                if let Some(desc) = node.get("description") {
                    match &mut output {
                        Some(o) => o.description = desc.text_value().map(str::to_string),
                        None => return Err(crate::java::JavaException::without_message("java.lang.NullPointerException")),
                    }
                }
            }
            for key in keys(node) {
                if key.starts_with("x-") {
                    match &mut output {
                        Some(o) => {
                            o.vendor_extensions.put(key.clone(), extension(node.get(&key).unwrap()));
                        }
                        None => return Err(crate::java::JavaException::without_message("java.lang.NullPointerException")),
                    }
                } else if !SECURITY_SCHEME_KEYS.contains(&key.as_str()) {
                    result.extra(location, &key);
                }
            }
        }
        Ok(output)
    }

    /// `securityRequirements(node, location, result)`: the `security` array, each object as scheme name to scopes; `None` when absent.
    pub fn security_requirements(&mut self, node: Option<&Vec<JsonNode>>, location: &str, result: &mut ParseResult) -> Option<Vec<SecurityRequirement>> {
        let node = node?;
        let mut output = Vec::new();
        for item in node {
            let mut security = SecurityRequirement::default();
            if item.is_object() {
                for key in keys(item) {
                    let mut scopes = Vec::new();
                    if let Some(obj) = self.get_array(&key, item, false, &format!("{location}.security"), result) {
                        for n in obj {
                            if let JsonNode::Text(t) = n {
                                scopes.push(t.to_string());
                            } else {
                                result.invalid_type(location, &key, "string");
                            }
                        }
                    }
                    security.requirements.insert(key, scopes);
                }
            }
            output.push(security);
        }
        Some(output)
    }

    fn tag_strings(nodes: Option<&Vec<JsonNode>>) -> Option<Vec<String>> {
        let nodes = nodes?;
        Some(nodes.iter().filter_map(|n| n.text_value().map(str::to_string)).collect())
    }

    fn tags(&mut self, nodes: Option<&Vec<JsonNode>>, location: &str, result: &mut ParseResult) -> Option<Vec<Tag>> {
        let nodes = nodes?;
        let mut output = Vec::new();
        for n in nodes {
            if n.is_object() {
                output.push(self.tag(n, &format!("{location}.tags"), result));
            }
        }
        Some(output)
    }

    fn tag(&mut self, node: &JsonNode, location: &str, result: &mut ParseResult) -> Tag {
        let mut tag = Tag {
            name: self.get_string("name", node, true, location, result, false),
            description: self.get_string("description", node, false, location, result, false),
            ..Tag::default()
        };
        let external_docs = self.get_object("externalDocs", node, false, location, result);
        tag.external_docs = self.external_docs(external_docs, &format!("{location}externalDocs"), result);
        for key in keys(node) {
            if key.starts_with("x-") {
                tag.vendor_extensions.put(key.clone(), extension(node.get(&key).unwrap()));
            } else if !TAG_KEYS.contains(&key.as_str()) {
                result.extra(&format!("{location}.externalDocs"), &key);
            }
        }
        tag
    }

    fn external_docs(&mut self, node: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> Option<ExternalDocs> {
        let node = node?;
        let output = ExternalDocs {
            description: self.get_string("description", node, false, location, result, false),
            url: self.get_string("url", node, true, location, result, false),
            ..ExternalDocs::default()
        };
        for key in keys(node) {
            if key.starts_with("x-") {
                output.vendor_extensions.put(key.clone(), extension(node.get(&key).unwrap()));
            } else if !EXTERNAL_DOCS_KEYS.contains(&key.as_str()) {
                result.extra(&format!("{location}.externalDocs"), &key);
            }
        }
        Some(output)
    }

    /// `getString(JsonNode node, location, result)`
    fn get_string_node(&self, node: &JsonNode, location: &str, result: &mut ParseResult) -> Option<String> {
        match node {
            JsonNode::Text(t) => Some(t.to_string()),
            _ => {
                result.invalid_type(location, "", "string");
                None
            }
        }
    }

    fn get_array<'a>(&self, key: &str, node: &'a JsonNode, required: bool, location: &str, result: &mut ParseResult) -> Option<&'a Vec<JsonNode>> {
        match node.get(key) {
            None => {
                if required {
                    result.missing(location, key);
                    result.invalid();
                }
                None
            }
            Some(v) => match v.as_array() {
                Some(a) => Some(a),
                None => {
                    result.invalid_type(location, key, "array");
                    None
                }
            },
        }
    }

    fn get_object<'a>(&self, key: &str, node: &'a JsonNode, required: bool, location: &str, result: &mut ParseResult) -> Option<&'a JsonNode> {
        match node.get(key) {
            None => {
                if required {
                    result.missing(location, key);
                    result.invalid();
                }
                None
            }
            Some(v) if v.is_object() => Some(v),
            Some(_) => {
                result.invalid_type(location, key, "object");
                if required {
                    result.invalid();
                }
                None
            }
        }
    }

    fn get_big_decimal(&self, key: &str, node: &JsonNode, required: bool, location: &str, result: &mut ParseResult) -> JResult<Option<BigDecimal>> {
        match node.get(key) {
            Some(v) => {
                if v.is_number() {
                    let s = v.as_text();
                    return BigDecimal::parse(&s).map(Some).map_err(|_| property_builder::number_format_bigdecimal(&s));
                }
                if !v.is_value_node() {
                    result.invalid_type(location, key, "double");
                }
                Ok(None)
            }
            None => {
                if required {
                    result.missing(location, key);
                    result.invalid();
                }
                Ok(None)
            }
        }
    }

    fn get_integer(&self, key: &str, node: &JsonNode, required: bool, location: &str, result: &mut ParseResult) -> Option<i32> {
        match node.get(key) {
            Some(v) => {
                if v.is_number() {
                    return Some(v.int_value());
                }
                if !v.is_value_node() {
                    result.invalid_type(location, key, "integer");
                }
                None
            }
            None => {
                if required {
                    result.missing(location, key);
                    result.invalid();
                }
                None
            }
        }
    }

    /// `getString(key, node, required, location, result[, uniqueValues])`; `unique` checks operationIds.
    fn get_string(&mut self, key: &str, node: &JsonNode, required: bool, location: &str, result: &mut ParseResult, unique: bool) -> Option<String> {
        match node.get(key) {
            Some(v) => {
                if !v.is_value_node() {
                    result.invalid_type(location, key, "string");
                    return None;
                }
                let value = v.as_text();
                if unique && !self.operation_ids.insert(value.clone()) {
                    result.unique(location, "operationId");
                    result.invalid();
                }
                Some(value)
            }
            None => {
                if required {
                    result.missing(location, key);
                    result.invalid();
                }
                None
            }
        }
    }
}

/// `HashMap`-ordered copy (used where Java builds a `HashMap`)
pub fn hash_map_of<V: Clone>(entries: impl IntoIterator<Item = (String, V)>) -> JMap<V> {
    JMap::from_entries(MapKind::Hash, entries)
}
