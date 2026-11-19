//! `io.swagger.parser.SwaggerParser` and `io.swagger.parser.Swagger20Parser` (swagger-parser 1.0.76), with
//! the parts of its `DeserializationUtils` they use.

use super::deserializer::SwaggerDeserializer;
use super::models::*;
use super::resolver::SwaggerResolver;
use crate::core::AuthorizationValue;
use crate::java::path::JavaPath;
use crate::java::{JResult, JavaException};
use crate::parser::deserialization_utils::read_yaml_tree_v1;
use jackson::JsonNode;
use jackson::json::{read_tree as read_json_tree, ReadFeatures};

/// swagger-core 1.x `Json.mapper().readTree(text)`
pub fn read_json(text: &str) -> JResult<JsonNode> {
    read_json_tree(text, ReadFeatures::default()).map_err(JavaException::from)
}

/// `DeserializationUtils.readYamlTree(contents, errorOutput)`
pub fn read_yaml(text: &str, error_output: Option<&mut SwaggerDeserializationResult>) -> JResult<JsonNode> {
    let mut messages = Vec::new();
    let r = read_yaml_tree_v1(text, &mut messages);
    if let Some(out) = error_output {
        for m in messages {
            out.message(m);
        }
    }
    r
}

/// `DeserializationUtils.deserializeIntoTree(contents, fileOrHost)`
pub fn deserialize_into_tree(contents: &str, file_or_host: &str) -> JResult<JsonNode> {
    let r = if is_json(contents) { read_json(contents) } else { read_yaml(contents, None) };
    r.map_err(|e| {
        JavaException::new("java.lang.RuntimeException", format!("An exception was thrown while trying to deserialize the contents of {file_or_host} into a JsonNode tree")).with_cause(e)
    })
}

/// `isJson(contents)`
pub fn is_json(contents: &str) -> bool {
    contents.trim().starts_with('{')
}

/// `String.trim().startsWith("{")` on the whole document
fn looks_like_json(data: &str) -> bool {
    data.trim_matches(|c: char| c <= ' ').starts_with('{')
}

/// `location.replaceAll("\\\\", "/")`
fn slashes(location: &str) -> String {
    location.replace('\\', "/")
}

/// Reads a location the way `Swagger20Parser` does: `http…` through `RemoteUrl`, otherwise a file, falling
/// back to the class path.
pub fn read_location_text(location: &str, auths: &[AuthorizationValue]) -> JResult<String> {
    if location.to_lowercase().starts_with("http") {
        return crate::parser::remote_url::url_to_string(location, auths, None);
    }
    let path = if location.to_lowercase().starts_with("file:") {
        let uri = crate::parser::path_utils::uri_create(location)?;
        crate::parser::path_utils::path_of_uri(&uri)?
    } else {
        JavaPath::new(location)
    };
    if path.exists() {
        return path.read_to_string().map_err(|e| JavaException::new("java.io.IOException", e.to_string()));
    }
    crate::parser::ref_utils::load_file_from_classpath(location)
}

/// `io.swagger.parser.Swagger20Parser`
#[derive(Clone, Copy, Debug, Default)]
pub struct Swagger20Parser;

impl Swagger20Parser {
    /// `readWithInfo(JsonNode)`
    pub fn read_with_info_node(&self, node: &JsonNode) -> JResult<SwaggerDeserializationResult> {
        SwaggerDeserializer::new().deserialize(node)
    }

    /// `readWithInfo(location, auths)`
    pub fn read_with_info(&self, location: &str, auths: &[AuthorizationValue]) -> SwaggerDeserializationResult {
        let mut error_output = SwaggerDeserializationResult::default();
        let location = slashes(location);
        let attempt = (|| -> JResult<SwaggerDeserializationResult> {
            let data = read_location_text(&location, auths)?;
            let root = if looks_like_json(&data) { read_json(&data)? } else { read_yaml(&data, Some(&mut error_output))? };
            self.read_with_info_node(&root)
        })();
        match attempt {
            Ok(r) => r,
            Err(e) => {
                if e.class == "javax.net.ssl.SSLHandshakeException" {
                    error_output.message(format!(
                        "unable to read location `{location}` due to a SSL configuration error.  It is possible that the server SSL certificate is invalid, self-signed, or has an untrusted Certificate Authority."
                    ));
                } else {
                    error_output.message(format!("unable to read location `{location}`"));
                }
                error_output
            }
        }
    }
}

/// A Swagger 1.x `SwaggerParserExtension` found by `ServiceLoader` (swagger-parser's `Swagger20Parser`,
/// then compat-spec-parser's `SwaggerCompatConverter`).
#[derive(Clone, Copy, Debug)]
enum V1Extension {
    Swagger20,
    Compat,
}

const EXTENSIONS: [V1Extension; 2] = [V1Extension::Swagger20, V1Extension::Compat];

impl V1Extension {
    fn read_with_info(&self, location: &str, auths: &[AuthorizationValue]) -> JResult<Option<SwaggerDeserializationResult>> {
        match self {
            V1Extension::Swagger20 => Ok(Some(Swagger20Parser.read_with_info(location, auths))),
            V1Extension::Compat => crate::compat::SwaggerCompatConverter::new().read_with_info(location, auths).map(Some),
        }
    }
}

fn is_swagger_20(r: &SwaggerDeserializationResult) -> bool {
    r.swagger.as_ref().is_some_and(|s| s.borrow().swagger.as_deref() == Some("2.0"))
}

/// `io.swagger.parser.SwaggerParser`
#[derive(Clone, Copy, Debug, Default)]
pub struct SwaggerParser;

impl SwaggerParser {
    /// `new SwaggerParser()`.
    pub fn new() -> SwaggerParser {
        SwaggerParser
    }

    /// `readWithInfo(location, auths, resolve)`
    pub fn read_with_info(&self, location: Option<&str>, auths: &[AuthorizationValue], resolve: bool) -> Option<SwaggerDeserializationResult> {
        let location = slashes(location?);
        let mut results: Vec<Option<SwaggerDeserializationResult>> = Vec::new();
        let mut output: Option<SwaggerDeserializationResult> = Some(SwaggerDeserializationResult::default());
        let attempt = (|| -> JResult<Option<SwaggerDeserializationResult>> {
            output = Some(Swagger20Parser.read_with_info(&location, auths));
            if output.as_ref().is_some_and(is_swagger_20) {
                if resolve {
                    let swagger = output.as_ref().unwrap().swagger.clone();
                    let resolved = SwaggerResolver::new(swagger, auths.to_vec(), Some(&location)).resolve()?;
                    output.as_mut().unwrap().swagger = resolved;
                }
                return Ok(output.clone());
            }
            for ext in EXTENSIONS {
                output = ext.read_with_info(&location, auths)?;
                if output.as_ref().is_some_and(is_swagger_20) {
                    return Ok(output.clone());
                }
                results.push(output.clone());
            }
            if output.is_none() {
                let mut o = SwaggerDeserializationResult::default();
                o.message("The swagger definition could not be read");
                output = Some(o);
            }
            Ok(None)
        })();
        match attempt {
            Ok(Some(r)) => return Some(r),
            Ok(None) => {}
            Err(e) => {
                let o = output.as_mut()?;
                o.message(e.get_message().unwrap_or("null"));
            },
        }
        let mut output = output.unwrap();
        if output.messages.is_none() && output.swagger.is_none() {
            for r in results.iter().flatten() {
                if r.messages.is_some() && r.swagger.is_some() {
                    output.messages = r.messages.clone();
                }
            }
        }
        Some(output)
    }

    /// `readWithInfo(swaggerAsString, resolve)`
    pub fn read_with_info_contents(&self, swagger_as_string: Option<&str>, resolve: bool) -> SwaggerDeserializationResult {
        let s = match swagger_as_string {
            Some(s) if !s.trim_matches(|c: char| c <= ' ').is_empty() => s,
            _ => {
                let mut r = SwaggerDeserializationResult::default();
                r.message("empty or null swagger supplied");
                return r;
            }
        };
        let attempt = (|| -> JResult<SwaggerDeserializationResult> {
            let node = if looks_like_json(s) { read_json(s)? } else { read_yaml(s, None)? };
            let mut result = Swagger20Parser.read_with_info_node(&node)?;
            if resolve {
                result.swagger = SwaggerResolver::new(result.swagger.clone(), Vec::new(), None).resolve()?;
            }
            Ok(result)
        })();
        match attempt {
            Ok(r) => r,
            Err(_) => {
                let mut r = SwaggerDeserializationResult::default();
                r.message("malformed or unreadable swagger supplied");
                r
            }
        }
    }
}
