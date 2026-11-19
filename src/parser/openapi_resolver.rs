//! `io.swagger.v3.parser.OpenAPIResolver`: the OpenAPI 3.0 resolution pass.

use crate::core::{AuthorizationValue, ParseOptions, SwaggerParseResult};
use crate::java::{JResult, JSet};
use crate::models::OpenAPI;
use crate::parser::processors::{ComponentsProcessor, OperationProcessor, PathsProcessor};
use crate::parser::resolver_cache::{Cache, ResolverCache};
use std::rc::Rc;

/// `OpenAPIResolver.Settings`
#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// If true, resource parameters are added to each operation
    pub add_parameters_to_each_operation: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { add_parameters_to_each_operation: true }
    }
}

impl Settings {
    /// `addParametersToEachOperation(boolean)`: sets the flag and returns the settings for chaining.
    pub fn add_parameters_to_each_operation(mut self, v: bool) -> Settings {
        self.add_parameters_to_each_operation = v;
        self
    }
}

/// Port of `io.swagger.v3.parser.OpenAPIResolver`: resolves the remote and relative `$ref`s of an OpenAPI 3.0 document, importing the referenced components.
pub struct OpenAPIResolver {
    open_api: Option<OpenAPI>,
    cache: Rc<ResolverCache>,
    components_processor: Option<ComponentsProcessor>,
    path_processor: Option<PathsProcessor>,
    operations_processor: Option<OperationProcessor>,
    resolve_validation_messages: JSet,
}

impl OpenAPIResolver {
    /// `new OpenAPIResolver(openApi)`
    pub fn new(open_api: Option<&OpenAPI>) -> JResult<OpenAPIResolver> {
        OpenAPIResolver::with_all(open_api, None, None, None, ParseOptions::default())
    }

    /// `new OpenAPIResolver(openApi, auths, parentFileLocation)`
    pub fn with_location(open_api: Option<&OpenAPI>, auths: Option<&[AuthorizationValue]>, parent_file_location: Option<&str>) -> JResult<OpenAPIResolver> {
        OpenAPIResolver::with_all(open_api, auths, parent_file_location, None, ParseOptions::default())
    }

    /// `new OpenAPIResolver(openApi, auths, parentFileLocation, settings, parseOptions)`
    pub fn with_all(
        open_api: Option<&OpenAPI>,
        auths: Option<&[AuthorizationValue]>,
        parent_file_location: Option<&str>,
        settings: Option<Settings>,
        parse_options: ParseOptions,
    ) -> JResult<OpenAPIResolver> {
        let settings = settings.unwrap_or_default();
        let messages = JSet::new_hash();
        let cache = Rc::new(ResolverCache::with_options(open_api, auths, parent_file_location, Some(messages.clone()), parse_options)?);
        let dyn_cache: Rc<dyn Cache> = cache.clone();
        // the processors dereference `openApi` only when used
        let (cp, pp, op) = match open_api {
            Some(api) => (
                Some(ComponentsProcessor::new(api, dyn_cache.clone())),
                Some(PathsProcessor::with_settings(dyn_cache.clone(), api, settings)),
                Some(OperationProcessor::new(dyn_cache.clone(), api)),
            ),
            None => (None, None, None),
        };
        Ok(OpenAPIResolver {
            open_api: open_api.cloned(),
            cache,
            components_processor: cp,
            path_processor: pp,
            operations_processor: op,
            resolve_validation_messages: messages,
        })
    }

    /// `getCache()`: the cache of loaded files and resolved references.
    pub fn get_cache(&self) -> &ResolverCache {
        &self.cache
    }

    /// `resolve(result)`
    pub fn resolve_result(&self, result: &mut SwaggerParseResult) -> JResult<()> {
        let Some(resolved) = self.resolve()? else { return Ok(()) };
        result.openapi = Some(resolved);
        let msgs = self.resolve_validation_messages.items();
        match &mut result.messages {
            Some(m) => m.extend(msgs),
            None => {
                return Err(crate::java::JavaException::npe(
                    "Cannot invoke \"java.util.List.addAll(java.util.Collection)\" because the return value of \"io.swagger.v3.parser.core.models.SwaggerParseResult.getMessages()\" is null",
                ));
            }
        }
        Ok(())
    }

    /// `resolve()`
    pub fn resolve(&self) -> JResult<Option<OpenAPI>> {
        let Some(api) = &self.open_api else { return Ok(None) };
        self.path_processor.as_ref().unwrap().process_paths()?;
        self.components_processor.as_ref().unwrap().process_components()?;
        if let Some(paths) = api.get_paths() {
            for name in paths.keys() {
                let Some(path_item) = paths.get(&name) else { continue };
                for op in path_item.read_operations() {
                    self.operations_processor.as_ref().unwrap().process_operation(&op)?;
                }
            }
        }
        Ok(Some(api.clone()))
    }
}
