//! `PathsProcessor`

use super::mocks;
use super::{reference_path_utils, ExternalRefProcessor, OperationProcessor, ParameterProcessor};
use crate::java::{JList, JResult, JavaException};
use crate::models::*;
use crate::parser::openapi_resolver::Settings;
use crate::parser::ref_utils::{compute_ref_format, is_an_external_ref_format, java_split};
use crate::parser::resolver_cache::{load_ref, Cache};
use std::rc::Rc;

/// Port of `io.swagger.v3.parser.processors.PathsProcessor`: resolves path item `$ref`s and the references in every path and its operations.
pub struct PathsProcessor {
    open_api: OpenAPI,
    cache: Rc<dyn Cache>,
    settings: Settings,
    parameter_processor: ParameterProcessor,
    operation_processor: OperationProcessor,
    external_ref_processor: ExternalRefProcessor,
}

impl PathsProcessor {
    /// `new PathsProcessor(cache, openAPI)`: default settings, OpenAPI 3.0.
    pub fn new(cache: Rc<dyn Cache>, open_api: &OpenAPI) -> PathsProcessor {
        PathsProcessor::with_settings(cache, open_api, Settings::default())
    }

    /// `new PathsProcessor(cache, openAPI, settings)`: OpenAPI 3.0.
    pub fn with_settings(cache: Rc<dyn Cache>, open_api: &OpenAPI, settings: Settings) -> PathsProcessor {
        PathsProcessor::new31(cache, open_api, settings, false)
    }

    /// `new PathsProcessor(cache, openAPI, settings, openapi31)`.
    pub fn new31(cache: Rc<dyn Cache>, open_api: &OpenAPI, settings: Settings, openapi31: bool) -> PathsProcessor {
        let _constructing = mocks::constructed("PathsProcessor");
        PathsProcessor {
            open_api: open_api.clone(),
            settings,
            parameter_processor: ParameterProcessor::new31(cache.clone(), open_api, openapi31),
            operation_processor: OperationProcessor::new31(cache.clone(), open_api, openapi31),
            external_ref_processor: ExternalRefProcessor::new(cache.clone(), open_api),
            cache,
        }
    }

    /// `processPaths()`
    pub fn process_paths(&self) -> JResult<()> {
        let Some(path_map) = self.open_api.get_paths() else { return Ok(()) };
        for path_str in path_map.keys() {
            let Some(mut path_item) = path_map.get(&path_str) else {
                return Err(JavaException::npe("Cannot invoke \"io.swagger.v3.oas.models.PathItem.get$ref()\" because \"pathItem\" is null"));
            };
            if let Some(r) = path_item.get_ref() {
                let resolved = self.process_reference_path(&path_item)?;
                let path_ref = java_split(&r, "#").into_iter().next().unwrap_or_default();
                if let Some(resolved) = resolved {
                    self.update_refs_path_item(&resolved, &path_ref);
                    self.open_api.path(&path_str, resolved.clone());
                    path_item = resolved;
                }
            }
            self.add_parameters_to_each_operation(&path_item)?;
            let processed = self.parameter_processor.process_parameters(path_item.get_parameters().as_ref())?;
            path_item.set_parameters(processed);
            for (_, op) in path_item.read_operations_map() {
                self.operation_processor.process_operation(&op)?;
            }
        }
        Ok(())
    }

    fn add_parameters_to_each_operation(&self, path_item: &PathItem) -> JResult<()> {
        if self.settings.add_parameters_to_each_operation {
            if let Some(parameters) = path_item.get_parameters() {
                for operation in path_item.read_operations() {
                    let mut to_add: Vec<Parameter> = Vec::new();
                    let existing = match operation.get_parameters() {
                        Some(e) => e,
                        None => {
                            let e = JList::new();
                            operation.set_parameters(e.clone());
                            e
                        }
                    };
                    for p in parameters.to_vec() {
                        let mut matched = false;
                        for e in existing.to_vec() {
                            if let Some(pin) = p.get_in() && Some(&pin) == e.get_in().as_ref() {
                                let name = p.get_name().ok_or_else(|| {
                                    JavaException::npe("Cannot invoke \"String.equals(Object)\" because the return value of \"io.swagger.v3.oas.models.parameters.Parameter.getName()\" is null")
                                })?;
                                if Some(name) == e.get_name() {
                                    matched = true;
                                }
                            }
                        }
                        if !matched {
                            to_add.push(p);
                        }
                    }
                    if !to_add.is_empty() {
                        let list = operation.get_parameters().unwrap();
                        for (i, p) in to_add.into_iter().enumerate() {
                            list.insert(i, p);
                        }
                    }
                }
            }
            path_item.set_parameters(None::<JList<Parameter>>);
        }
        Ok(())
    }

    /// `updateRefs(PathItem, pathRef)`
    pub fn update_refs_path_item(&self, path: &PathItem, path_ref: &str) {
        if let Some(params) = path.get_parameters() {
            for p in params.to_vec() {
                self.update_refs_parameter(&p, path_ref);
            }
        }
        for op in path.read_operations() {
            if let Some(params) = op.get_parameters() {
                for p in params.to_vec() {
                    self.update_refs_parameter(&p, path_ref);
                }
            }
            if let Some(responses) = op.get_responses() {
                for r in responses.values() {
                    self.update_refs_response(&r, path_ref);
                }
            }
            if let Some(body) = op.get_request_body() {
                self.update_refs_request_body(&body, path_ref);
            }
            if let Some(callbacks) = op.get_callbacks() {
                for name in callbacks.keys() {
                    if let Some(callback) = callbacks.get(&name) {
                        if let Some(r) = callback.get_ref() {
                            callback.set_ref(self.compute_ref(&r, path_ref));
                        }
                        for cn in callback.keys() {
                            if let Some(pi) = callback.get(&cn) {
                                self.update_refs_path_item(&pi, path_ref);
                            }
                        }
                    }
                }
            }
        }
    }

    /// `updateRefs(ApiResponse, pathRef)`
    pub fn update_refs_response(&self, response: &ApiResponse, path_ref: &str) {
        if let Some(r) = response.get_ref() {
            response.set_ref(self.compute_ref(&r, path_ref));
        }
        if let Some(content) = response.get_content() {
            for (_, mt) in content.entries() {
                if let Some(s) = mt.get_schema() {
                    self.update_refs_schema(&s, path_ref);
                }
                if let Some(ex) = mt.get_examples() {
                    for e in ex.values() {
                        self.update_refs_example(&e, path_ref);
                    }
                }
            }
        }
    }

    /// `updateRefs(Example, pathRef)`
    pub fn update_refs_example(&self, example: &Example, path_ref: &str) {
        if let Some(r) = example.get_ref() {
            example.set_ref(compute_preprocessed_ref(&r, path_ref));
        }
    }

    /// `updateRefs(Parameter, pathRef)`
    pub fn update_refs_parameter(&self, param: &Parameter, path_ref: &str) {
        if let Some(r) = param.get_ref() {
            param.set_ref(self.compute_ref(&r, path_ref));
        }
        if let Some(s) = param.get_schema() {
            self.update_refs_schema(&s, path_ref);
        }
        if let Some(content) = param.get_content() {
            for (_, mt) in content.entries() {
                if let Some(s) = mt.get_schema() {
                    self.update_refs_schema(&s, path_ref);
                }
            }
        }
    }

    /// `updateRefs(RequestBody, pathRef)`
    pub fn update_refs_request_body(&self, body: &RequestBody, path_ref: &str) {
        if let Some(r) = body.get_ref() {
            body.set_ref(self.compute_ref(&r, path_ref));
        }
        if let Some(content) = body.get_content() {
            for (_, mt) in content.entries() {
                if let Some(s) = mt.get_schema() {
                    self.update_refs_schema(&s, path_ref);
                }
                if let Some(ex) = mt.get_examples() {
                    for e in ex.values() {
                        self.update_refs_example(&e, path_ref);
                    }
                }
            }
        }
    }

    /// `updateRefs(Schema, pathRef)`
    pub fn update_refs_schema(&self, model: &Schema, path_ref: &str) {
        if let Some(r) = model.get_ref() {
            model.set_ref(compute_preprocessed_ref(&r, path_ref));
        } else if let Some(props) = model.get_properties() {
            for key in props.keys() {
                if let Some(p) = props.get(&key) {
                    self.update_refs_schema(&p, path_ref);
                }
            }
        } else if model.is_composed_schema() {
            if let Some(all_of) = model.get_all_of() {
                for inner in all_of.to_vec() {
                    self.update_refs_schema(&inner, path_ref);
                }
            }
            if model.get_any_of().is_some() || model.get_one_of().is_some() {
                let mut ref_mappings: Option<crate::java::JMap<String>> =
                    if model.get_discriminator().is_some_and(|d| d.get_mapping().is_some()) { Some(crate::java::JMap::new_hash()) } else { None };
                for l in [model.get_any_of(), model.get_one_of()].into_iter().flatten() {
                    for inner in l.to_vec() {
                        let old = inner.get_ref();
                        self.update_refs_schema(&inner, path_ref);
                        if let (Some(old), Some(rm)) = (old, ref_mappings.as_mut())
                            && Some(&old) != inner.get_ref().as_ref()
                        {
                            match inner.get_ref() {
                                Some(n) => {
                                    rm.put(old, n);
                                }
                                None => {
                                    rm.put_null_value(&old);
                                }
                            }
                        }
                    }
                }
                if let Some(rm) = ref_mappings.filter(|m| !m.is_empty()) {
                    let dm = model.get_discriminator().unwrap().get_mapping().unwrap();
                    for key in dm.keys() {
                        if let Some(v) = dm.get(&key) && let Some(n) = rm.get(&v) {
                            dm.put(key, n);
                        }
                    }
                }
            }
        } else if model.is_array_schema() && let Some(items) = model.get_items() {
            self.update_refs_schema(&items, path_ref);
        }
    }

    /// `computeRef(ref, prefix)`
    pub fn compute_ref(&self, r: &str, prefix: &str) -> String {
        if is_local_ref(r) && !is_internal_schema_ref(r) {
            return compute_local_ref(r, prefix);
        }
        if r.is_empty() {
            return r.to_string();
        }
        if reference_path_utils::is_absolute(Some(r)) {
            return r.to_string();
        }
        if is_internal_schema_ref(r) {
            return r.to_string();
        }
        reference_path_utils::resolve(Some(prefix), Some(r)).unwrap_or_default()
    }

    /// `processReferencePath(pathItem)`
    pub fn process_reference_path(&self, path_item: &PathItem) -> JResult<Option<PathItem>> {
        let r = path_item.get_ref().unwrap_or_default();
        let format = compute_ref_format(&r);
        if is_an_external_ref_format(format) {
            self.external_ref_processor.process_ref_to_external_path_item(&r, format)
        } else {
            load_ref::<PathItem>(self.cache.as_ref(), &r, format)
        }
    }
}

fn is_local_ref(r: &str) -> bool {
    r.starts_with('#')
}

fn is_internal_schema_ref(r: &str) -> bool {
    r.starts_with("#/components/schemas")
}

fn compute_preprocessed_ref(r: &str, prefix: &str) -> String {
    if is_local_ref(r) && !is_internal_schema_ref(r) {
        return compute_local_ref(r, prefix);
    }
    if !r.starts_with('.') || r.starts_with("./") || is_internal_schema_ref(r) {
        return r.to_string();
    }
    reference_path_utils::resolve(Some(prefix), Some(r)).unwrap_or_default()
}

fn compute_local_ref(r: &str, prefix: &str) -> String {
    format!("{prefix}{r}")
}
