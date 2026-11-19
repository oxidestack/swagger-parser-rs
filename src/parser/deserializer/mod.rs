//! `io.swagger.v3.parser.util.OpenAPIDeserializer`: JSON tree → OpenAPI model, with the exact validation
//! messages of swagger-parser 2.1.48.

pub mod schema;
pub use schema::{create_schema, create_schema_by_type};

use crate::core::{ParseOptions, SwaggerParseResult};
use crate::java::net::{is_valid_uri, is_valid_url, JavaUri, JavaUrl};
use crate::java::{JList, JMap, JResult, JavaException};
use crate::models::*;
use indexmap::IndexMap;
use jackson::convert::node_to_object;
use jackson::{BigDecimal, JavaObject, JsonNode};
use std::collections::HashSet;
use std::sync::OnceLock;

pub(crate) const ROOT_KEYS: &[&str] = &["openapi", "info", "servers", "paths", "components", "security", "tags", "externalDocs"];
const INFO_KEYS: &[&str] = &["title", "description", "termsOfService", "contact", "license", "version"];
const CONTACT_KEYS: &[&str] = &["name", "url", "email"];
const LICENSE_KEYS: &[&str] = &["name", "url"];
const TAG_KEYS: &[&str] = &["description", "name", "externalDocs"];
const RESPONSE_KEYS: &[&str] = &["$ref", "description", "headers", "content", "links"];
const SERVER_KEYS: &[&str] = &["url", "description", "variables"];
const SERVER_VARIABLE_KEYS: &[&str] = &["enum", "default", "description"];
const PATHITEM_KEYS: &[&str] = &["$ref", "summary", "description", "get", "put", "post", "delete", "head", "patch", "options", "trace", "servers", "parameters"];
const OPERATION_KEYS: &[&str] =
    &["tags", "summary", "description", "externalDocs", "operationId", "parameters", "requestBody", "responses", "callbacks", "deprecated", "security", "servers"];
const PARAMETER_KEYS: &[&str] =
    &["$ref", "name", "in", "description", "required", "deprecated", "allowEmptyValue", "style", "explode", "allowReserved", "schema", "example", "examples", "content"];
const REQUEST_BODY_KEYS: &[&str] = &["$ref", "description", "content", "required"];
const SECURITY_SCHEME_KEYS: &[&str] = &["$ref", "type", "name", "in", "description", "flows", "scheme", "bearerFormat", "openIdConnectUrl"];
const EXTERNAL_DOCS_KEYS: &[&str] = &["description", "url"];
const COMPONENTS_KEYS: &[&str] = &["schemas", "responses", "parameters", "examples", "requestBodies", "headers", "securitySchemes", "links", "callbacks"];
pub(crate) const SCHEMA_KEYS: &[&str] = &[
    "$ref", "title", "multipleOf", "maximum", "format", "exclusiveMaximum", "minimum", "exclusiveMinimum", "maxLength", "minLength", "pattern", "maxItems", "minItems",
    "uniqueItems", "maxProperties", "minProperties", "required", "enum", "type", "allOf", "oneOf", "anyOf", "not", "items", "properties", "additionalProperties",
    "description", "default", "nullable", "discriminator", "readOnly", "writeOnly", "xml", "externalDocs", "example", "deprecated",
];
const EXAMPLE_KEYS: &[&str] = &["$ref", "summary", "description", "value", "externalValue"];
const HEADER_KEYS: &[&str] =
    &["$ref", "name", "in", "description", "required", "deprecated", "allowEmptyValue", "style", "explode", "allowReserved", "schema", "example", "examples", "content"];
const LINK_KEYS: &[&str] = &["$ref", "operationRef", "operationId", "parameters", "requestBody", "description", "server"];
const MEDIATYPE_KEYS: &[&str] = &["schema", "example", "examples", "encoding"];
const XML_KEYS: &[&str] = &["name", "namespace", "prefix", "attribute", "wrapped"];
const OAUTHFLOW_KEYS: &[&str] = &["authorizationUrl", "tokenUrl", "refreshUrl", "scopes"];
const OAUTHFLOWS_KEYS: &[&str] = &["implicit", "password", "clientCredentials", "authorizationCode"];
const ENCODING_KEYS: &[&str] = &["contentType", "headers", "style", "explode", "allowReserved"];

const ROOT_KEYS_31: &[&str] = &["openapi", "info", "servers", "paths", "components", "security", "tags", "externalDocs", "webhooks", "jsonSchemaDialect"];
const RESERVED_KEYWORDS_31: &[&str] = &["x-oai-", "x-oas-"];
const INFO_KEYS_31: &[&str] = &["title", "summary", "description", "termsOfService", "contact", "license", "version"];
const LICENSE_KEYS_31: &[&str] = &["name", "url", "identifier"];
const COMPONENTS_KEYS_31: &[&str] =
    &["schemas", "responses", "pathItems", "parameters", "examples", "requestBodies", "headers", "securitySchemes", "links", "callbacks"];
pub(crate) const SCHEMA_KEYS_31: &[&str] = &[
    "$ref", "title", "multipleOf", "maximum", "format", "exclusiveMaximum", "minimum", "exclusiveMinimum", "maxLength", "minLength", "pattern", "maxItems", "minItems",
    "uniqueItems", "maxProperties", "minProperties", "required", "enum", "type", "allOf", "oneOf", "anyOf", "not", "items", "properties", "additionalProperties",
    "description", "default", "discriminator", "readOnly", "writeOnly", "xml", "externalDocs", "example", "deprecated", "const", "examples", "$id", "$comment",
    "if", "then", "else", "unevaluatedProperties", "unevaluatedItems", "prefixItems", "contains", "contentEncoding", "contentMediaType", "$anchor", "$schema",
    "contentSchema", "propertyNames", "dependentSchemas", "dependentRequired", "minContains", "maxContains", "patternProperties", "$vocabulary", "$dynamicAnchor",
    "$dynamicRef",
];

/// `KEYS.get(openapi31 ? "openapi31" : "openapi30").get(name)`
pub(crate) fn spec_keys(openapi31: bool, name: &str) -> &'static [&'static str] {
    match (openapi31, name) {
        (false, "ROOT_KEYS") => ROOT_KEYS,
        (true, "ROOT_KEYS") => ROOT_KEYS_31,
        (false, "INFO_KEYS") => INFO_KEYS,
        (true, "INFO_KEYS") => INFO_KEYS_31,
        (_, "CONTACT_KEYS") => CONTACT_KEYS,
        (false, "LICENSE_KEYS") => LICENSE_KEYS,
        (true, "LICENSE_KEYS") => LICENSE_KEYS_31,
        (_, "TAG_KEYS") => TAG_KEYS,
        (_, "RESPONSE_KEYS") => RESPONSE_KEYS,
        (_, "SERVER_KEYS") => SERVER_KEYS,
        (_, "SERVER_VARIABLE_KEYS") => SERVER_VARIABLE_KEYS,
        (_, "PATHITEM_KEYS") => PATHITEM_KEYS,
        (_, "OPERATION_KEYS") => OPERATION_KEYS,
        (_, "PARAMETER_KEYS") => PARAMETER_KEYS,
        (_, "REQUEST_BODY_KEYS") => REQUEST_BODY_KEYS,
        (_, "SECURITY_SCHEME_KEYS") => SECURITY_SCHEME_KEYS,
        (_, "EXTERNAL_DOCS_KEYS") => EXTERNAL_DOCS_KEYS,
        (false, "COMPONENTS_KEYS") => COMPONENTS_KEYS,
        (true, "COMPONENTS_KEYS") => COMPONENTS_KEYS_31,
        (false, "SCHEMA_KEYS") => SCHEMA_KEYS,
        (true, "SCHEMA_KEYS") => SCHEMA_KEYS_31,
        (_, "EXAMPLE_KEYS") => EXAMPLE_KEYS,
        (_, "HEADER_KEYS") => HEADER_KEYS,
        (_, "LINK_KEYS") => LINK_KEYS,
        (_, "MEDIATYPE_KEYS") => MEDIATYPE_KEYS,
        (_, "XML_KEYS") => XML_KEYS,
        (_, "OAUTHFLOW_KEYS") => OAUTHFLOW_KEYS,
        (_, "OAUTHFLOWS_KEYS") => OAUTHFLOWS_KEYS,
        (_, "ENCODING_KEYS") => ENCODING_KEYS,
        (true, "RESERVED_KEYWORDS") => RESERVED_KEYWORDS_31,
        _ => &[],
    }
}

const QUERY_PARAMETER: &str = "query";
const COOKIE_PARAMETER: &str = "cookie";
const PATH_PARAMETER: &str = "path";
const HEADER_PARAMETER: &str = "header";
const REFERENCE_SEPARATOR: &str = "#/";

/// `JsonNode.getNodeType()`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeType {
    /// `JsonNodeType.ARRAY`.
    Array,
    /// `JsonNodeType.BINARY`.
    Binary,
    /// `JsonNodeType.BOOLEAN`.
    Boolean,
    /// `JsonNodeType.MISSING`.
    Missing,
    /// `JsonNodeType.NULL`.
    Null,
    /// `JsonNodeType.NUMBER`: every numeric node kind.
    Number,
    /// `JsonNodeType.OBJECT`.
    Object,
    /// `JsonNodeType.POJO`.
    Pojo,
    /// `JsonNodeType.STRING`: a text node.
    String,
}

/// `node.getNodeType()`: the Jackson node type of `n`.
pub fn node_type(n: &JsonNode) -> NodeType {
    match n {
        JsonNode::Object(_) => NodeType::Object,
        JsonNode::Array(_) => NodeType::Array,
        JsonNode::Text(_) => NodeType::String,
        JsonNode::Binary(_) => NodeType::Binary,
        JsonNode::Boolean(_) => NodeType::Boolean,
        JsonNode::Null => NodeType::Null,
        JsonNode::Missing => NodeType::Missing,
        JsonNode::Pojo(_) => NodeType::Pojo,
        _ => NodeType::Number,
    }
}

/// Jackson node class name (for `ClassCastException` texts).
pub fn node_class_name(n: &JsonNode) -> &'static str {
    match n {
        JsonNode::Object(_) => "com.fasterxml.jackson.databind.node.ObjectNode",
        JsonNode::Array(_) => "com.fasterxml.jackson.databind.node.ArrayNode",
        JsonNode::Text(_) => "com.fasterxml.jackson.databind.node.TextNode",
        JsonNode::Binary(_) => "com.fasterxml.jackson.databind.node.BinaryNode",
        JsonNode::Boolean(_) => "com.fasterxml.jackson.databind.node.BooleanNode",
        JsonNode::Null => "com.fasterxml.jackson.databind.node.NullNode",
        JsonNode::Missing => "com.fasterxml.jackson.databind.node.MissingNode",
        JsonNode::Short(_) => "com.fasterxml.jackson.databind.node.ShortNode",
        JsonNode::Int(_) => "com.fasterxml.jackson.databind.node.IntNode",
        JsonNode::Long(_) => "com.fasterxml.jackson.databind.node.LongNode",
        JsonNode::BigInteger(_) => "com.fasterxml.jackson.databind.node.BigIntegerNode",
        JsonNode::Float(_) => "com.fasterxml.jackson.databind.node.FloatNode",
        JsonNode::Double(_) => "com.fasterxml.jackson.databind.node.DoubleNode",
        JsonNode::Decimal(_) => "com.fasterxml.jackson.databind.node.DecimalNode",
        JsonNode::Pojo(_) => "com.fasterxml.jackson.databind.node.POJONode",
    }
}

/// `(ObjectNode) node`
pub fn cast_object(n: &JsonNode) -> JResult<&JsonNode> {
    if n.is_object() { Ok(n) } else { Err(JavaException::class_cast(node_class_name(n), "com.fasterxml.jackson.databind.node.ObjectNode")) }
}

/// `StringUtils.isBlank`
pub fn is_blank(s: Option<&str>) -> bool {
    s.is_none_or(|s| s.chars().all(char::is_whitespace))
}

fn matches_component_name(key: &str) -> bool {
    !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
}

/// `Location`
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Location {
    location: String,
    key: String,
}

/// `OpenAPIDeserializer.ParseResult`
#[derive(Clone, Debug)]
pub struct ParseResult {
    valid: bool,
    extra: IndexMap<Location, JsonNode>,
    unsupported: IndexMap<Location, JsonNode>,
    invalid_type: IndexMap<Location, String>,
    missing: Vec<Location>,
    warnings: Vec<Location>,
    unique: Vec<Location>,
    unique_tags: Vec<Location>,
    reserved: Vec<Location>,
    /// `ParseOptions.isAllowEmptyString()`: when `false`, blank strings count as absent. Default `true`.
    pub allow_empty_strings: bool,
    /// `ParseOptions.isValidateInternalRefs()`: check that local `#/components/...` references resolve. Default `false`.
    pub validate_internal_refs: bool,
    /// `ParseOptions.isInferSchemaType()`: infer a schema's `type` from its keywords when it has none. Default `true`.
    pub infer_schema_type: bool,
    /// Whether the document declares OpenAPI 3.1 (`openapi: 3.1.x`); set while parsing the root.
    pub openapi31: bool,
    /// `ParseOptions.isOaiAuthor()`, copied from the options. Default `false`.
    pub oai_author: bool,
    /// `ParseOptions.isExplicitStyleAndExplode()`: fill in the default `style`/`explode` of a parameter that omits them. Default `true`.
    pub explicit_style_and_explode: bool,
}

impl Default for ParseResult {
    fn default() -> Self {
        ParseResult {
            valid: true,
            extra: IndexMap::new(),
            unsupported: IndexMap::new(),
            invalid_type: IndexMap::new(),
            missing: Vec::new(),
            warnings: Vec::new(),
            unique: Vec::new(),
            unique_tags: Vec::new(),
            reserved: Vec::new(),
            allow_empty_strings: true,
            validate_internal_refs: false,
            infer_schema_type: true,
            openapi31: false,
            oai_author: false,
            explicit_style_and_explode: true,
        }
    }
}

fn loc(location: &str, key: &str) -> Location {
    Location { location: location.to_string(), key: key.to_string() }
}

impl ParseResult {
    /// `new ParseResult()`: valid, no messages, default option flags.
    pub fn new() -> ParseResult {
        ParseResult::default()
    }

    /// `ParseResult.openapi31(boolean)`: sets the `openapi31` field and returns the result for chaining.
    pub fn openapi31(mut self, v: bool) -> ParseResult {
        self.openapi31 = v;
        self
    }

    /// `unsupported(location, key, value)`: records an "is unsupported" message for `location.key`.
    pub fn unsupported(&mut self, location: &str, key: &str, value: &JsonNode) {
        self.unsupported.insert(loc(location, key), value.clone());
    }
    /// `reserved(location, key)`: records an "is reserved by The OpenAPI Initiative" message for `location.key`.
    pub fn reserved(&mut self, location: &str, key: &str) {
        self.reserved.push(loc(location, key));
    }
    /// `extra(location, key, value)`: records an "is unexpected" message for `location.key`.
    pub fn extra(&mut self, location: &str, key: &str, value: Option<&JsonNode>) {
        self.extra.insert(loc(location, key), value.cloned().unwrap_or(JsonNode::Null));
    }
    /// `missing(location, key)`: records an "is missing" message for `location.key`.
    pub fn missing(&mut self, location: &str, key: &str) {
        self.missing.push(loc(location, key));
    }
    /// `warning(location, key)`: records `key` itself as a message, prefixed by `location.`.
    pub fn warning(&mut self, location: &str, key: &str) {
        self.warnings.push(loc(location, key));
    }
    /// `unique(location, key)`: records an "is repeated" message for `location.key`.
    pub fn unique(&mut self, location: &str, key: &str) {
        self.unique.push(loc(location, key));
    }
    /// `uniqueTags(location, key)`: records an "is repeated" message for a duplicate tag at `location.key`.
    pub fn unique_tags(&mut self, location: &str, key: &str) {
        self.unique_tags.push(loc(location, key));
    }
    /// `invalidType(location, key, expectedType, value)`: records an "is not of type `expected`" message for `location.key`.
    pub fn invalid_type(&mut self, location: &str, key: &str, expected: &str, _value: Option<&JsonNode>) {
        self.invalid_type.insert(loc(location, key), expected.to_string());
    }
    /// `invalid()`: marks the document invalid.
    pub fn invalid(&mut self) {
        self.valid = false;
    }
    /// `isValid()`: `false` once [`ParseResult::invalid`] has been called.
    pub fn is_valid(&self) -> bool {
        self.valid
    }

    /// `getMessages()`
    pub fn get_messages(&self) -> Vec<String> {
        let prefix = |l: &Location| if l.location.is_empty() { String::new() } else { format!("{}.", l.location) };
        let mut m = Vec::new();
        for l in self.extra.keys() {
            m.push(format!("attribute {}{} is unexpected", prefix(l), l.key));
        }
        for (l, t) in &self.invalid_type {
            m.push(format!("attribute {}{} is not of type `{}`", prefix(l), l.key, t));
        }
        for l in &self.missing {
            m.push(format!("attribute {}{} is missing", prefix(l), l.key));
        }
        for l in &self.warnings {
            m.push(format!("{}{}", prefix(l), l.key));
        }
        for l in self.unsupported.keys() {
            m.push(format!("attribute {}{} is unsupported", prefix(l), l.key));
        }
        for l in &self.unique {
            m.push(format!("attribute {}{} is repeated", prefix(l), l.key));
        }
        for l in &self.unique_tags {
            m.push(format!("attribute {}{} is repeated", prefix(l), l.key));
        }
        for l in &self.reserved {
            m.push(format!("attribute {}{} is reserved by The OpenAPI Initiative", prefix(l), l.key));
        }
        m
    }

    fn allow(&self, value: &Option<String>) -> bool {
        (self.allow_empty_strings && value.is_some()) || (!self.allow_empty_strings && !is_blank(value.as_deref()))
    }
}

/// `OpenAPIDeserializer` (one instance per document, like Java).
#[derive(Default)]
pub struct OpenAPIDeserializer {
    components: Option<Components>,
    base_path: Option<String>,
    operation_ids: HashSet<String>,
    /// `localSchemaRefs` (a `HashMap<String,String>`: iterated in Java hash order)
    local_schema_refs: JMap<String>,
}

impl OpenAPIDeserializer {
    /// `new OpenAPIDeserializer()`: a fresh deserializer for one document.
    pub fn new() -> OpenAPIDeserializer {
        OpenAPIDeserializer { local_schema_refs: JMap::new_hash(), ..Default::default() }
    }

    /// `deserialize(rootNode)`
    pub fn deserialize(&mut self, root: &JsonNode) -> SwaggerParseResult {
        self.deserialize_with(root, None, &ParseOptions::default(), false)
    }

    /// `deserialize(rootNode, path)`
    pub fn deserialize_path(&mut self, root: &JsonNode, path: Option<&str>) -> SwaggerParseResult {
        self.deserialize_with(root, path, &ParseOptions::default(), false)
    }

    /// `deserialize(rootNode, path, options)` — note Java passes a fresh `ParseOptions` here (options ignored).
    pub fn deserialize_options(&mut self, root: &JsonNode, path: Option<&str>, _options: &ParseOptions) -> SwaggerParseResult {
        self.deserialize_with(root, path, &ParseOptions::default(), false)
    }

    /// `deserialize(rootNode, path, options)` as used by `parseJsonNode(path, node)`, surfacing the root-map error.
    pub fn try_deserialize_options(&mut self, root: &JsonNode, path: Option<&str>) -> JResult<SwaggerParseResult> {
        self.try_deserialize_with(root, path, &ParseOptions::default(), false)
    }

    /// `deserialize(rootNode, path, options, isOaiAuthor)`. Java throws (unchecked) when the root is not an
    /// object/null (the `convertValue(rootNode, Map.class)` step); that case panics here — use
    /// [`OpenAPIDeserializer::try_deserialize_with`] to observe it as an error.
    pub fn deserialize_with(&mut self, root: &JsonNode, path: Option<&str>, options: &ParseOptions, is_oai_author: bool) -> SwaggerParseResult {
        match self.try_deserialize_with(root, path, options, is_oai_author) {
            Ok(r) => r,
            Err(e) => panic!("{}", e.to_java_string()),
        }
    }

    /// `deserialize(...)` with the exception thrown by the root-map conversion surfaced as `Err`.
    pub fn try_deserialize_with(&mut self, root: &JsonNode, path: Option<&str>, options: &ParseOptions, _is_oai_author: bool) -> JResult<SwaggerParseResult> {
        self.base_path = path.map(str::to_string);
        root_map_conversion(root)?;
        let mut result = SwaggerParseResult::new();
        let mut root_parse = ParseResult::new();
        root_parse.oai_author = options.oai_author;
        root_parse.infer_schema_type = options.infer_schema_type;
        root_parse.allow_empty_strings = options.allow_empty_strings;
        root_parse.validate_internal_refs = options.validate_internal_refs;
        root_parse.explicit_style_and_explode = options.explicit_style_and_explode;
        match self.parse_root(root, &mut root_parse, path) {
            Ok(api) => {
                result.openapi31 = root_parse.openapi31;
                result.openapi = api;
                result.messages = Some(root_parse.get_messages());
            }
            Err(e) => {
                let msg = e.get_message().filter(|m| !is_blank(Some(m))).map(str::to_string).unwrap_or_else(|| "Unexpected error deserialising spec".to_string());
                result.messages = Some(vec![msg]);
            }
        }
        Ok(result)
    }

    /// `parseRoot(node, result, path)`: builds the `OpenAPI` from the root object, `None` when the root is not an object or `openapi` is missing or not 3.0/3.1. Messages go to `result`.
    pub fn parse_root(&mut self, node: &JsonNode, result: &mut ParseResult, path: Option<&str>) -> JResult<Option<OpenAPI>> {
        let location = "";
        let open_api = OpenAPI::new();
        if !node.is_object() {
            result.invalid_type(location, "openapi", "object", Some(node));
            result.invalid();
            return Ok(None);
        }
        let root = node;
        let value = self.get_string("openapi", root, true, location, result);
        match &value {
            None => return Ok(None),
            Some(v) if !v.starts_with("3.0") && !v.starts_with("3.1") => return Ok(None),
            Some(v) if v.starts_with("3.1") => {
                result.openapi31 = true;
                open_api.set_spec_version(SpecVersion::V31);
            }
            _ => {}
        }
        let v = value.unwrap();
        if !v.starts_with("3.0.") && !v.starts_with("3.1.") {
            result.warning(location, "The provided definition does not specify a valid version field");
        }
        open_api.set_openapi(v);

        if let Some(obj) = self.get_object("info", root, true, location, result) {
            let info = self.get_info(Some(&obj), "info", result)?;
            open_api.set_info(info);
        }

        if let Some(obj) = self.get_object("components", root, false, location, result) {
            let components = self.get_components(Some(&obj), "components", result)?.unwrap();
            open_api.set_components(components.clone());
            self.components = Some(components.clone());
            if result.validate_internal_refs {
                for (schema, schema_location) in self.local_schema_refs.entries() {
                    match components.get_schemas() {
                        None => result.missing(&schema_location, &schema),
                        Some(s) => {
                            if s.get(&schema).is_none() {
                                result.invalid_type(&schema_location, &schema, "schema", Some(root));
                            }
                        }
                    }
                }
            }
        }

        let paths_required = !result.openapi31;
        if let Some(obj) = self.get_object("paths", root, paths_required, location, result) {
            let paths = self.get_paths(&obj, "paths", result)?;
            open_api.set_paths(paths);
        }

        let array = self.get_array("servers", root, false, location, result);
        match array {
            Some(a) if a.size() > 0 => {
                open_api.set_servers(self.get_servers_list(Some(&a), &format!("{location}.servers"), result, path)?);
            }
            _ => {
                let default_server = Server::new();
                default_server.set_url("/");
                open_api.set_servers(JList::from_vec(vec![default_server]));
            }
        }

        if let Some(obj) = self.get_object("externalDocs", root, false, location, result) {
            let d = self.get_external_docs(Some(&obj), "externalDocs", result);
            open_api.set_external_docs(d);
        }

        if let Some(a) = self.get_array("tags", root, false, location, result) && a.size() > 0 {
            open_api.set_tags(self.get_tag_list(Some(&a), "tags", result)?);
        }

        if let Some(a) = self.get_array("security", root, false, location, result) && a.size() > 0 {
            let reqs = self.get_security_requirements_list(Some(&a), "security", result);
            if let Some(r) = reqs && r.size() > 0 {
                open_api.set_security(r);
            }
        }

        if result.openapi31 && let Some(obj) = self.get_object("webhooks", root, false, location, result) {
            let webhooks = self.get_webhooks(&obj, "webhooks", result)?;
            open_api.set_webhooks(webhooks);
        }

        let extensions = self.get_extensions(root)?;
        if !extensions.is_empty() {
            open_api.set_extensions(extensions);
        }

        if result.openapi31 && let Some(v) = self.get_string("jsonSchemaDialect", root, false, location, result) {
            if is_valid_uri(&v) {
                open_api.set_json_schema_dialect(v);
            } else {
                result.warning(location, &format!("jsonSchemaDialect. Invalid url: {v}"));
            }
        }

        if result.openapi31 && open_api.get_components().is_none() && open_api.get_paths().is_none() && open_api.get_webhooks().is_none() {
            result.warning(location, "The OpenAPI document MUST contain at least one paths field, a components field or a webhooks field");
        }

        for key in get_keys(Some(root)) {
            if !spec_keys(result.openapi31, "ROOT_KEYS").contains(&key.as_str()) && !key.starts_with("x-") {
                result.extra(location, &key, node.get(&key));
            }
            validate_reserved_keywords(&key, location, result);
        }
        Ok(Some(open_api))
    }

    /// `mungedRef`
    pub fn munged_ref(&self, s: &str) -> Option<String> {
        if !s.contains(':') && !s.starts_with('#') && !s.starts_with('/') && s.find('.').is_some_and(|i| i > 0) {
            return Some(format!("./{s}"));
        }
        None
    }

    /// `getExtensions(node)`: direct `x-` keys, or else those under an `extensions` object.
    pub fn get_extensions(&self, node: &JsonNode) -> JResult<JMap<JavaObject>> {
        let ext = try_direct_extensions(node);
        if ext.is_empty() {
            return try_unwrap_lookup_extensions(node);
        }
        Ok(ext)
    }

    /// `getComponents(obj, location, result)`: the `components` object, `None` when `obj` is `None`.
    pub fn get_components(&mut self, obj: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> JResult<Option<Components>> {
        let Some(obj) = obj else { return Ok(None) };
        let components = Components::new();
        if let Some(node) = self.get_object("schemas", obj, false, location, result) {
            components.set_schemas(self.get_schemas(Some(&node), &format!("{location}.schemas"), result, true)?);
        }
        if let Some(node) = self.get_object("responses", obj, false, location, result) {
            components.set_responses(self.get_responses_map(Some(&node), &format!("{location}.responses"), result, true)?);
        }
        if result.openapi31 && let Some(node) = self.get_object("pathItems", obj, false, location, result) {
            components.set_path_items(self.get_path_items_map(Some(&node), &format!("{location}.pathItems"), result, true)?);
        }
        if let Some(node) = self.get_object("parameters", obj, false, location, result) {
            components.set_parameters(self.get_parameters(Some(&node), &format!("{location}.parameters"), result, true)?);
        }
        if let Some(node) = self.get_object("examples", obj, false, location, result) {
            components.set_examples(self.get_examples(Some(&node), &format!("{location}.examples"), result, true)?);
        }
        if let Some(node) = self.get_object("requestBodies", obj, false, location, result) {
            components.set_request_bodies(self.get_request_bodies(Some(&node), &format!("{location}.requestBodies"), result, true)?);
        }
        if let Some(node) = self.get_object("headers", obj, false, location, result) {
            components.set_headers(self.get_headers(Some(&node), &format!("{location}.headers"), result, true)?);
        }
        if let Some(node) = self.get_object("securitySchemes", obj, false, location, result) {
            components.set_security_schemes(self.get_security_schemes(Some(&node), &format!("{location}.securitySchemes"), result, true)?);
        }
        if let Some(node) = self.get_object("links", obj, false, location, result) {
            components.set_links(self.get_links(Some(&node), &format!("{location}.links"), result, true)?);
        }
        if let Some(node) = self.get_object("callbacks", obj, false, location, result) {
            components.set_callbacks(self.get_callbacks(Some(&node), &format!("{location}.callbacks"), result, true)?);
        }
        components.set_extensions(JMap::new());
        let extensions = self.get_extensions(obj)?;
        if !extensions.is_empty() {
            components.set_extensions(extensions);
        }
        for key in get_keys(Some(obj)) {
            if !spec_keys(result.openapi31, "COMPONENTS_KEYS").contains(&key.as_str()) && !key.starts_with("x-") {
                result.extra(location, &key, obj.get(&key));
            }
            validate_reserved_keywords(&key, location, result);
        }
        Ok(Some(components))
    }

    /// `getTagList(obj, location, result)`: the `tags` array; duplicate names are reported as repeated.
    pub fn get_tag_list(&mut self, obj: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> JResult<JList<Tag>> {
        let tags = JList::new();
        let Some(obj) = obj else { return Ok(tags) };
        let mut tracker: HashSet<Option<String>> = HashSet::new();
        for item in obj.as_array().unwrap().iter() {
            if item.is_object() && let Some(tag) = self.get_tag(Some(item), location, result)? {
                tags.add(tag.clone());
                let name = tag.get_name();
                if tracker.contains(&name) {
                    result.unique_tags(location, name.as_deref().unwrap_or("null"));
                }
                tracker.insert(name);
            }
        }
        Ok(tags)
    }

    /// `getTag(obj, location, result)`: one tag object, `None` when `obj` is `None`.
    pub fn get_tag(&mut self, obj: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> JResult<Option<Tag>> {
        let Some(obj) = obj else { return Ok(None) };
        let tag = Tag::new();
        let value = self.get_string("name", obj, true, location, result);
        if result.allow(&value) {
            tag.set_name(value);
        }
        let value = self.get_string("description", obj, false, location, result);
        if result.allow(&value) {
            tag.set_description(value);
        }
        let docs = self.get_object("externalDocs", obj, false, location, result);
        let external_docs = self.get_external_docs(docs.as_ref(), &format!("{location}.externalDocs"), result);
        if external_docs.is_some() {
            tag.set_external_docs(external_docs);
        }
        let extensions = self.get_extensions(obj)?;
        if !extensions.is_empty() {
            tag.set_extensions(extensions);
        }
        for key in get_keys(Some(obj)) {
            if !spec_keys(result.openapi31, "TAG_KEYS").contains(&key.as_str()) && !key.starts_with("x-") {
                result.extra(location, &key, obj.get(&key));
            }
            validate_reserved_keywords(&key, location, result);
        }
        Ok(Some(tag))
    }

    /// `getServersList(obj, location, result, path)`: the `servers` array; `path` (the document location) resolves relative server URLs.
    pub fn get_servers_list(&mut self, obj: Option<&JsonNode>, location: &str, result: &mut ParseResult, path: Option<&str>) -> JResult<JList<Server>> {
        let servers = JList::new();
        let Some(obj) = obj else { return Ok(servers) };
        for item in obj.as_array().unwrap().iter() {
            if item.is_object() {
                match self.get_server(Some(item), location, result, path)? {
                    Some(s) => servers.add(s),
                    None => {
                        let d = Server::new();
                        d.set_url("/");
                        servers.add(d);
                    }
                }
            }
        }
        Ok(servers)
    }

    /// `getServer(obj, location, result, path)`: one server object, `None` when `obj` is `None`.
    pub fn get_server(&mut self, obj: Option<&JsonNode>, location: &str, result: &mut ParseResult, path: Option<&str>) -> JResult<Option<Server>> {
        let Some(obj) = obj else { return Ok(None) };
        let server = Server::new();
        if obj.get("variables").is_some() {
            let variables = self.get_object("variables", obj, false, location, result);
            let sv = self.get_server_variables(variables.as_ref(), &format!("{location}.variables"), result)?;
            if let Some(sv) = sv && sv.size() > 0 {
                server.set_variables(sv);
            }
        }
        let value = self.get_string("url", obj, true, location, result);
        if result.allow(&value) {
            let mut value = value.unwrap();
            if !is_valid_url(&value) && let Some(path) = path {
                let base = path.replace('\\', "/");
                match JavaUri::parse(&base) {
                    Ok(abs) => {
                        if matches!(abs.get_scheme(), Some("http") | Some("https")) {
                            match JavaUri::parse(&value) {
                                Ok(child) => value = abs.resolve(&child).to_java_string(),
                                Err(_) => self.server_url_syntax_error(&value, &server, location, result),
                            }
                        }
                    }
                    Err(_) => self.server_url_syntax_error(&value, &server, location, result),
                }
            }
            server.set_url(value);
        }
        let value = self.get_string("description", obj, false, location, result);
        if result.allow(&value) {
            server.set_description(value);
        }
        let extensions = self.get_extensions(obj)?;
        if !extensions.is_empty() {
            server.set_extensions(extensions);
        }
        for key in get_keys(Some(obj)) {
            if !spec_keys(result.openapi31, "SERVER_KEYS").contains(&key.as_str()) && !key.starts_with("x-") {
                result.extra(location, &key, obj.get(&key));
            }
            validate_reserved_keywords(&key, location, result);
        }
        Ok(Some(server))
    }

    fn server_url_syntax_error(&self, value: &str, server: &Server, location: &str, result: &mut ParseResult) {
        let open = value.find('{');
        let close = value.find('}');
        if let (Some(o), Some(c)) = (open, close) {
            let variable = value.get(o + 1..c).unwrap_or("");
            if let Some(vars) = server.get_variables() && !vars.contains_key(variable) {
                result.warning(location, &format!("invalid url : {value}"));
            }
        }
    }

    /// `getServerVariables(obj, location, result)`: the server's `variables` map, `None` when `obj` is `None`.
    pub fn get_server_variables(&mut self, obj: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> JResult<Option<ServerVariables>> {
        let server_variables = ServerVariables::new();
        let Some(obj) = obj else { return Ok(None) };
        for name in get_keys(Some(obj)) {
            let value = obj.get(&name).unwrap();
            let server = cast_object(value)?;
            let sv = self.get_server_variable(Some(server), &format!("{location}.{name}"), result)?;
            if let Some(sv) = sv {
                server_variables.add_server_variable(&name, sv);
            }
        }
        Ok(Some(server_variables))
    }

    /// `getServerVariable(obj, location, result)`: one server variable, `None` when `obj` is `None`.
    pub fn get_server_variable(&mut self, obj: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> JResult<Option<ServerVariable>> {
        let Some(obj) = obj else { return Ok(None) };
        let sv = ServerVariable::new();
        if let Some(arr) = self.get_array("enum", obj, false, location, result) {
            if arr.size() == 0 && result.openapi31 {
                result.warning(location, "enum array MUST NOT be empty");
            } else {
                let list: JList<String> = JList::new();
                for n in arr.as_array().unwrap().iter() {
                    if n.is_value_node() {
                        list.add(n.as_text());
                        sv.set_enum(list.clone());
                    } else {
                        result.invalid_type(location, "enum", "value", Some(n));
                    }
                }
            }
        }
        let value = self.get_string("default", obj, true, &format!("{location}.default"), result);
        if result.allow(&value) {
            sv.set_default(value);
        }
        let value = self.get_string("description", obj, false, &format!("{location}.description"), result);
        if result.allow(&value) {
            sv.set_description(value);
        }
        let extensions = self.get_extensions(obj)?;
        if !extensions.is_empty() {
            sv.set_extensions(extensions);
        }
        for key in get_keys(Some(obj)) {
            if !spec_keys(result.openapi31, "SERVER_VARIABLE_KEYS").contains(&key.as_str()) && !key.starts_with("x-") {
                result.extra(location, &key, obj.get(&key));
            }
            validate_reserved_keywords(&key, location, result);
        }
        Ok(Some(sv))
    }

    /// `getPaths`
    pub fn get_paths(&mut self, obj: &JsonNode, location: &str, result: &mut ParseResult) -> JResult<Paths> {
        let paths = Paths::new();
        self.fill_path_items(Some(obj), location, result, PathsTarget::Paths(&paths), false)?;
        Ok(paths)
    }

    /// `getPathItems(node, location, result, underComponents)` (components.pathItems)
    pub fn get_path_items_map(&mut self, node: Option<&JsonNode>, location: &str, result: &mut ParseResult, under_components: bool) -> JResult<JMap<PathItem>> {
        let items = JMap::new();
        let Some(node) = node else { return Ok(items) };
        for key in get_keys(Some(node)) {
            if under_components && !matches_component_name(&key) {
                result.warning(location, &format!("PathItem key {key} doesn't adhere to regular expression ^[a-zA-Z0-9\\.\\-_]+$"));
            }
            let v = cast_object(node.get(&key).unwrap())?;
            if let Some(pi) = self.get_path_item(v, location, result)? {
                items.put(key, pi);
            }
        }
        Ok(items)
    }

    /// `getWebhooks`
    pub fn get_webhooks(&mut self, obj: &JsonNode, location: &str, result: &mut ParseResult) -> JResult<JMap<PathItem>> {
        let webhooks = JMap::new();
        self.fill_path_items(Some(obj), location, result, PathsTarget::Map(&webhooks), true)?;
        Ok(webhooks)
    }

    fn fill_path_items(&mut self, obj: Option<&JsonNode>, location: &str, result: &mut ParseResult, target: PathsTarget, is_webhook: bool) -> JResult<bool> {
        let Some(obj) = obj else { return Ok(false) };
        for path_name in get_keys(Some(obj)) {
            let path_value = obj.get(&path_name).unwrap();
            if matches!(target, PathsTarget::Paths(_)) && path_name.starts_with("x-") {
                let ext = self.get_extensions(obj)?;
                if !ext.is_empty() && let PathsTarget::Paths(p) = &target {
                    p.set_extensions(ext);
                }
                continue;
            }
            if !path_value.is_object() {
                result.invalid_type(location, &path_name, "object", Some(path_value));
                continue;
            }
            if !path_name.starts_with('/') && !is_webhook {
                result.warning(location, &format!(" Resource {path_name} should start with /"));
            }
            let path_obj = self.get_path_item(path_value, &format!("{location}.'{path_name}'"), result)?;
            // declared path parameters
            let parts = path_template_parts(&path_name);
            for part in parts {
                let path_param = &part[1..part.len() - 1];
                let po = path_obj.as_ref().ok_or_else(|| JavaException::npe("Cannot invoke \"io.swagger.v3.oas.models.PathItem.getParameters()\" because \"pathObj\" is null"))?;
                let defined_in_path_level = is_path_param_defined(path_param, po.get_parameters().map(|l| l.to_vec()).as_deref());
                if !defined_in_path_level {
                    for operation in get_all_operations_in_a_path(po) {
                        let params = operation.get_parameters().map(|l| l.to_vec()).unwrap_or_default();
                        for p in &params {
                            if p.get_in().is_some_and(|i| i.eq_ignore_ascii_case(PATH_PARAMETER)) && p.get_required() == Some(false) {
                                result.warning(location, &format!("For path parameter {} the required value should be true", p.get_name().as_deref().unwrap_or("null")));
                            }
                        }
                        if !is_path_param_defined(path_param, Some(&params)) {
                            result.warning(
                                &format!("{location}.'{path_name}'"),
                                &format!(" Declared path parameter {path_param} needs to be defined as a path parameter in path or operation level"),
                            );
                        }
                    }
                }
            }
            match &target {
                PathsTarget::Paths(p) => {
                    if let Some(pi) = path_obj {
                        p.put(path_name, pi);
                    }
                }
                PathsTarget::Map(m) => {
                    if let Some(pi) = path_obj {
                        m.put(path_name, pi);
                    }
                }
            }
        }
        Ok(true)
    }

    /// `getPathItem(obj, location, result)`: one path item with its operations and parameters.
    pub fn get_path_item(&mut self, obj: &JsonNode, location: &str, result: &mut ParseResult) -> JResult<Option<PathItem>> {
        let path_item = PathItem::new();
        if let Some(r) = obj.get("$ref") {
            if let Some(text) = r.text_value() {
                match self.munged_ref(text) {
                    Some(m) => path_item.set_ref(m),
                    None => path_item.set_ref(text),
                }
                if text.starts_with("#/components") && !text.starts_with("#/components/pathItems") {
                    result.warning(location, &format!("$ref target {text} is not of expected type PathItem"));
                }
                if result.openapi31 {
                    let v = self.get_string("summary", obj, false, location, result);
                    if !is_blank(v.as_deref()) {
                        path_item.set_summary(v);
                    }
                    let v = self.get_string("description", obj, false, location, result);
                    if !is_blank(v.as_deref()) {
                        path_item.set_description(v);
                    }
                }
                return Ok(Some(path_item));
            } else if r.is_object() {
                for key in get_keys(Some(r)) {
                    result.extra(location, &key, r.get(&key));
                }
            }
            return Ok(None);
        }
        let v = self.get_string("summary", obj, false, location, result);
        if result.allow(&v) {
            path_item.set_summary(v);
        }
        let v = self.get_string("description", obj, false, location, result);
        if result.allow(&v) {
            path_item.set_description(v);
        }
        if let Some(params) = self.get_array("parameters", obj, false, location, result) && params.size() > 0 {
            path_item.set_parameters(self.get_parameter_list(Some(&params), location, result)?);
        }
        if let Some(servers) = self.get_array("servers", obj, false, location, result) && servers.size() > 0 {
            path_item.set_servers(self.get_servers_list(Some(&servers), location, result, None)?);
        }
        for (method, setter) in [
            ("get", PathItem::set_get as fn(&PathItem, Option<Operation>)),
            ("put", PathItem::set_put_),
            ("post", PathItem::set_post_),
            ("head", PathItem::set_head_),
            ("delete", PathItem::set_delete_),
            ("patch", PathItem::set_patch_),
            ("options", PathItem::set_options_),
            ("trace", PathItem::set_trace_),
        ] {
            if let Some(node) = self.get_object(method, obj, false, location, result)
                && let Some(op) = self.get_operation(Some(&node), &format!("{location}({method})"), result)?
            {
                setter(&path_item, Some(op));
            }
        }
        let ext = self.get_extensions(obj)?;
        if !ext.is_empty() {
            path_item.set_extensions(ext);
        }
        for key in get_keys(Some(obj)) {
            if !spec_keys(result.openapi31, "PATHITEM_KEYS").contains(&key.as_str()) && !key.starts_with("x-") {
                result.extra(location, &key, obj.get(&key));
            }
            validate_reserved_keywords(&key, location, result);
        }
        Ok(Some(path_item))
    }

    /// `getExternalDocs(node, location, result)`: the `externalDocs` object, `None` when `node` is `None`.
    pub fn get_external_docs(&mut self, node: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> Option<ExternalDocumentation> {
        let node = node?;
        let docs = ExternalDocumentation::new();
        let v = self.get_string("description", node, false, location, result);
        if result.allow(&v) {
            docs.set_description(v);
        }
        let v = self.get_string("url", node, true, location, result);
        if result.allow(&v) {
            docs.set_url(v);
        }
        if let Ok(ext) = self.get_extensions(node) && !ext.is_empty() {
            docs.set_extensions(ext);
        }
        for key in get_keys(Some(node)) {
            if !spec_keys(result.openapi31, "EXTERNAL_DOCS_KEYS").contains(&key.as_str()) && !key.starts_with("x-") {
                result.extra(location, &key, node.get(&key));
            }
        }
        Some(docs)
    }

    /// `getString(key, node, required, location, result, uniqueValues, noInvalidError, missingForNullNode)`
    #[allow(clippy::too_many_arguments)]
    pub fn get_string_full(
        &mut self,
        key: &str,
        node: &JsonNode,
        required: bool,
        location: &str,
        result: &mut ParseResult,
        unique: Option<&mut HashSet<String>>,
        no_invalid_error: bool,
        missing_for_null_node: bool,
    ) -> Option<String> {
        let v = node.get(key);
        match v {
            None => {
                if required {
                    result.missing(location, key);
                    result.invalid();
                }
                None
            }
            Some(v) if v.is_null() && missing_for_null_node => {
                if required {
                    result.missing(location, key);
                    result.invalid();
                }
                None
            }
            Some(v) if !v.is_value_node() => {
                if !no_invalid_error {
                    result.invalid_type(location, key, "string", Some(node));
                }
                None
            }
            Some(v) if v.is_null() => None,
            Some(v) => {
                let value = v.as_text();
                if let Some(u) = unique && !u.insert(value.clone()) {
                    result.unique(location, "operationId");
                    result.invalid();
                }
                Some(value)
            }
        }
    }

    /// `getString(key, node, required, location, result)`: the text value of `key`; a missing required key or a non-text value is reported to `result` and gives `None`.
    pub fn get_string(&mut self, key: &str, node: &JsonNode, required: bool, location: &str, result: &mut ParseResult) -> Option<String> {
        self.get_string_full(key, node, required, location, result, None, false, false)
    }

    /// `getObject`
    pub fn get_object(&mut self, key: &str, node: &JsonNode, required: bool, location: &str, result: &mut ParseResult) -> Option<JsonNode> {
        match node.get(key) {
            None => {
                if required {
                    result.missing(location, key);
                    result.invalid();
                }
                None
            }
            Some(v) if !v.is_object() => {
                result.invalid_type(location, key, "object", Some(v));
                if required {
                    result.invalid();
                }
                None
            }
            Some(v) => Some(v.clone()),
        }
    }

    /// `getObjectOrBoolean`
    pub fn get_object_or_boolean(&mut self, key: &str, node: &JsonNode, required: bool, location: &str, result: &mut ParseResult) -> Option<JsonNode> {
        let value = match node.get(key) {
            None => {
                if required {
                    result.missing(location, key);
                    result.invalid();
                }
                return None;
            }
            Some(v) => v,
        };
        let mut is_bool = false;
        if value.is_boolean() {
            is_bool = true;
        } else if let Some(s) = value.text_value() {
            if s.eq_ignore_ascii_case("true") || s.eq_ignore_ascii_case("false") {
                is_bool = true;
            } else {
                result.invalid_type(location, key, "object", Some(value));
                return None;
            }
        }
        if is_bool {
            return Some(value.clone());
        }
        if !value.is_object() {
            result.invalid_type(location, key, "object", Some(value));
            return None;
        }
        Some(value.clone())
    }

    /// `getInfo(node, location, result)`: the `info` object, `None` when `node` is `None`.
    pub fn get_info(&mut self, node: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> JResult<Option<Info>> {
        let Some(node) = node else { return Ok(None) };
        let info = Info::new();
        let v = self.get_string("title", node, true, location, result);
        if result.allow(&v) {
            info.set_title(v);
        }
        let v = self.get_string("description", node, false, location, result);
        if result.allow(&v) {
            info.set_description(v);
        }
        if result.openapi31 {
            let v = self.get_string("summary", node, false, location, result);
            if !is_blank(v.as_deref()) {
                info.set_summary(v);
            }
        }
        let v = self.get_string("termsOfService", node, false, location, result);
        if result.allow(&v) {
            info.set_terms_of_service(v);
        }
        let obj = self.get_object("contact", node, false, "contact", result);
        let contact = self.get_contact(obj.as_ref(), &format!("{location}.contact"), result)?;
        if obj.is_some() {
            info.set_contact(contact);
        }
        let obj = self.get_object("license", node, false, location, result);
        let license = self.get_license(obj.as_ref(), &format!("{location}.license"), result)?;
        if obj.is_some() {
            info.set_license(license);
        }
        let v = self.get_string_full("version", node, true, location, result, None, false, true);
        if result.allow(&v) {
            info.set_version(v);
        }
        let ext = self.get_extensions(node)?;
        if !ext.is_empty() {
            info.set_extensions(ext);
        }
        for key in get_keys(Some(node)) {
            if !spec_keys(result.openapi31, "INFO_KEYS").contains(&key.as_str()) && !key.starts_with("x-") {
                result.extra(location, &key, node.get(&key));
            }
            validate_reserved_keywords(&key, location, result);
        }
        Ok(Some(info))
    }

    /// `getLicense(node, location, result)`: the `license` object, `None` when `node` is `None`.
    pub fn get_license(&mut self, node: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> JResult<Option<License>> {
        let Some(node) = node else { return Ok(None) };
        let license = License::new();
        let v = self.get_string("name", node, true, location, result);
        if result.allow(&v) {
            license.set_name(v);
        }
        let v = self.get_string("url", node, false, location, result);
        if result.allow(&v) {
            let u = v.clone().unwrap();
            if JavaUrl::parse(&u).is_err() {
                result.warning(location, &u);
            }
            license.set_url(v);
        }
        if result.openapi31 {
            let needs_identifier = license.get_url().is_none();
            let v = self.get_string("identifier", node, needs_identifier, location, result);
            if !is_blank(v.as_deref()) {
                if !needs_identifier {
                    result.extra(location, "identifier", Some(node));
                    result.invalid();
                } else {
                    license.set_identifier(v);
                }
            }
        }
        let ext = self.get_extensions(node)?;
        if !ext.is_empty() {
            license.set_extensions(ext);
        }
        for key in get_keys(Some(node)) {
            if !spec_keys(result.openapi31, "LICENSE_KEYS").contains(&key.as_str()) && !key.starts_with("x-") {
                result.extra(location, &key, node.get(&key));
            }
            validate_reserved_keywords(&key, location, result);
        }
        Ok(Some(license))
    }

    /// `getContact(node, location, result)`: the `contact` object, `None` when `node` is `None`.
    pub fn get_contact(&mut self, node: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> JResult<Option<Contact>> {
        let Some(node) = node else { return Ok(None) };
        let contact = Contact::new();
        let v = self.get_string("name", node, false, location, result);
        if result.allow(&v) {
            contact.set_name(v);
        }
        let v = self.get_string("url", node, false, location, result);
        if result.allow(&v) {
            let u = v.clone().unwrap();
            if JavaUrl::parse(&u).is_err() {
                result.warning(location, &u);
            }
            contact.set_url(v);
        }
        let v = self.get_string("email", node, false, location, result);
        if result.allow(&v) {
            contact.set_email(v);
        }
        let ext = self.get_extensions(node)?;
        if !ext.is_empty() {
            contact.set_extensions(ext);
        }
        for key in get_keys(Some(node)) {
            if !spec_keys(result.openapi31, "CONTACT_KEYS").contains(&key.as_str()) && !key.starts_with("x-") {
                result.extra(location, &key, node.get(&key));
            }
            validate_reserved_keywords(&key, location, result);
        }
        Ok(Some(contact))
    }

    /// `getContent(node, location, result)`: the `content` map of media types, `None` when `node` is `None`.
    pub fn get_content(&mut self, node: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> JResult<Option<Content>> {
        let Some(node) = node else { return Ok(None) };
        let content = Content::new();
        for key in get_keys(Some(node)) {
            let mt_node = cast_object(node.get(&key).unwrap())?;
            if let Some(mt) = self.get_media_type(Some(mt_node), &format!("{location}.'{key}'"), result)? {
                content.add_media_type(&key, mt);
            }
        }
        Ok(Some(content))
    }

    /// `getMediaType(contentNode, location, result)`: one media type object, `None` when the node is `None`.
    pub fn get_media_type(&mut self, content_node: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> JResult<Option<MediaType>> {
        let Some(cn) = content_node else { return Ok(None) };
        let media_type = MediaType::new();
        if let Some(so) = self.get_object("schema", cn, false, location, result) {
            media_type.set_schema(self.get_schema(Some(&so), &format!("{location}.schema"), Some(result))?);
        }
        if let Some(eo) = self.get_object("encoding", cn, false, location, result) {
            let encoding_location = format!("{location}.encoding");
            media_type.set_encoding(self.get_encoding_map(Some(&eo), &encoding_location, result)?);
            let props: Option<Vec<String>> = match media_type.get_schema() {
                None => Some(Vec::new()),
                Some(s) if s.get_ref().is_none() => Some(s.get_properties().map(|p| p.keys()).unwrap_or_default()),
                Some(_) => None,
            };
            if let Some(props) = props && let Some(enc) = media_type.get_encoding() {
                for ep in enc.keys() {
                    if !props.contains(&ep) {
                        result.extra(&encoding_location, &ep, Some(&eo));
                    }
                }
            }
        }
        let ext = self.get_extensions(cn)?;
        if !ext.is_empty() {
            media_type.set_extensions(ext);
        }
        let examples_object = self.get_object("examples", cn, false, location, result);
        if let Some(eo) = &examples_object {
            media_type.set_examples(self.get_examples(Some(eo), &format!("{location}.examples"), result, false)?);
        }
        if let Some(example) = self.get_any_type("example", cn, location, result) {
            if examples_object.is_some() {
                result.warning(location, "examples already defined -- ignoring \"example\" field");
            } else {
                media_type.set_example(if is_null_node(&example) { None } else { Some(example) });
            }
        }
        for key in get_keys(Some(cn)) {
            if !spec_keys(result.openapi31, "MEDIATYPE_KEYS").contains(&key.as_str()) && !key.starts_with("x-") {
                result.extra(location, &key, cn.get(&key));
            }
            validate_reserved_keywords(&key, location, result);
        }
        Ok(Some(media_type))
    }

    /// `getEncodingMap(node, location, result)`: a media type's `encoding` map, empty when `node` is `None`.
    pub fn get_encoding_map(&mut self, node: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> JResult<JMap<Encoding>> {
        let encodings = JMap::new();
        let Some(node) = node else { return Ok(encodings) };
        for key in get_keys(Some(node)) {
            let n = cast_object(node.get(&key).unwrap())?;
            if let Some(e) = self.get_encoding(Some(n), location, result)? {
                encodings.put(key, e);
            }
        }
        Ok(encodings)
    }

    /// `getEncoding(node, location, result)`: one encoding object, `None` when `node` is `None`.
    pub fn get_encoding(&mut self, node: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> JResult<Option<Encoding>> {
        let Some(node) = node else { return Ok(None) };
        let encoding = Encoding::new();
        let v = self.get_string("contentType", node, false, location, result);
        if result.allow(&v) {
            encoding.set_content_type(v);
        }
        let v = self.get_string("style", node, false, location, result);
        if is_blank(v.as_deref()) {
            if result.explicit_style_and_explode {
                encoding.set_style(EncodingStyle::Form);
            }
        } else {
            match EncodingStyle::from_str_value(v.as_deref().unwrap()) {
                Some(s) => encoding.set_style(s),
                None => result.invalid_type(location, "style", "string", Some(node)),
            }
        }
        if let Some(b) = get_boolean("explode", node, false, location, result) {
            encoding.set_explode(b);
        }
        if let Some(b) = get_boolean("allowReserved", node, false, location, result) {
            encoding.set_allow_reserved(b);
        }
        if let Some(h) = self.get_object("headers", node, false, location, result) {
            encoding.set_headers(self.get_headers(Some(&h), location, result, false)?);
        }
        let ext = self.get_extensions(node)?;
        if !ext.is_empty() {
            encoding.set_extensions(ext);
        }
        for key in get_keys(Some(node)) {
            if !spec_keys(result.openapi31, "ENCODING_KEYS").contains(&key.as_str()) && !key.starts_with("x-") {
                result.extra(location, &key, node.get(&key));
            }
            validate_reserved_keywords(&key, location, result);
        }
        Ok(Some(encoding))
    }

    /// `getLinks(obj, location, result, underComponents)`: a `links` map; under `components` a name outside `^[a-zA-Z0-9\.\-_]+$` adds a warning.
    pub fn get_links(&mut self, obj: Option<&JsonNode>, location: &str, result: &mut ParseResult, under_components: bool) -> JResult<JMap<Link>> {
        let links = JMap::new();
        let Some(obj) = obj else { return Ok(links) };
        for name in get_keys(Some(obj)) {
            if under_components && !matches_component_name(&name) {
                result.warning(location, &format!("Link name {name} doesn't adhere to regular expression ^[a-zA-Z0-9\\.\\-_]+$"));
            }
            let v = obj.get(&name).unwrap();
            if !v.is_object() {
                result.invalid_type(location, &name, "object", Some(v));
            } else if let Some(l) = self.get_link(Some(v), &format!("{location}.{name}"), result)? {
                links.put(name, l);
            }
        }
        Ok(links)
    }

    /// `getLink(node, location, result)`: one link object, `None` when `node` is `None`.
    pub fn get_link(&mut self, node: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> JResult<Option<Link>> {
        let Some(node) = node else { return Ok(None) };
        let link = Link::new();
        if let Some(r) = node.get("$ref") {
            if let Some(text) = r.text_value() {
                match self.munged_ref(text) {
                    Some(m) => link.set_ref(m),
                    None => link.set_ref(text),
                }
                if text.starts_with("#/components") && !text.starts_with("#/components/links") {
                    result.warning(location, &format!("$ref target {text} is not of expected type Link"));
                }
                if result.openapi31 {
                    let d = self.get_string("description", node, false, location, result);
                    if !is_blank(d.as_deref()) {
                        link.set_description(d);
                    }
                }
                return Ok(Some(link));
            }
            result.invalid_type(location, "$ref", "string", Some(node));
            return Ok(None);
        }
        let v = self.get_string("operationRef", node, false, location, result);
        if result.allow(&v) {
            link.set_operation_ref(v);
        }
        let v = self.get_string("operationId", node, false, location, result);
        if result.allow(&v) {
            link.set_operation_id(v);
        }
        if let Some(po) = self.get_object("parameters", node, false, location, result) {
            let params: JMap<String> = JMap::new();
            for name in get_keys(Some(&po)) {
                params.put(name.clone(), po.get(&name).unwrap().as_text());
            }
            link.set_parameters(params);
        }
        let rb = self.get_string("requestBody", node, false, location, result);
        if result.allow_empty_strings && let Some(rb) = rb {
            link.set_request_body(JavaObject::string(rb));
        }
        if let Some(h) = self.get_object("headers", node, false, location, result) {
            link.set_headers(self.get_headers(Some(&h), location, result, false)?);
        }
        if let Some(s) = self.get_object("server", node, false, location, result) {
            link.set_server(self.get_server(Some(&s), location, result, None)?);
        }
        let v = self.get_string("description", node, false, location, result);
        if result.allow(&v) {
            link.set_description(v);
        }
        let ext = self.get_extensions(node)?;
        if !ext.is_empty() {
            link.set_extensions(ext);
        }
        for key in get_keys(Some(node)) {
            if !spec_keys(result.openapi31, "LINK_KEYS").contains(&key.as_str()) && !key.starts_with("x-") {
                result.extra(location, &key, node.get(&key));
            }
            validate_reserved_keywords(&key, location, result);
        }
        Ok(Some(link))
    }

    /// `getCallbacks(node, location, result, underComponents)`: a `callbacks` map; under `components` names are checked against the component-name pattern.
    pub fn get_callbacks(&mut self, node: Option<&JsonNode>, location: &str, result: &mut ParseResult, under_components: bool) -> JResult<JMap<Callback>> {
        let callbacks = JMap::new();
        let Some(node) = node else { return Ok(callbacks) };
        for key in get_keys(Some(node)) {
            if under_components && !matches_component_name(&key) {
                result.warning(location, &format!("Callback key {key} doesn't adhere to regular expression ^[a-zA-Z0-9\\.\\-_]+$"));
            }
            let n = cast_object(node.get(&key).unwrap())?;
            if let Some(cb) = self.get_callback(Some(n), location, result)? {
                callbacks.put(key, cb);
            }
        }
        Ok(callbacks)
    }

    /// `getCallback(node, location, result)`: one callback (a map of expressions to path items), `None` when `node` is `None`.
    pub fn get_callback(&mut self, node: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> JResult<Option<Callback>> {
        let Some(node) = node else { return Ok(None) };
        let callback = Callback::new();
        for name in get_keys(Some(node)) {
            let value = node.get(&name).unwrap();
            if let Some(r) = node.get("$ref") {
                if let Some(text) = r.text_value() {
                    match self.munged_ref(text) {
                        Some(m) => callback.set_ref(m),
                        None => callback.set_ref(text),
                    }
                    if text.starts_with("#/components") && !text.starts_with("#/components/callbacks") {
                        result.warning(location, &format!("$ref target {text} is not of expected type Callback"));
                    }
                    return Ok(Some(callback));
                }
                result.invalid_type(location, "$ref", "string", Some(node));
                return Ok(None);
            }
            if value.is_object() {
                let pi = self.get_path_item(value, location, result)?;
                match pi {
                    Some(pi) => {
                        callback.put(name.clone(), pi);
                    }
                    None => {
                        // Java puts a null value; a null PathItem cannot be represented, so it is skipped
                    }
                }
            } else {
                result.invalid_type(location, &name, "object", Some(value));
            }
            let ext = self.get_extensions(node)?;
            if !ext.is_empty() {
                callback.set_extensions(ext);
            }
        }
        Ok(Some(callback))
    }

    /// `getXml(node, location, result)`: a schema's `xml` object, `None` when `node` is `None`.
    pub fn get_xml(&mut self, node: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> JResult<Option<XML>> {
        let Some(node) = node else { return Ok(None) };
        let xml = XML::new();
        let v = self.get_string("name", node, false, &format!("{location}.name"), result);
        if result.allow(&v) {
            xml.set_name(v);
        }
        let v = self.get_string("namespace", node, false, &format!("{location}.namespace"), result);
        if result.allow(&v) {
            xml.set_namespace(v);
        }
        let v = self.get_string("prefix", node, false, &format!("{location}.prefix"), result);
        if result.allow(&v) {
            xml.set_prefix(v);
        }
        if let Some(b) = get_boolean("attribute", node, false, location, result) {
            xml.set_attribute(b);
        }
        if let Some(b) = get_boolean("wrapped", node, false, location, result) {
            xml.set_wrapped(b);
        }
        let ext = self.get_extensions(node)?;
        if !ext.is_empty() {
            xml.set_extensions(ext);
        }
        for key in get_keys(Some(node)) {
            if !spec_keys(result.openapi31, "XML_KEYS").contains(&key.as_str()) && !key.starts_with("x-") {
                result.extra(location, &key, node.get(&key));
            }
            validate_reserved_keywords(&key, location, result);
        }
        Ok(Some(xml))
    }

    /// `getArray(key, node, required, location, result, noInvalidError)`
    pub fn get_array_full(&mut self, key: &str, node: &JsonNode, required: bool, location: &str, result: &mut ParseResult, no_invalid_error: bool) -> Option<JsonNode> {
        match node.get(key) {
            None => {
                if required {
                    result.missing(location, key);
                    result.invalid();
                }
                None
            }
            Some(v) if !v.is_array() => {
                if !no_invalid_error {
                    result.invalid_type(location, key, "array", Some(v));
                }
                None
            }
            Some(v) => Some(v.clone()),
        }
    }

    /// `getArray(key, node, required, location, result)`: the array value of `key`; a missing required key or a non-array value is reported to `result` and gives `None`.
    pub fn get_array(&mut self, key: &str, node: &JsonNode, required: bool, location: &str, result: &mut ParseResult) -> Option<JsonNode> {
        self.get_array_full(key, node, required, location, result, false)
    }

    /// `getParameters(obj, location, result, underComponents)`: a `parameters` map; under `components` names are checked against the component-name pattern.
    pub fn get_parameters(&mut self, obj: Option<&JsonNode>, location: &str, result: &mut ParseResult, under_components: bool) -> JResult<JMap<Parameter>> {
        let parameters = JMap::new();
        let Some(obj) = obj else { return Ok(parameters) };
        for name in get_keys(Some(obj)) {
            if under_components && !matches_component_name(&name) {
                result.warning(location, &format!("Parameter name {name} doesn't adhere to regular expression ^[a-zA-Z0-9\\.\\-_]+$"));
            }
            let v = obj.get(&name).unwrap();
            if v.is_object() && let Some(p) = self.get_parameter(Some(v), &format!("{location}.{name}"), result)? {
                if p.get_in().is_some_and(|i| i.eq_ignore_ascii_case(PATH_PARAMETER)) && p.get_required() == Some(false) {
                    result.warning(location, &format!("For path parameter {name} the required value should be true"));
                }
                parameters.put(name, p);
            }
        }
        Ok(parameters)
    }

    /// `getParameterList(obj, location, result)`: an operation's or path item's `parameters` array; duplicate name and `in` pairs add a warning.
    pub fn get_parameter_list(&mut self, obj: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> JResult<JList<Parameter>> {
        let parameters = JList::new();
        let Some(obj) = obj else { return Ok(parameters) };
        for item in obj.as_array().unwrap().iter() {
            if item.is_object() && let Some(p) = self.get_parameter(Some(item), location, result)? {
                parameters.add(p);
            }
        }
        let mut filter: HashSet<String> = HashSet::new();
        for p in parameters.to_vec() {
            let param = self.get_parameter_definition(&p);
            let r = param.get_ref();
            let key = format!("{}#{}", param.get_name().as_deref().unwrap_or("null"), param.get_in().as_deref().unwrap_or("null"));
            if !filter.insert(key) {
                match r {
                    Some(r) => {
                        if r.starts_with(REFERENCE_SEPARATOR) {
                            result.warning(location, " There are duplicate parameter values");
                        }
                    }
                    None => result.warning(location, " There are duplicate parameter values"),
                }
            }
        }
        Ok(parameters)
    }

    fn get_parameter_definition(&self, parameter: &Parameter) -> Parameter {
        let Some(r) = parameter.get_ref() else { return parameter.clone() };
        let name = extract_simple_name(&r).0;
        self.components.as_ref().and_then(|c| c.get_parameters()).and_then(|p| p.get(&name)).unwrap_or_else(|| parameter.clone())
    }

    /// `getParameter(obj, location, result)`: one parameter object (or `$ref`), `None` when `obj` is `None` or invalid.
    pub fn get_parameter(&mut self, obj: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> JResult<Option<Parameter>> {
        let Some(obj) = obj else { return Ok(None) };
        if let Some(r) = obj.get("$ref") {
            if let Some(text) = r.text_value() {
                let parameter = Parameter::new();
                match self.munged_ref(text) {
                    Some(m) => parameter.set_ref(m),
                    None => parameter.set_ref(text),
                }
                if text.starts_with("#/components") && !(text.starts_with("#/components/parameters") || text.starts_with("#/components/headers")) {
                    result.warning(location, &format!("$ref target {text} is not of expected type Parameter/Header"));
                }
                if result.openapi31 {
                    let d = self.get_string("description", obj, false, location, result);
                    if !is_blank(d.as_deref()) {
                        parameter.set_description(d);
                    }
                }
                return Ok(Some(parameter));
            }
            result.invalid_type(location, "$ref", "string", Some(obj));
            return Ok(None);
        }
        let l = match obj.get("name") {
            Some(n) => n.as_text(),
            None => "['unknown']".to_string(),
        };
        let location = format!("{location}.[{l}]");
        let location = location.as_str();
        let value = self.get_string("in", obj, true, location, result);
        if (!result.allow_empty_strings && is_blank(value.as_deref())) || (result.allow_empty_strings && value.is_none()) {
            return Ok(None);
        }
        let value = value.unwrap();
        let parameter = match value.as_str() {
            QUERY_PARAMETER => Parameter::of(ParameterKind::Query),
            HEADER_PARAMETER => Parameter::of(ParameterKind::Header),
            PATH_PARAMETER => Parameter::of(ParameterKind::Path),
            COOKIE_PARAMETER => Parameter::of(ParameterKind::Cookie),
            _ => {
                result.invalid_type(location, "in", "[query|header|path|cookie]", Some(obj));
                return Ok(None);
            }
        };
        parameter.set_in(value);
        let v = self.get_string("name", obj, true, location, result);
        if result.allow(&v) {
            parameter.set_name(v);
        }
        let v = self.get_string("description", obj, false, location, result);
        if result.allow(&v) {
            parameter.set_description(v);
        }
        match get_boolean("required", obj, false, location, result) {
            Some(b) => parameter.set_required(b),
            None => parameter.set_required(false),
        }
        if let Some(b) = get_boolean("deprecated", obj, false, location, result) {
            parameter.set_deprecated(b);
        }
        if parameter.kind() == ParameterKind::Query
            && let Some(b) = get_boolean("allowEmptyValue", obj, false, location, result)
        {
            parameter.set_allow_empty_value(b);
        }
        if let Some(so) = self.get_object("schema", obj, false, location, result) {
            parameter.set_schema(self.get_schema(Some(&so), &format!("{location}.schemas"), Some(result))?);
        }
        let examples_object = self.get_object("examples", obj, false, location, result);
        if let Some(eo) = &examples_object {
            parameter.set_examples(self.get_examples(Some(eo), &format!("{location}.examples"), result, false)?);
        }
        if let Some(example) = self.get_any_type("example", obj, location, result) {
            if examples_object.is_some() {
                result.warning(location, "examples already defined -- ignoring \"example\" field");
            } else {
                parameter.set_example(if is_null_node(&example) { None } else { Some(example) });
            }
        }
        if let Some(b) = get_boolean("allowReserved", obj, false, location, result) {
            parameter.set_allow_reserved(b);
        }
        if let Some(cn) = self.get_object("content", obj, false, location, result) {
            let content = self.get_content(Some(&cn), &format!("{location}.content"), result)?.unwrap();
            if content.size() == 0 {
                result.unsupported(location, "content with no media type", &cn);
                result.invalid();
            } else if content.size() > 1 {
                result.unsupported(location, "content with multiple media types", &cn);
                result.invalid();
            } else if parameter.get_schema().is_some() {
                result.unsupported(location, "content when schema defined", &cn);
                result.invalid();
            } else {
                parameter.set_content(content);
            }
        } else if parameter.get_schema().is_none() {
            result.missing(location, "content");
        }
        let v = self.get_string("style", obj, false, location, result);
        if parameter.get_content().is_none() {
            self.set_style(v.as_deref(), &parameter, location, obj, result);
            match get_boolean("explode", obj, false, location, result) {
                Some(b) => parameter.set_explode(b),
                None => {
                    if parameter.get_style() == Some(ParameterStyle::Form) && result.explicit_style_and_explode {
                        parameter.set_explode(true);
                    } else if result.explicit_style_and_explode {
                        parameter.set_explode(false);
                    }
                }
            }
        }
        let ext = self.get_extensions(obj)?;
        if !ext.is_empty() {
            parameter.set_extensions(ext);
        }
        for key in get_keys(Some(obj)) {
            if !spec_keys(result.openapi31, "PARAMETER_KEYS").contains(&key.as_str()) && !key.starts_with("x-") {
                result.extra(location, &key, obj.get(&key));
            }
            validate_reserved_keywords(&key, location, result);
        }
        Ok(Some(parameter))
    }

    /// `getHeaders(obj, location, result, underComponents)`: a `headers` map; under `components` names are checked against the component-name pattern.
    pub fn get_headers(&mut self, obj: Option<&JsonNode>, location: &str, result: &mut ParseResult, under_components: bool) -> JResult<JMap<Header>> {
        let headers = JMap::new();
        let Some(obj) = obj else { return Ok(headers) };
        for name in get_keys(Some(obj)) {
            if under_components && !matches_component_name(&name) {
                result.warning(location, &format!("Header name {name} doesn't adhere to regular expression ^[a-zA-Z0-9\\.\\-_]+$"));
            }
            let v = obj.get(&name).unwrap();
            if !v.is_object() {
                result.invalid_type(location, &name, "object", Some(v));
            } else if let Some(h) = self.get_header(Some(v), &format!("{location}.{name}"), result)? {
                headers.put(name, h);
            }
        }
        Ok(headers)
    }

    /// `getHeader(node, location, result)`: one header object (or `$ref`), `None` when `node` is `None`.
    pub fn get_header(&mut self, node: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> JResult<Option<Header>> {
        let Some(node) = node else { return Ok(None) };
        let header = Header::new();
        if let Some(r) = node.get("$ref") {
            if let Some(text) = r.text_value() {
                match self.munged_ref(text) {
                    Some(m) => header.set_ref(m),
                    None => header.set_ref(text),
                }
                if text.starts_with("#/components") && !(text.starts_with("#/components/parameters") || text.starts_with("#/components/headers")) {
                    result.warning(location, &format!("$ref target {text} is not of expected type Header/Parameter"));
                }
                if result.openapi31 {
                    let d = self.get_string("description", node, false, location, result);
                    if !is_blank(d.as_deref()) {
                        header.set_description(d);
                    }
                }
                return Ok(Some(header));
            }
            result.invalid_type(location, "$ref", "string", Some(node));
            return Ok(None);
        }
        let v = self.get_string("description", node, false, location, result);
        if result.allow(&v) {
            header.set_description(v);
        }
        if let Some(b) = get_boolean("required", node, false, location, result) {
            header.set_required(b);
        }
        if let Some(b) = get_boolean("deprecated", node, false, location, result) {
            header.set_deprecated(b);
        }
        let style = self.get_string("style", node, false, location, result);
        if is_blank(style.as_deref()) {
            if result.explicit_style_and_explode {
                header.set_style(HeaderStyle::Simple);
            }
        } else if style.as_deref() == Some("simple") {
            header.set_style(HeaderStyle::Simple);
        } else {
            result.invalid_type(location, "style", "simple", Some(node));
        }
        match get_boolean("explode", node, false, location, result) {
            Some(b) => header.set_explode(b),
            None => {
                if result.explicit_style_and_explode {
                    header.set_explode(false);
                }
            }
        }
        if let Some(so) = self.get_object("schema", node, false, location, result) {
            header.set_schema(self.get_schema(Some(&so), location, Some(result))?);
        }
        let examples_object = self.get_object("examples", node, false, location, result);
        if let Some(eo) = &examples_object {
            header.set_examples(self.get_examples(Some(eo), location, result, false)?);
        }
        if let Some(example) = self.get_any_type("example", node, location, result) {
            if examples_object.is_some() {
                result.warning(location, "examples already defined -- ignoring \"example\" field");
            } else {
                header.set_example(if is_null_node(&example) { None } else { Some(example) });
            }
        }
        if let Some(cn) = self.get_object("content", node, false, location, result) {
            header.set_content(self.get_content(Some(&cn), &format!("{location}.content"), result)?);
        }
        let ext = self.get_extensions(node)?;
        if !ext.is_empty() {
            header.set_extensions(ext);
        }
        for key in get_keys(Some(node)) {
            if !spec_keys(result.openapi31, "HEADER_KEYS").contains(&key.as_str()) && !key.starts_with("x-") {
                result.extra(location, &key, node.get(&key));
            }
        }
        Ok(Some(header))
    }

    /// `getAnyType`: String / Integer / BigDecimal / ObjectNode / ArrayNode / Boolean / NullNode.
    pub fn get_any_type(&mut self, key: &str, node: &JsonNode, location: &str, result: &mut ParseResult) -> Option<JavaObject> {
        let example = node.get(key)?;
        match node_type(example) {
            NodeType::String => self.get_string(key, node, false, location, result).map(JavaObject::string),
            NodeType::Number => {
                if let Some(i) = get_integer(key, node, false, location, result) {
                    return Some(JavaObject::Integer(i));
                }
                get_big_decimal(key, node, false, location, result).map(|b| JavaObject::BigDecimal(b.into()))
            }
            NodeType::Object => self.get_object(key, node, false, location, result).map(JavaObject::Node),
            NodeType::Array => self.get_array(key, node, false, location, result).map(JavaObject::Node),
            NodeType::Boolean => get_boolean(key, node, false, location, result).map(JavaObject::Boolean),
            NodeType::Null => Some(JavaObject::Node(JsonNode::Null)),
            _ => None,
        }
    }

    /// `getSecuritySchemes(obj, location, result, underComponents)`: a `securitySchemes` map; under `components` names are checked against the component-name pattern.
    pub fn get_security_schemes(&mut self, obj: Option<&JsonNode>, location: &str, result: &mut ParseResult, under_components: bool) -> JResult<JMap<SecurityScheme>> {
        let schemes = JMap::new();
        let Some(obj) = obj else { return Ok(schemes) };
        for name in get_keys(Some(obj)) {
            if under_components && !matches_component_name(&name) {
                result.warning(location, &format!("SecurityScheme name {name} doesn't adhere to regular expression ^[a-zA-Z0-9\\.\\-_]+$"));
            }
            let v = obj.get(&name).unwrap();
            if !v.is_object() {
                result.invalid_type(location, &name, "object", Some(v));
            } else {
                // Java checks the node (not the result) for null, so a null scheme is stored; skip it here
                if let Some(s) = self.get_security_scheme(Some(v), &format!("{location}.{name}"), result)? {
                    schemes.put(name, s);
                }
            }
        }
        Ok(schemes)
    }

    /// `getSecurityScheme(node, location, result)`: one security scheme (or `$ref`), `None` when `node` is `None` or invalid.
    pub fn get_security_scheme(&mut self, node: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> JResult<Option<SecurityScheme>> {
        let Some(node) = node else { return Ok(None) };
        let ss = SecurityScheme::new();
        if let Some(r) = node.get("$ref") {
            if let Some(text) = r.text_value() {
                match self.munged_ref(text) {
                    Some(m) => ss.set_ref(m),
                    None => ss.set_ref(text),
                }
                if text.starts_with("#/components") && !text.starts_with("#/components/securitySchemes") {
                    result.warning(location, &format!("$ref target {text} is not of expected type securitySchemes"));
                }
                if result.openapi31 {
                    let d = self.get_string("description", node, false, location, result);
                    if !is_blank(d.as_deref()) {
                        ss.set_description(d);
                    }
                }
                return Ok(Some(ss));
            }
            result.invalid_type(location, "$ref", "string", Some(node));
            return Ok(None);
        }
        let (mut name_required, mut in_required, mut scheme_required, mut flows_required, mut oidc_required) = (false, false, false, false, false);
        let v = self.get_string("type", node, true, location, result);
        if result.allow(&v) {
            match v.as_deref().unwrap() {
                "apiKey" => {
                    ss.set_type(SecuritySchemeType::ApiKey);
                    name_required = true;
                    in_required = true;
                }
                "http" => {
                    ss.set_type(SecuritySchemeType::Http);
                    scheme_required = true;
                }
                "oauth2" => {
                    ss.set_type(SecuritySchemeType::Oauth2);
                    flows_required = true;
                }
                "openIdConnect" => {
                    ss.set_type(SecuritySchemeType::OpenIdConnect);
                    oidc_required = true;
                }
                "mutualTLS" if result.openapi31 => ss.set_type(SecuritySchemeType::MutualTls),
                _ => result.invalid_type(&format!("{location}.type"), "type", "http|apiKey|oauth2|openIdConnect|mutualTLS ", Some(node)),
            }
        }
        let v = self.get_string("description", node, false, location, result);
        if result.allow(&v) {
            ss.set_description(v);
        }
        let v = self.get_string("name", node, name_required, location, result);
        if result.allow(&v) {
            ss.set_name(v);
        }
        let ss_in = self.get_string("in", node, in_required, location, result);
        let matching = ss_in.as_deref().and_then(SecuritySchemeIn::from_str_value);
        if in_required && ss_in.is_some() && matching.is_none() {
            result.invalid_type(location, "in", "cookie|header|query", Some(node));
        }
        ss.set_in(matching);
        let v = self.get_string("scheme", node, scheme_required, location, result);
        if result.allow(&v) {
            ss.set_scheme(v);
        }
        let v = self.get_string("bearerFormat", node, false, location, result);
        if result.allow(&v) {
            ss.set_bearer_format(v);
        }
        if let Some(fo) = self.get_object("flows", node, flows_required, location, result) {
            ss.set_flows(self.get_oauth_flows(Some(&fo), location, result)?);
        }
        let v = self.get_string("openIdConnectUrl", node, oidc_required, location, result);
        if result.allow(&v) {
            ss.set_open_id_connect_url(v);
        }
        let ext = self.get_extensions(node)?;
        if !ext.is_empty() {
            ss.set_extensions(ext);
        }
        for key in get_keys(Some(node)) {
            if !spec_keys(result.openapi31, "SECURITY_SCHEME_KEYS").contains(&key.as_str()) && !key.starts_with("x-") {
                result.extra(location, &key, node.get(&key));
            }
        }
        Ok(Some(ss))
    }

    /// `getOAuthFlows(node, location, result)`: a security scheme's `flows` object, `None` when `node` is `None`.
    pub fn get_oauth_flows(&mut self, node: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> JResult<Option<OAuthFlows>> {
        let Some(node) = node else { return Ok(None) };
        let flows = OAuthFlows::new();
        if let Some(o) = self.get_object("implicit", node, false, location, result) {
            flows.set_implicit(self.get_oauth_flow("implicit", Some(&o), location, result)?);
        }
        if let Some(o) = self.get_object("password", node, false, location, result) {
            flows.set_password(self.get_oauth_flow("password", Some(&o), location, result)?);
        }
        if let Some(o) = self.get_object("clientCredentials", node, false, location, result) {
            flows.set_client_credentials(self.get_oauth_flow("clientCredentials", Some(&o), location, result)?);
        }
        if let Some(o) = self.get_object("authorizationCode", node, false, location, result) {
            flows.set_authorization_code(self.get_oauth_flow("authorizationCode", Some(&o), location, result)?);
        }
        let ext = self.get_extensions(node)?;
        if !ext.is_empty() {
            flows.set_extensions(ext);
        }
        for key in get_keys(Some(node)) {
            if !spec_keys(result.openapi31, "OAUTHFLOWS_KEYS").contains(&key.as_str()) && !key.starts_with("x-") {
                result.extra(location, &key, node.get(&key));
            }
        }
        Ok(Some(flows))
    }

    /// `getOAuthFlow(flowType, node, location, result)`: one OAuth flow; `flowType` (`implicit`, `password`, ...) decides which URLs are required.
    pub fn get_oauth_flow(&mut self, flow_type: &str, node: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> JResult<Option<OAuthFlow>> {
        let Some(node) = node else { return Ok(None) };
        let flow = OAuthFlow::new();
        let (auth_req, token_req) = match flow_type {
            "implicit" => (true, false),
            "password" | "clientCredentials" => (false, true),
            "authorizationCode" => (true, true),
            _ => (false, false),
        };
        let v = self.get_string("authorizationUrl", node, auth_req, location, result);
        if result.allow(&v) {
            flow.set_authorization_url(v);
        }
        let v = self.get_string("tokenUrl", node, token_req, location, result);
        if result.allow(&v) {
            flow.set_token_url(v);
        }
        let v = self.get_string("refreshUrl", node, false, location, result);
        if result.allow(&v) {
            flow.set_refresh_url(v);
        }
        let scopes_object = self.get_object("scopes", node, true, location, result);
        let scope = Scopes::new();
        for name in get_keys(scopes_object.as_ref()) {
            let sv = scopes_object.as_ref().unwrap().get(&name).unwrap();
            scope.add_string(&name, &sv.as_text());
        }
        flow.set_scopes(scope);
        let ext = self.get_extensions(node)?;
        if !ext.is_empty() {
            flow.set_extensions(ext);
        }
        for key in get_keys(Some(node)) {
            if !spec_keys(result.openapi31, "OAUTHFLOW_KEYS").contains(&key.as_str()) && !key.starts_with("x-") {
                result.extra(location, &key, node.get(&key));
            }
        }
        Ok(Some(flow))
    }

    /// `getExamples(obj, location, result, underComponents)`: an `examples` map; under `components` names are checked against the component-name pattern.
    pub fn get_examples(&mut self, obj: Option<&JsonNode>, location: &str, result: &mut ParseResult, under_components: bool) -> JResult<JMap<Example>> {
        let examples = JMap::new();
        let Some(obj) = obj else { return Ok(examples) };
        for name in get_keys(Some(obj)) {
            if under_components && !matches_component_name(&name) {
                result.warning(location, &format!("Example name {name} doesn't adhere to regular expression ^[a-zA-Z0-9\\.\\-_]+$"));
            }
            let v = obj.get(&name).unwrap();
            if !(v.is_object() || v.is_textual()) {
                result.invalid_type(location, &name, "object", Some(v));
            } else if let Some(text) = v.text_value() {
                let e = Example::new();
                e.set_value(JavaObject::string(text));
                examples.put(name, e);
            } else if let Some(e) = self.get_example(Some(v), &format!("{location}.{name}"), result)? {
                examples.put(name, e);
            }
        }
        Ok(examples)
    }

    /// `getExample(node, location, result)`: one example object (or `$ref`), `None` when `node` is `None`.
    pub fn get_example(&mut self, node: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> JResult<Option<Example>> {
        let Some(node) = node else { return Ok(None) };
        let example = Example::new();
        if let Some(r) = node.get("$ref") {
            if let Some(text) = r.text_value() {
                match self.munged_ref(text) {
                    Some(m) => example.set_ref(m),
                    None => example.set_ref(text),
                }
                if text.starts_with("#/components") && !text.starts_with("#/components/examples") {
                    result.warning(location, &format!("$ref target {text} is not of expected type Examples"));
                }
                if result.openapi31 {
                    let v = self.get_string("summary", node, false, location, result);
                    if !is_blank(v.as_deref()) {
                        example.set_summary(v);
                    }
                    let v = self.get_string("description", node, false, location, result);
                    if !is_blank(v.as_deref()) {
                        example.set_description(v);
                    }
                }
                return Ok(Some(example));
            }
            result.invalid_type(location, "$ref", "string", Some(node));
            return Ok(None);
        }
        let v = self.get_string("summary", node, false, location, result);
        if result.allow(&v) {
            example.set_summary(v);
        }
        let v = self.get_string("description", node, false, location, result);
        if result.allow(&v) {
            example.set_description(v);
        }
        let sample = self.get_any_type("value", node, location, result);
        if let Some(s) = &sample {
            example.set_value(if is_null_node(s) { None } else { Some(s.clone()) });
        }
        let value = self.get_string("externalValue", node, false, location, result);
        if result.allow(&value) {
            example.set_external_value(value.clone());
        }
        if sample.is_some() && value.is_some() {
            result.warning(location, " value and externalValue are both present");
        }
        let ext = self.get_extensions(node)?;
        if !ext.is_empty() {
            example.set_extensions(ext);
        }
        for key in get_keys(Some(node)) {
            if !spec_keys(result.openapi31, "EXAMPLE_KEYS").contains(&key.as_str()) && !key.starts_with("x-") {
                result.extra(location, &key, node.get(&key));
            }
            validate_reserved_keywords(&key, location, result);
        }
        Ok(Some(example))
    }

    /// `setStyle(value, parameter, location, obj, result)`: sets the parameter's `style` from `value`; a blank value takes the default for its `in` (`form` for query/cookie, `simple` for path/header) when `explicit_style_and_explode` is on, an unknown one is reported as an invalid type.
    pub fn set_style(&mut self, value: Option<&str>, parameter: &Parameter, location: &str, obj: &JsonNode, result: &mut ParseResult) {
        if is_blank(value) {
            if result.explicit_style_and_explode {
                let i = parameter.get_in();
                if i.as_deref() == Some(QUERY_PARAMETER) || i.as_deref() == Some(COOKIE_PARAMETER) {
                    parameter.set_style(ParameterStyle::Form);
                } else if i.as_deref() == Some(PATH_PARAMETER) || i.as_deref() == Some(HEADER_PARAMETER) {
                    parameter.set_style(ParameterStyle::Simple);
                }
            }
        } else {
            match ParameterStyle::from_str_value(value.unwrap()) {
                Some(s) => parameter.set_style(s),
                None => result.invalid_type(location, "style", "StyleEnum", Some(obj)),
            }
        }
    }

    /// `getResponses(node, location, result, underComponents)`: the `responses` object, `None` when `node` is `None`.
    pub fn get_responses(&mut self, node: Option<&JsonNode>, location: &str, result: &mut ParseResult, under_components: bool) -> JResult<Option<ApiResponses>> {
        let Some(node) = node else { return Ok(None) };
        let responses = ApiResponses::new();
        for key in get_keys(Some(node)) {
            if under_components && !matches_component_name(&key) {
                result.warning(location, &format!("Response key {key} doesn't adhere to regular expression ^[a-zA-Z0-9\\.\\-_]+$"));
            }
            if key.starts_with("x-") {
                let ext = self.get_extensions(node)?;
                if !ext.is_empty() {
                    responses.set_extensions(ext);
                }
            } else if let Some(obj) = self.get_object(&key, node, false, location, result)
                && let Some(r) = self.get_response(Some(&obj), &format!("{location}.{key}"), result)?
            {
                responses.put(key, r);
            }
        }
        Ok(Some(responses))
    }

    /// Components-level `getResponses` returns the `ApiResponses` map itself (a `Map<String, ApiResponse>`).
    fn get_responses_map(&mut self, node: Option<&JsonNode>, location: &str, result: &mut ParseResult, under_components: bool) -> JResult<JMap<ApiResponse>> {
        let r = self.get_responses(node, location, result, under_components)?;
        let m = JMap::new();
        if let Some(r) = r {
            for (k, v) in r.entries() {
                m.put(k, v);
            }
        }
        Ok(m)
    }

    /// `getResponse(node, location, result)`: one response object (or `$ref`), `None` when `node` is `None`.
    pub fn get_response(&mut self, node: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> JResult<Option<ApiResponse>> {
        let Some(node) = node else { return Ok(None) };
        let response = ApiResponse::new();
        if let Some(r) = node.get("$ref") {
            if let Some(text) = r.text_value() {
                match self.munged_ref(text) {
                    Some(m) => response.set_ref(m),
                    None => response.set_ref(text),
                }
                if text.starts_with("#/components") && !text.starts_with("#/components/responses") {
                    result.warning(location, &format!("$ref target {text} is not of expected type Response"));
                }
                if result.openapi31 {
                    let v = self.get_string("description", node, false, location, result);
                    if !is_blank(v.as_deref()) {
                        response.set_description(v);
                    }
                }
                return Ok(Some(response));
            }
            result.invalid_type(location, "$ref", "string", Some(node));
            return Ok(None);
        }
        let v = self.get_string("description", node, true, location, result);
        if result.allow(&v) {
            response.set_description(v);
        }
        if let Some(h) = self.get_object("headers", node, false, location, result) {
            let headers = self.get_headers(Some(&h), &format!("{location}.headers"), result, false)?;
            if headers.size() > 0 {
                response.set_headers(headers);
            }
        }
        if let Some(l) = self.get_object("links", node, false, location, result) {
            let links = self.get_links(Some(&l), &format!("{location}.links"), result, false)?;
            if links.size() > 0 {
                response.set_links(links);
            }
        }
        if let Some(c) = self.get_object("content", node, false, location, result) {
            response.set_content(self.get_content(Some(&c), &format!("{location}.content"), result)?);
        }
        let ext = self.get_extensions(node)?;
        if !ext.is_empty() {
            response.set_extensions(ext);
        }
        for key in get_keys(Some(node)) {
            if !spec_keys(result.openapi31, "RESPONSE_KEYS").contains(&key.as_str()) && !key.starts_with("x-") {
                result.extra(location, &key, node.get(&key));
            }
            validate_reserved_keywords(&key, location, result);
        }
        Ok(Some(response))
    }

    /// `getOperation(obj, location, result)`: one operation; a repeated `operationId` is reported.
    pub fn get_operation(&mut self, obj: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> JResult<Option<Operation>> {
        let Some(obj) = obj else { return Ok(None) };
        let operation = Operation::new();
        let array = self.get_array("tags", obj, false, location, result);
        if let Some(a) = array {
            let tags: JList<String> = JList::new();
            for n in a.as_array().unwrap().iter() {
                if let Some(t) = n.text_value() {
                    tags.add(t.to_string());
                }
            }
            operation.set_tags(tags);
        }
        let v = self.get_string("summary", obj, false, location, result);
        if result.allow(&v) {
            operation.set_summary(v);
        }
        let v = self.get_string("description", obj, false, location, result);
        if result.allow(&v) {
            operation.set_description(v);
        }
        let ed = self.get_object("externalDocs", obj, false, location, result);
        let docs = self.get_external_docs(ed.as_ref(), &format!("{location}.externalDocs"), result);
        if docs.is_some() {
            operation.set_external_docs(docs);
        }
        let mut ids = std::mem::take(&mut self.operation_ids);
        let v = self.get_string_full("operationId", obj, false, location, result, Some(&mut ids), false, false);
        self.operation_ids = ids;
        if result.allow(&v) {
            operation.set_operation_id(v);
        }
        if let Some(p) = self.get_array("parameters", obj, false, location, result) {
            operation.set_parameters(self.get_parameter_list(Some(&p), &format!("{location}.parameters"), result)?);
        }
        if let Some(rb) = self.get_object("requestBody", obj, false, location, result) {
            operation.set_request_body(self.get_request_body(Some(&rb), &format!("{location}.requestBody"), result)?);
        }
        let responses_node = self.get_object("responses", obj, true, location, result);
        if let Some(r) = self.get_responses(responses_node.as_ref(), &format!("{location}.responses"), result, false)? {
            operation.set_responses(r);
        }
        let cb = self.get_object("callbacks", obj, false, location, result);
        if cb.is_some() {
            let callbacks = self.get_callbacks(cb.as_ref(), &format!("{location}.callbacks"), result, false)?;
            operation.set_callbacks(callbacks);
        }
        if let Some(b) = get_boolean("deprecated", obj, false, location, result) {
            operation.set_deprecated(b);
        }
        if let Some(a) = self.get_array("servers", obj, false, location, result) && a.size() > 0 {
            operation.set_servers(self.get_servers_list(Some(&a), &format!("{location}.servers"), result, None)?);
        }
        if let Some(a) = self.get_array("security", obj, false, location, result) {
            operation.set_security(self.get_security_requirements_list(Some(&a), &format!("{location}.security"), result));
        }
        let ext = self.get_extensions(obj)?;
        if !ext.is_empty() {
            operation.set_extensions(ext);
        }
        for key in get_keys(Some(obj)) {
            if !spec_keys(result.openapi31, "OPERATION_KEYS").contains(&key.as_str()) && !key.starts_with("x-") {
                result.extra(location, &key, obj.get(&key));
            }
            validate_reserved_keywords(&key, location, result);
        }
        Ok(Some(operation))
    }

    /// `getSecurityRequirementsList(nodes, location, result)`: the `security` array; an empty object becomes an empty requirement, non-array scope values are skipped. `None` when `nodes` is `None`.
    pub fn get_security_requirements_list(&mut self, nodes: Option<&JsonNode>, _location: &str, _result: &mut ParseResult) -> Option<JList<SecurityRequirement>> {
        let nodes = nodes?;
        let reqs = JList::new();
        for node in nodes.as_array().unwrap().iter() {
            if node.is_object() {
                let sr = SecurityRequirement::new();
                let keys = get_keys(Some(node));
                if keys.is_empty() {
                    reqs.add(sr);
                } else {
                    for key in keys {
                        let value = node.get(&key).unwrap();
                        if let Some(arr) = value.as_array() {
                            let scopes: Vec<String> = arr.iter().map(|n| n.as_text()).collect();
                            sr.add_list_items(&key, JList::from_vec(scopes));
                        }
                    }
                    if sr.size() > 0 {
                        reqs.add(sr);
                    }
                }
            }
        }
        Some(reqs)
    }

    /// `getRequestBodies(obj, location, result, underComponents)`: a `requestBodies` map; under `components` names are checked against the component-name pattern.
    pub fn get_request_bodies(&mut self, obj: Option<&JsonNode>, location: &str, result: &mut ParseResult, under_components: bool) -> JResult<JMap<RequestBody>> {
        let bodies = JMap::new();
        let Some(obj) = obj else { return Ok(bodies) };
        for name in get_keys(Some(obj)) {
            if under_components && !matches_component_name(&name) {
                result.warning(location, &format!("RequestBody name {name} doesn't adhere to regular expression ^[a-zA-Z0-9\\.\\-_]+$"));
            }
            let v = obj.get(&name).unwrap();
            if !v.is_object() {
                result.invalid_type(location, &name, "object", Some(v));
            } else if let Some(b) = self.get_request_body(Some(v), &format!("{location}.{name}"), result)? {
                bodies.put(name, b);
            }
        }
        Ok(bodies)
    }

    /// `getRequestBody(node, location, result)`: one request body object (or `$ref`), `None` when `node` is `None`.
    pub fn get_request_body(&mut self, node: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> JResult<Option<RequestBody>> {
        let Some(node) = node else { return Ok(None) };
        let body = RequestBody::new();
        if let Some(r) = node.get("$ref") {
            if let Some(text) = r.text_value() {
                match self.munged_ref(text) {
                    Some(m) => body.set_ref(m),
                    None => body.set_ref(text),
                }
                if text.starts_with("#/components") && !text.starts_with("#/components/requestBodies") {
                    result.warning(location, &format!("$ref target {text} is not of expected type RequestBody"));
                }
                if result.openapi31 {
                    let d = self.get_string("description", node, false, location, result);
                    if !is_blank(d.as_deref()) {
                        body.set_description(d);
                    }
                }
                return Ok(Some(body));
            }
            result.invalid_type(location, "$ref", "string", Some(node));
            return Ok(None);
        }
        let d = self.get_string("description", node, false, location, result);
        if result.allow_empty_strings && d.is_some() {
            body.set_description(d);
        }
        if let Some(b) = get_boolean("required", node, false, location, result) {
            body.set_required(b);
        }
        let content_node = self.get_object("content", node, true, location, result);
        let content = self.get_content(content_node.as_ref(), &format!("{location}.content"), result)?;
        match &content {
            Some(c) if c.is_empty() => {
                result.unsupported(location, "content with no media type", content_node.as_ref().unwrap_or(&JsonNode::Null));
                result.invalid();
            }
            _ => body.set_content(content),
        }
        let ext = self.get_extensions(node)?;
        if !ext.is_empty() {
            body.set_extensions(ext);
        }
        for key in get_keys(Some(node)) {
            if !spec_keys(result.openapi31, "REQUEST_BODY_KEYS").contains(&key.as_str()) && !key.starts_with("x-") {
                result.extra(location, &key, node.get(&key));
            }
            validate_reserved_keywords(&key, location, result);
        }
        Ok(Some(body))
    }
}

enum PathsTarget<'a> {
    Paths(&'a Paths),
    Map(&'a JMap<PathItem>),
}

/// `new ObjectMapper().convertValue(rootNode, Map.class)`: only objects and null convert.
fn root_map_conversion(root: &JsonNode) -> JResult<()> {
    const SUFFIX: &str = "\n at [Source: UNKNOWN; byte offset: #UNKNOWN]";
    const MAP: &str = "`java.util.LinkedHashMap<java.lang.Object,java.lang.Object>`";
    let msg = match root {
        JsonNode::Object(_) | JsonNode::Null | JsonNode::Missing | JsonNode::Pojo(_) => return Ok(()),
        JsonNode::Array(_) => format!("Cannot deserialize value of type {MAP} from Array value (token `JsonToken.START_ARRAY`)"),
        JsonNode::Text(t) if t.is_empty() => {
            "Cannot coerce empty String (\"\") to element of `java.util.LinkedHashMap` (but could if coercion was enabled using `CoercionConfig`)".to_string()
        }
        JsonNode::Text(t) => format!(
            "Cannot construct instance of `java.util.LinkedHashMap` (although at least one Creator exists): no String-argument constructor/factory method to deserialize from String value ('{t}')"
        ),
        JsonNode::Boolean(true) => format!("Cannot deserialize value of type {MAP} from Boolean value (token `JsonToken.VALUE_TRUE`)"),
        JsonNode::Boolean(false) => format!("Cannot deserialize value of type {MAP} from Boolean value (token `JsonToken.VALUE_FALSE`)"),
        JsonNode::Binary(_) => format!("Cannot deserialize value of type {MAP} from Embedded Object value (token `JsonToken.VALUE_EMBEDDED_OBJECT`)"),
        n if n.is_integral_number() => format!("Cannot deserialize value of type {MAP} from Integer value (token `JsonToken.VALUE_NUMBER_INT`)"),
        _ => format!("Cannot deserialize value of type {MAP} from Floating-point value (token `JsonToken.VALUE_NUMBER_FLOAT`)"),
    };
    Err(JavaException::illegal_argument(format!("{msg}{SUFFIX}")))
}

/// `getKeys(node)`: field names in order.
pub fn get_keys(node: Option<&JsonNode>) -> Vec<String> {
    node.map(|n| n.field_names()).unwrap_or_default()
}

fn validate_reserved_keywords(key: &str, location: &str, result: &mut ParseResult) {
    if !result.oai_author && result.openapi31 && RESERVED_KEYWORDS_31.iter().any(|rk| key.starts_with(rk)) {
        result.reserved(location, key);
    }
}

fn try_direct_extensions(node: &JsonNode) -> JMap<JavaObject> {
    let ext = JMap::new();
    for key in get_keys(Some(node)) {
        if key.starts_with("x-") {
            ext.put(key.clone(), node_to_object(node.get(&key).unwrap()));
        }
    }
    ext
}

fn try_unwrap_lookup_extensions(node: &JsonNode) -> JResult<JMap<JavaObject>> {
    let ext = JMap::new();
    if let Some(en) = node.get("extensions") && node.is_object() {
        let eo = cast_object(en)?;
        for (k, v) in try_direct_extensions(eo).entries() {
            ext.put(k, v);
        }
    }
    Ok(ext)
}

pub(crate) fn is_null_node(o: &JavaObject) -> bool {
    matches!(o, JavaObject::Node(JsonNode::Null))
}

/// `getBoolean`
pub fn get_boolean(key: &str, node: &JsonNode, required: bool, location: &str, result: &mut ParseResult) -> Option<bool> {
    match node.get(key) {
        None => {
            if required {
                result.missing(location, key);
                result.invalid();
            }
            None
        }
        Some(JsonNode::Boolean(b)) => Some(*b),
        Some(JsonNode::Text(s)) => Some(s.eq_ignore_ascii_case("true")),
        _ => None,
    }
}

/// `getBigDecimal`
pub fn get_big_decimal(key: &str, node: &JsonNode, required: bool, location: &str, result: &mut ParseResult) -> Option<BigDecimal> {
    match node.get(key) {
        None => {
            if required {
                result.missing(location, key);
                result.invalid();
            }
            None
        }
        Some(v) if v.is_number() => BigDecimal::parse(&v.as_text()).ok(),
        Some(v) if !v.is_value_node() => {
            result.invalid_type(location, key, "double", Some(node));
            None
        }
        _ => None,
    }
}

/// `getInteger`: only `IntNode` values
pub fn get_integer(key: &str, node: &JsonNode, required: bool, location: &str, result: &mut ParseResult) -> Option<i32> {
    match node.get(key) {
        None => {
            if required {
                result.missing(location, key);
                result.invalid();
            }
            None
        }
        Some(JsonNode::Int(i)) => Some(*i),
        Some(v) if v.is_number() => None,
        Some(v) if !v.is_value_node() => {
            result.invalid_type(location, key, "integer", Some(node));
            None
        }
        _ => None,
    }
}

fn path_template_parts(path: &str) -> Vec<String> {
    // Pattern "\{(.+?)\}"
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"\{(.+?)\}").unwrap());
    re.find_iter(path).map(|m| m.as_str().to_string()).collect()
}

fn is_path_param_defined(path_param: &str, parameters: Option<&[Parameter]>) -> bool {
    match parameters {
        None => false,
        Some([]) => false,
        Some(p) => p.iter().any(|param| param.get_ref().is_some() || (param.get_name().as_deref() == Some(path_param) && param.get_in().as_deref() == Some("path"))),
    }
}

/// `getAllOperationsInAPath` (get, put, post, patch, delete, trace, options, head)
pub fn get_all_operations_in_a_path(p: &PathItem) -> Vec<Operation> {
    let d = p.read();
    [&d.get, &d.put, &d.post, &d.patch, &d.delete, &d.trace, &d.options, &d.head].into_iter().flatten().cloned().collect()
}

/// swagger-core `RefUtils.extractSimpleName(ref)` → (simple, prefix)
pub fn extract_simple_name(r: &str) -> (String, Option<String>) {
    if let Some(idx) = r.rfind('/') && idx > 0 {
        let simple = &r[idx + 1..];
        if !is_blank(Some(simple)) {
            return (simple.to_string(), Some(r[..=idx].to_string()));
        }
    }
    (r.to_string(), None)
}

impl PathItem {
    pub(crate) fn set_put_(&self, o: Option<Operation>) {
        self.set_put(o)
    }
    pub(crate) fn set_post_(&self, o: Option<Operation>) {
        self.set_post(o)
    }
    pub(crate) fn set_head_(&self, o: Option<Operation>) {
        self.set_head(o)
    }
    pub(crate) fn set_delete_(&self, o: Option<Operation>) {
        self.set_delete(o)
    }
    pub(crate) fn set_patch_(&self, o: Option<Operation>) {
        self.set_patch(o)
    }
    pub(crate) fn set_options_(&self, o: Option<Operation>) {
        self.set_options(o)
    }
    pub(crate) fn set_trace_(&self, o: Option<Operation>) {
        self.set_trace(o)
    }
}

#[allow(dead_code)]
fn unused_imports() {
    let _: Option<IndexMap<String, String>> = None;
}
