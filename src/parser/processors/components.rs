//! `ComponentsProcessor`

use super::mocks;
use super::*;
use crate::java::{JMap, JResult};
use crate::models::*;
use crate::parser::resolver_cache::Cache;
use std::rc::Rc;

/// Port of `io.swagger.v3.parser.processors.ComponentsProcessor`: resolves the references inside every entry of `components`.
pub struct ComponentsProcessor {
    cache: Rc<dyn Cache>,
    open_api: OpenAPI,
    schema_processor: SchemaProcessor,
    response_processor: ResponseProcessor,
    request_body_processor: RequestBodyProcessor,
    parameter_processor: ParameterProcessor,
    header_processor: HeaderProcessor,
    example_processor: ExampleProcessor,
    link_processor: LinkProcessor,
    callback_processor: CallbackProcessor,
    security_scheme_processor: SecuritySchemeProcessor,
}

/// `while (map.keySet().size() > keySet.size()) process(keySet, map)`: `keySet` is a `LinkedHashSet`
/// accumulating the keys seen so far; each round processes all of them again.
fn rounds<V: Clone>(map: &JMap<V>, mut f: impl FnMut(&[String]) -> JResult<()>) -> JResult<()> {
    let mut key_set: Vec<String> = Vec::new();
    while map.len() > key_set.len() {
        for k in map.keys() {
            if !key_set.contains(&k) {
                key_set.push(k);
            }
        }
        f(&key_set)?;
    }
    Ok(())
}

impl ComponentsProcessor {
    /// `new ComponentsProcessor(openAPI, cache)`: the OpenAPI 3.0 processor.
    pub fn new(open_api: &OpenAPI, cache: Rc<dyn Cache>) -> ComponentsProcessor {
        ComponentsProcessor::new31(open_api, cache, false)
    }

    /// `new ComponentsProcessor(openAPI, cache, openapi31)`.
    pub fn new31(open_api: &OpenAPI, cache: Rc<dyn Cache>, openapi31: bool) -> ComponentsProcessor {
        let _constructing = mocks::constructed("ComponentsProcessor");
        ComponentsProcessor {
            schema_processor: SchemaProcessor::new31(cache.clone(), open_api, openapi31),
            response_processor: ResponseProcessor::new31(cache.clone(), open_api, openapi31),
            request_body_processor: RequestBodyProcessor::new31(cache.clone(), open_api, openapi31),
            parameter_processor: ParameterProcessor::new31(cache.clone(), open_api, openapi31),
            header_processor: HeaderProcessor::new31(cache.clone(), open_api, openapi31),
            example_processor: ExampleProcessor::new(cache.clone(), open_api),
            link_processor: LinkProcessor::new31(cache.clone(), open_api, openapi31),
            callback_processor: CallbackProcessor::new31(cache.clone(), open_api, openapi31),
            security_scheme_processor: SecuritySchemeProcessor::new(cache.clone(), open_api),
            cache,
            open_api: open_api.clone(),
        }
    }

    /// `processComponents()`
    pub fn process_components(&self) -> JResult<()> {
        let Some(c) = self.open_api.get_components() else { return Ok(()) };
        let responses = c.get_responses();
        let request_bodies = c.get_request_bodies();
        let parameters = c.get_parameters();
        let headers = c.get_headers();
        let examples = c.get_examples();
        let links = c.get_links();
        let callbacks = c.get_callbacks();
        let security_schemes = c.get_security_schemes();

        if let Some(m) = &responses {
            rounds(m, |keys| {
                for k in keys {
                    match m.get(k) {
                        Some(v) => self.response_processor.process_response(&v)?,
                        None => return Err(npe_get_ref("io.swagger.v3.oas.models.responses.ApiResponse", "response")),
                    }
                }
                Ok(())
            })?;
        }
        if let Some(m) = &request_bodies {
            rounds(m, |keys| {
                for k in keys {
                    match m.get(k) {
                        Some(v) => self.request_body_processor.process_request_body(&v)?,
                        None => return Err(npe_get_ref("io.swagger.v3.oas.models.parameters.RequestBody", "requestBody")),
                    }
                }
                Ok(())
            })?;
        }
        if let Some(m) = &parameters {
            rounds(m, |keys| {
                for k in keys {
                    match m.get(k) {
                        Some(v) => self.parameter_processor.process_parameter(&v)?,
                        None => return Err(npe_get_ref("io.swagger.v3.oas.models.parameters.Parameter", "parameter")),
                    }
                }
                Ok(())
            })?;
        }
        if let Some(m) = &headers {
            rounds(m, |keys| {
                for k in keys {
                    match m.get(k) {
                        Some(v) => self.header_processor.process_header(&v)?,
                        None => return Err(npe_get_ref("io.swagger.v3.oas.models.headers.Header", "header")),
                    }
                }
                Ok(())
            })?;
        }
        if let Some(m) = &examples {
            rounds(m, |keys| {
                for k in keys {
                    match m.get(k) {
                        Some(v) => self.example_processor.process_example(&v)?,
                        None => return Err(npe_get_ref("io.swagger.v3.oas.models.examples.Example", "example")),
                    }
                }
                Ok(())
            })?;
        }
        if let Some(m) = &links {
            rounds(m, |keys| {
                for k in keys {
                    match m.get(k) {
                        Some(v) => self.link_processor.process_link(&v)?,
                        None => return Err(npe_get_ref("io.swagger.v3.oas.models.links.Link", "link")),
                    }
                }
                Ok(())
            })?;
        }
        if let Some(m) = &callbacks {
            rounds(m, |keys| {
                for k in keys {
                    match m.get(k) {
                        Some(v) => self.callback_processor.process_callback(&v)?,
                        None => return Err(npe_get_ref("io.swagger.v3.oas.models.callbacks.Callback", "callback")),
                    }
                }
                Ok(())
            })?;
        }
        if let Some(m) = &security_schemes {
            rounds(m, |keys| {
                for k in keys {
                    match m.get(k) {
                        Some(v) => {
                            let resolved = self.security_scheme_processor.process_security_scheme(&v)?;
                            // Map.replace(key, oldValue, newValue): only while the key still maps to the old value
                            if m.get(k).is_some_and(|cur| cur.ptr_eq(&v) || cur == v) {
                                m.put(k.clone(), resolved);
                            }
                        }
                        None => return Err(npe_get_ref("io.swagger.v3.oas.models.security.SecurityScheme", "securityScheme")),
                    }
                }
                Ok(())
            })?;
        }
        let schemas = self.open_api.get_components().and_then(|c| c.get_schemas());
        if let Some(m) = &schemas {
            rounds(m, |keys| self.process_schemas(keys, m))?;
        }
        Ok(())
    }

    /// `processSchemas(schemaKeys, schemas)` (the key set is already extended with `schemas.keySet()`)
    pub fn process_schemas(&self, schema_keys: &[String], schemas: &JMap<Schema>) -> JResult<()> {
        for model_name in schema_keys {
            let Some(model) = schemas.get(model_name) else {
                return Err(npe_get_ref("io.swagger.v3.oas.models.media.Schema", "model"));
            };
            let original_ref = model.get_ref();
            self.schema_processor.process_schema(Some(&model))?;
            if model.get_ref().is_some() && let Some(renamed) = self.cache.get_renamed_ref(original_ref.as_deref()) {
                let resolved = schemas.get(&renamed);
                if !self.cache.has_referenced_key(&renamed) {
                    schemas.remove(&renamed);
                }
                match resolved {
                    Some(r) => {
                        schemas.put(model_name.clone(), r);
                    }
                    // schemas.put(modelName, null)
                    None => {
                        schemas.put_null_value(model_name);
                    }
                }
            }
        }
        for model_name in schema_keys {
            let Some(model) = schemas.get(model_name) else {
                return Err(crate::java::JavaException::npe(
                    "Cannot invoke \"io.swagger.v3.oas.models.media.Schema.getProperties()\" because \"model\" is null",
                ));
            };
            if let Some(props) = model.get_properties() {
                for (_, property) in props.entries() {
                    if property.get_ref().is_some() {
                        self.schema_processor.process_schema(Some(&model))?;
                    }
                }
            }
        }
        Ok(())
    }
}

fn npe_get_ref(class: &str, var: &str) -> crate::java::JavaException {
    crate::java::JavaException::npe(format!("Cannot invoke \"{class}.get$ref()\" because \"{var}\" is null"))
}
