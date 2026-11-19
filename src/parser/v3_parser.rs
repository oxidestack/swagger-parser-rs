//! `io.swagger.v3.parser.OpenAPIV3Parser`

use crate::core::{AuthorizationValue, ParseOptions, SwaggerParseResult, SwaggerParserExtension};
use crate::models::OpenAPI;
use crate::java::{JResult, JavaException};
use crate::parser::deserialization_utils::{deserialize_into_tree, is_json, original_message, read_json_strict};
use crate::parser::deserializer::OpenAPIDeserializer;
use jackson::JsonNode;
use jackson::yaml::{YamlReadFeatures, read_tree as read_yaml};

/// Port of `io.swagger.v3.parser.OpenAPIV3Parser`: reads, parses and optionally resolves OpenAPI 3.x documents.
#[derive(Clone, Copy, Debug, Default)]
pub struct OpenAPIV3Parser;

impl OpenAPIV3Parser {
    /// `new OpenAPIV3Parser()`.
    pub fn new() -> OpenAPIV3Parser {
        OpenAPIV3Parser
    }

    /// `parseJsonNode(path, node)` (default options)
    pub fn parse_json_node(&self, path: Option<&str>, node: &JsonNode) -> JResult<SwaggerParseResult> {
        OpenAPIDeserializer::new().try_deserialize_with(node, path, &ParseOptions::default(), false)
    }

    /// `parseJsonNode(path, node, options)`
    pub fn parse_json_node_with(&self, path: Option<&str>, node: &JsonNode, options: &ParseOptions) -> JResult<SwaggerParseResult> {
        OpenAPIDeserializer::new().try_deserialize_with(node, path, options, options.oai_author)
    }

    /// `readContents(yaml)` (resolve = true)
    pub fn read_contents_simple(&self, yaml: &str) -> SwaggerParseResult {
        let options = ParseOptions { resolve: true, ..Default::default() };
        self.read_contents(yaml, None, Some(&options))
    }

    /// `readContents(swaggerAsString, auth, options)`
    pub fn read_contents(&self, s: &str, auth: Option<&[AuthorizationValue]>, options: Option<&ParseOptions>) -> SwaggerParseResult {
        self.read_contents_at(s, auth, options, None)
    }

    /// `readContents(swaggerAsString, auth, options, location)`
    pub fn read_contents_at(&self, s: &str, auth: Option<&[AuthorizationValue]>, options: Option<&ParseOptions>, location: Option<&str>) -> SwaggerParseResult {
        if s.trim().is_empty() {
            return SwaggerParseResult::of_error("Null or empty definition");
        }
        let outcome = (|| -> JResult<SwaggerParseResult> {
            let mut du_result = SwaggerParseResult::new();
            let root = if options.is_some_and(|o| o.legacy_yaml_deserialization) {
                right_mapper_read(s)?
            } else {
                match deserialize_into_tree(s, location, options, &mut du_result) {
                    Ok(n) => n,
                    Err(_) => right_mapper_read(s)?,
                }
            };
            let mut result = match options {
                Some(o) => self.parse_json_node_with(location, &root, o)?,
                None => self.parse_json_node(location, &root)?,
            };
            if result.openapi.is_some() {
                result = self.resolve(result, auth, options, location)?;
            }
            if let Some(msgs) = &du_result.messages {
                for m in msgs {
                    result.message(get_parse_error_message(Some(m), location));
                }
            }
            Ok(result)
        })();
        match outcome {
            Ok(r) => r,
            Err(e) => {
                let msg = if e.class.starts_with("com.fasterxml.jackson") {
                    // JsonProcessingException: getOriginalMessage()
                    let je = jackson::JacksonError { class: e.class.clone(), message: e.message.clone().unwrap_or_default() };
                    get_parse_error_message(Some(&original_message(&je)), location)
                } else {
                    get_parse_error_message(e.get_message(), location)
                };
                SwaggerParseResult::of_error(msg)
            }
        }
    }

    fn resolve(&self, result: SwaggerParseResult, auth: Option<&[AuthorizationValue]>, options: Option<&ParseOptions>, location: Option<&str>) -> JResult<SwaggerParseResult> {
        crate::parser::resolve::resolve_result(result, auth, options, location)
    }

    /// `getExtensions()`: this parser first, then the `SwaggerParserExtension`s on the class path (none in
    /// the v3 module; see `crate::parser_extensions` for the full list with the v2 converter).
    pub fn get_extensions() -> Vec<Box<dyn SwaggerParserExtension>> {
        vec![Box::new(OpenAPIV3Parser)]
    }

    /// `getEncoding()`
    pub fn get_encoding() -> String {
        crate::parser::remote::get_encoding()
    }

    /// `setEncoding(encoding)` (process-wide, as the Java static)
    pub fn set_encoding(&self, encoding: &str) -> JResult<()> {
        crate::parser::remote::set_encoding(encoding)
    }

    /// `read(location)` (resolve = true)
    pub fn read(&self, location: &str) -> Option<OpenAPI> {
        let options = ParseOptions { resolve: true, ..Default::default() };
        self.read_with(Some(location), None, Some(&options))
    }

    /// `read(location, auths, options)`: the first extension that yields a model wins.
    pub fn read_with(&self, location: Option<&str>, auths: Option<&[AuthorizationValue]>, options: Option<&ParseOptions>) -> Option<OpenAPI> {
        let location = location?;
        for ext in OpenAPIV3Parser::get_extensions() {
            let parsed = ext.read_location(location, auths, options);
            if let Some(o) = parsed.openapi {
                return Some(o);
            }
        }
        None
    }

    /// `readLocation(url, auth, options)`
    pub fn read_location(&self, url: &str, auth: Option<&[AuthorizationValue]>, options: Option<&ParseOptions>) -> SwaggerParseResult {
        match crate::parser::remote::read_content_from_location(url, auth.unwrap_or(&[])) {
            Ok(content) => self.read_contents_at(&content, auth, options, Some(url)),
            Err(e) => {
                let mut r = SwaggerParseResult::new();
                r.messages = Some(vec![e.get_message().map(str::to_string).unwrap_or_else(|| "null".into())]);
                r
            }
        }
    }
}

impl SwaggerParserExtension for OpenAPIV3Parser {
    fn read_location(&self, url: &str, auth: Option<&[AuthorizationValue]>, options: Option<&ParseOptions>) -> SwaggerParseResult {
        OpenAPIV3Parser::read_location(self, url, auth, options)
    }

    fn read_contents(&self, s: &str, auth: Option<&[AuthorizationValue]>, options: Option<&ParseOptions>) -> SwaggerParseResult {
        OpenAPIV3Parser::read_contents(self, s, auth, options)
    }
}

/// `getRightMapper(data).readTree(data)`: swagger-parser's strict JSON or YAML mapper.
fn right_mapper_read(s: &str) -> JResult<JsonNode> {
    if is_json(s) {
        read_json_strict(s).map_err(JavaException::from)
    } else {
        read_yaml(s, YamlReadFeatures::swagger_parser()).map_err(JavaException::from)
    }
}

/// `getParseErrorMessage`
pub fn get_parse_error_message(original: Option<&str>, location: Option<&str>) -> String {
    let loc = location.unwrap_or("null");
    match original {
        None => format!("Unable to parse `{loc}`"),
        Some(m) if m.starts_with("Duplicate field") => format!("{m} in `{loc}`"),
        Some(m) => m.to_string(),
    }
}
