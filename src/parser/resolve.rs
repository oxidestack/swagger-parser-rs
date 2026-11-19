//! `OpenAPIV3Parser.resolve`: dispatch to the 3.0 resolver or the 3.1 dereferencer, then `ResolverFully` and
//! `InlineModelResolver` as the options ask.

use crate::core::{AuthorizationValue, ParseOptions, SwaggerParseResult};
use crate::java::{JResult, JavaException};
use crate::parser::inline_model_resolver::InlineModelResolver;
use crate::parser::openapi_resolver::OpenAPIResolver;
use crate::parser::resolver_fully::ResolverFully;

/// The environment variable that, when set to a non-blank value, skips dereferencing OpenAPI 3.1 documents.
pub const DISABLE_OAS31_RESOLVE: &str = "disableOas31Resolve";

/// `resolve(result, auth, options, location)`: resolution failures become a message (`e.getMessage()`,
/// which may be `null`).
pub fn resolve_result(mut result: SwaggerParseResult, auth: Option<&[AuthorizationValue]>, options: Option<&ParseOptions>, location: Option<&str>) -> JResult<SwaggerParseResult> {
    let location = location.map(|l| l.replace('\\', "/"));
    if let Err(e) = resolve_inner(&mut result, auth, options, location.as_deref()) {
        match &mut result.messages {
            Some(m) => m.push(e.message.clone().unwrap_or_else(|| "null".to_string())),
            // result.getMessages().add(...) on a null list
            None => return Err(JavaException::npe("Cannot invoke \"java.util.List.add(Object)\" because the return value of \"io.swagger.v3.parser.core.models.SwaggerParseResult.getMessages()\" is null")),
        }
    }
    Ok(result)
}

fn resolve_inner(result: &mut SwaggerParseResult, auth: Option<&[AuthorizationValue]>, options: Option<&ParseOptions>, location: Option<&str>) -> JResult<()> {
    let Some(options) = options else { return Ok(()) };
    if options.resolve || options.resolve_fully {
        let api = result.openapi.clone().expect("resolve is called with a parsed OpenAPI");
        if api.get_openapi().is_some_and(|v| v.starts_with("3.1")) {
            if std::env::var(DISABLE_OAS31_RESOLVE).map(|v| v.trim().is_empty()).unwrap_or(true) {
                crate::parser::reference::dereference_31(result, auth, location, options)?;
                if options.resolve_fully {
                    ResolverFully::new(options).resolve_fully(result.openapi.as_ref())?;
                }
            } else {
                let msg = "Resolution of OAS 3.1 spec disabled by 'disableOas31Resolve' env variable";
                match &mut result.messages {
                    Some(m) => m.push(msg.to_string()),
                    None => return Err(JavaException::npe("Cannot invoke \"java.util.List.add(Object)\" because the return value of \"io.swagger.v3.parser.core.models.SwaggerParseResult.getMessages()\" is null")),
                }
            }
        } else {
            let resolver = OpenAPIResolver::with_all(Some(&api), Some(auth.unwrap_or(&[])), location, None, options.clone())?;
            resolver.resolve_result(result)?;
            if options.resolve_fully {
                ResolverFully::new(options).resolve_fully(result.openapi.as_ref())?;
            }
        }
    }
    if options.flatten {
        let r = InlineModelResolver::new(options.flatten_composed_schemas, options.camel_case_flatten_naming, options.skip_matches);
        if let Some(api) = &result.openapi {
            r.flatten(api)?;
        }
    }
    Ok(())
}
