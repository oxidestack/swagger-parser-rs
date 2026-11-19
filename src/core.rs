//! swagger-parser-core: `ParseOptions`, `SwaggerParseResult`, `AuthorizationValue` and the
//! `SwaggerParserExtension` interface.

use crate::models::OpenAPI;
use std::sync::Arc;

/// `ParseOptions` (field names follow the Java properties; defaults are Java's).
#[derive(Clone, Debug)]
pub struct ParseOptions {
    /// `resolve`: resolve `$ref`s to other documents.
    pub resolve: bool,
    /// `resolveCombinators`: with `resolve_fully`, merge `allOf` into one schema.
    pub resolve_combinators: bool,
    /// `resolveFully`: replace every `$ref` with what it points at.
    pub resolve_fully: bool,
    /// `flatten`: move inline schemas into `components` (`InlineModelResolver`).
    pub flatten: bool,
    /// `flattenComposedSchemas`: flatten the parts of composed schemas too.
    pub flatten_composed_schemas: bool,
    /// `camelCaseFlattenNaming`: name flattened schemas in camel case.
    pub camel_case_flatten_naming: bool,
    /// `skipMatches`: give flattened schemas that look alike their own names.
    pub skip_matches: bool,
    /// `allowEmptyStrings` (`isAllowEmptyString()`)
    pub allow_empty_strings: bool,
    /// `validateExternalRefs`: report external references that cannot be resolved.
    pub validate_external_refs: bool,
    /// `validateInternalRefs`: report internal references that point nowhere.
    pub validate_internal_refs: bool,
    /// `legacyYamlDeserialization`: the older YAML deserialization swagger-parser keeps for compatibility.
    pub legacy_yaml_deserialization: bool,
    /// `resolveRequestBody`: resolve request bodies' references fully.
    pub resolve_request_body: bool,
    /// `resolveResponses`: resolve responses' references fully.
    pub resolve_responses: bool,
    /// `oaiAuthor`: the deserializer's OAI author mode.
    pub oai_author: bool,
    /// `inferSchemaType`: give a schema without `type` the one its keywords imply.
    pub infer_schema_type: bool,
    /// `safelyResolveURL`: check remote reference URLs against the allow and block lists and restricted IPs.
    pub safely_resolve_url: bool,
    /// `remoteRefAllowList`: the hosts a remote reference may use.
    pub remote_ref_allow_list: Option<Vec<String>>,
    /// `remoteRefBlockList`: the hosts a remote reference may not use.
    pub remote_ref_block_list: Option<Vec<String>>,
    /// `explicitStyleAndExplode`, swagger-parser's option of that name (default `true`).
    pub explicit_style_and_explode: bool,
    /// `explicitObjectSchema`, swagger-parser's option of that name (default `true`).
    pub explicit_object_schema: bool,
}

impl Default for ParseOptions {
    fn default() -> Self {
        ParseOptions {
            resolve: false,
            resolve_combinators: true,
            resolve_fully: false,
            flatten: false,
            flatten_composed_schemas: false,
            camel_case_flatten_naming: false,
            skip_matches: false,
            allow_empty_strings: true,
            validate_external_refs: false,
            validate_internal_refs: true,
            legacy_yaml_deserialization: false,
            resolve_request_body: false,
            resolve_responses: false,
            oai_author: false,
            infer_schema_type: true,
            safely_resolve_url: false,
            remote_ref_allow_list: None,
            remote_ref_block_list: None,
            explicit_style_and_explode: true,
            explicit_object_schema: true,
        }
    }
}

impl ParseOptions {
    /// Java's defaults.
    pub fn new() -> ParseOptions {
        ParseOptions::default()
    }
}

/// `SwaggerParseResult`
#[derive(Clone, Debug, Default)]
pub struct SwaggerParseResult {
    /// `getMessages()`: what parsing reported, `None` when nothing.
    pub messages: Option<Vec<String>>,
    /// `getOpenAPI()`: the document, `None` when it could not be read.
    pub openapi: Option<OpenAPI>,
    /// `isOpenapi31()`: whether the document is OpenAPI 3.1.
    pub openapi31: bool,
}

impl SwaggerParseResult {
    /// An empty result.
    pub fn new() -> SwaggerParseResult {
        SwaggerParseResult::default()
    }

    /// `message(String)`: appends, creating the list
    pub fn message(&mut self, m: impl Into<String>) -> &mut Self {
        self.messages.get_or_insert_with(Vec::new).push(m.into());
        self
    }

    /// `addMessages(List)`
    pub fn add_messages(&mut self, m: &[String]) -> &mut Self {
        self.messages.get_or_insert_with(Vec::new).extend(m.iter().cloned());
        self
    }

    /// `getMessages()`
    pub fn get_messages(&self) -> Option<&Vec<String>> {
        self.messages.as_ref()
    }

    /// `getOpenAPI()`.
    pub fn get_openapi(&self) -> Option<OpenAPI> {
        self.openapi.clone()
    }

    /// `SwaggerParseResult.ofError(message)`
    pub fn of_error(message: impl Into<String>) -> SwaggerParseResult {
        SwaggerParseResult { messages: Some(vec![message.into()]), ..Default::default() }
    }
}

/// `Predicate<URL>` of an [`AuthorizationValue`]: decides which URLs get the credential.
pub type UrlMatcher = Arc<dyn Fn(&crate::java::net::JavaUrl) -> bool + Send + Sync>;

/// `AuthorizationValue`: a header or query parameter to add when fetching remote references.
#[derive(Clone)]
pub struct AuthorizationValue {
    /// `getValue()`: the credential.
    pub value: Option<String>,
    /// `"header"` or `"query"`
    pub type_: Option<String>,
    /// `getKeyName()`: the header's or query parameter's name.
    pub key_name: Option<String>,
    /// `getUrlMatcher()`: which URLs get the credential.
    pub url_matcher: UrlMatcher,
}

impl std::fmt::Debug for AuthorizationValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthorizationValue").field("value", &self.value).field("type", &self.type_).field("keyName", &self.key_name).finish()
    }
}

impl Default for AuthorizationValue {
    fn default() -> Self {
        AuthorizationValue { value: None, type_: None, key_name: None, url_matcher: Arc::new(|_| true) }
    }
}

impl AuthorizationValue {
    /// `new AuthorizationValue(keyName, value, type)` (matches every URL)
    pub fn new(key_name: &str, value: &str, type_: &str) -> AuthorizationValue {
        AuthorizationValue { value: Some(value.into()), type_: Some(type_.into()), key_name: Some(key_name.into()), url_matcher: Arc::new(|_| true) }
    }

    /// `new AuthorizationValue(keyName, value, type, urlMatcher)`
    pub fn with_matcher(key_name: &str, value: &str, type_: &str, m: UrlMatcher) -> AuthorizationValue {
        AuthorizationValue { value: Some(value.into()), type_: Some(type_.into()), key_name: Some(key_name.into()), url_matcher: m }
    }
}

/// `SwaggerParserExtension`
pub trait SwaggerParserExtension: Send + Sync {
    /// `readLocation(url, auth, options)`: the document at `url`, a URL or a file path.
    fn read_location(&self, url: &str, auth: Option<&[AuthorizationValue]>, options: Option<&ParseOptions>) -> SwaggerParseResult;
    /// `readContents(swaggerAsString, auth, options)`: the document from its text, JSON or YAML.
    fn read_contents(&self, swagger_as_string: &str, auth: Option<&[AuthorizationValue]>, options: Option<&ParseOptions>) -> SwaggerParseResult;

    /// `readLocation` where an exception escapes the Java method (`Err`), or it returns null (`Ok(None)`).
    fn try_read_location(&self, url: &str, auth: Option<&[AuthorizationValue]>, options: Option<&ParseOptions>) -> crate::java::JResult<Option<SwaggerParseResult>> {
        Ok(Some(self.read_location(url, auth, options)))
    }

    /// `readContents` where an exception escapes the Java method (`Err`), or it returns null (`Ok(None)`).
    fn try_read_contents(&self, swagger_as_string: &str, auth: Option<&[AuthorizationValue]>, options: Option<&ParseOptions>) -> crate::java::JResult<Option<SwaggerParseResult>> {
        Ok(Some(self.read_contents(swagger_as_string, auth, options)))
    }
}

/// Jackson's bean serialization of `SwaggerParseResult` (e.g. `Yaml.pretty(result)`): the properties
/// `messages`, `openAPI` and `openapi31` in declaration order, nulls left out (the swagger-core mappers
/// use `Include.NON_NULL`).
impl crate::models::Serialize for SwaggerParseResult {
    fn serialize_with(&self, g: &mut dyn jackson::JsonGenerator, m: crate::models::Mapper) -> Result<(), jackson::JacksonError> {
        g.write_start_object()?;
        crate::models::write_bean_properties(g, |g| {
            if let Some(messages) = &self.messages {
                g.write_field_name("messages")?;
                g.write_start_array()?;
                for message in messages {
                    g.write_string(message)?;
                }
                g.write_end_array()?;
            }
            if let Some(openapi) = &self.openapi {
                g.write_field_name("openAPI")?;
                openapi.serialize(g, m)?;
            }
            g.write_field_name("openapi31")?;
            g.write_boolean(self.openapi31)
        })?;
        g.write_end_object()
    }
}
