//! swagger-parser 2.1.48 (the `swagger-parser` module): `io.swagger.parser.OpenAPIParser`, which tries every
//! `SwaggerParserExtension` on the class path: `OpenAPIV3Parser`, then the v2 converter.

use crate::converter::SwaggerConverter;
use crate::core::{AuthorizationValue, ParseOptions, SwaggerParseResult, SwaggerParserExtension};
use crate::java::JResult;
use crate::parser::OpenAPIV3Parser;

/// `OpenAPIV3Parser.getExtensions()` with the whole swagger-parser distribution on the class path:
/// `OpenAPIV3Parser` first, then `SwaggerConverter` (registered through `META-INF/services`).
pub fn parser_extensions() -> Vec<Box<dyn SwaggerParserExtension>> {
    vec![Box::new(OpenAPIV3Parser::new()), Box::new(SwaggerConverter::new())]
}

/// `io.swagger.parser.OpenAPIParser`
#[derive(Clone, Copy, Debug, Default)]
pub struct OpenAPIParser;

impl OpenAPIParser {
    /// `new OpenAPIParser()`.
    pub fn new() -> OpenAPIParser {
        OpenAPIParser
    }

    /// `readLocation(url, auth, options)`: the first extension that yields an OpenAPI wins, otherwise the
    /// last extension's result. `Err` where an exception escapes in Java, `Ok(None)` where Java returns null.
    pub fn try_read_location(&self, url: &str, auth: Option<&[AuthorizationValue]>, options: Option<&ParseOptions>) -> JResult<Option<SwaggerParseResult>> {
        let mut output = None;
        for ext in parser_extensions() {
            output = ext.try_read_location(url, auth, options)?;
            if output.as_ref().is_some_and(|o| o.openapi.is_some()) {
                return Ok(output);
            }
        }
        Ok(output)
    }

    /// `readContents(swaggerAsString, auth, options)`
    pub fn try_read_contents(&self, s: &str, auth: Option<&[AuthorizationValue]>, options: Option<&ParseOptions>) -> JResult<Option<SwaggerParseResult>> {
        let mut output = None;
        for ext in parser_extensions() {
            output = ext.try_read_contents(s, auth, options)?;
            if output.as_ref().is_some_and(|o| o.openapi.is_some()) {
                return Ok(output);
            }
        }
        Ok(output)
    }

    /// `readLocation` with exceptions reported as a single message (a null result becomes an empty one).
    pub fn read_location(&self, url: &str, auth: Option<&[AuthorizationValue]>, options: Option<&ParseOptions>) -> SwaggerParseResult {
        match self.try_read_location(url, auth, options) {
            Ok(r) => r.unwrap_or_default(),
            Err(e) => SwaggerParseResult::of_error(e.get_message().unwrap_or("null")),
        }
    }

    /// `readContents` with exceptions reported as a single message (a null result becomes an empty one).
    pub fn read_contents(&self, s: &str, auth: Option<&[AuthorizationValue]>, options: Option<&ParseOptions>) -> SwaggerParseResult {
        match self.try_read_contents(s, auth, options) {
            Ok(r) => r.unwrap_or_default(),
            Err(e) => SwaggerParseResult::of_error(e.get_message().unwrap_or("null")),
        }
    }
}
