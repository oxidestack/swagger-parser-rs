//! `ParameterProcessor`

use super::mocks;
use super::{ExampleProcessor, ExternalRefProcessor, SchemaProcessor};
use crate::java::{JList, JResult, JavaException};
use crate::models::*;
use crate::parser::ref_utils::{compute_ref_format, is_an_external_ref_format};
use crate::parser::resolver_cache::{load_ref, Cache};
use std::rc::Rc;

/// Port of `io.swagger.v3.parser.processors.ParameterProcessor`: resolves a parameter's `$ref` and the schemas, examples and content inside it.
pub struct ParameterProcessor {
    cache: Rc<dyn Cache>,
    schema_processor: SchemaProcessor,
    example_processor: ExampleProcessor,
    external_ref_processor: ExternalRefProcessor,
}

fn name_npe() -> JavaException {
    JavaException::npe("Cannot invoke \"String.equals(Object)\" because the return value of \"io.swagger.v3.oas.models.parameters.Parameter.getName()\" is null")
}

impl ParameterProcessor {
    /// `new ParameterProcessor(cache, openAPI)`: the OpenAPI 3.0 processor.
    pub fn new(cache: Rc<dyn Cache>, open_api: &OpenAPI) -> ParameterProcessor {
        ParameterProcessor::new31(cache, open_api, false)
    }

    /// `new ParameterProcessor(cache, openAPI, openapi31)`.
    pub fn new31(cache: Rc<dyn Cache>, open_api: &OpenAPI, openapi31: bool) -> ParameterProcessor {
        let _constructing = mocks::constructed("ParameterProcessor");
        ParameterProcessor {
            schema_processor: SchemaProcessor::new31(cache.clone(), open_api, openapi31),
            example_processor: ExampleProcessor::new(cache.clone(), open_api),
            external_ref_processor: ExternalRefProcessor::new(cache.clone(), open_api),
            cache,
        }
    }

    /// `processParameter(parameter)`
    pub fn process_parameter(&self, parameter: &Parameter) -> JResult<()> {
        if let Some(h) = mocks::current().process_parameter && let Some(r) = h(parameter) {
            return r;
        }
        if let Some(reference_path) = parameter.get_ref() {
            let format = compute_ref_format(&reference_path);
            if is_an_external_ref_format(format)
                && let Some(n) = self.external_ref_processor.process_ref_to_external_parameter(&reference_path, format)?
            {
                parameter.set_ref(format!("#/components/parameters/{n}"));
            }
        }
        if let Some(s) = parameter.get_schema() {
            self.schema_processor.process_schema(Some(&s))?;
        }
        if let Some(examples) = parameter.get_examples() {
            for e in examples.values() {
                self.example_processor.process_example(&e)?;
            }
        }
        if let Some(content) = parameter.get_content() {
            for (_, mt) in content.entries() {
                if let Some(s) = mt.get_schema() {
                    self.schema_processor.process_schema(Some(&s))?;
                }
            }
        }
        Ok(())
    }

    /// `processParameters(parameters)`
    pub fn process_parameters(&self, parameters: Option<&JList<Parameter>>) -> JResult<Option<JList<Parameter>>> {
        if let Some(h) = mocks::current().process_parameters && let Some(r) = h(parameters) {
            return r;
        }
        let Some(parameters) = parameters else { return Ok(None) };
        let mut ref_parameters: Vec<Parameter> = Vec::new();
        let processed = self.process_path_level_params(parameters, &mut ref_parameters)?;
        for resolved in &ref_parameters {
            for (pos, param) in processed.to_vec().into_iter().enumerate() {
                let name = param.get_name().ok_or_else(name_npe)?;
                if Some(name) == resolved.get_name() {
                    processed.set(pos, resolved.clone());
                    break;
                }
            }
        }
        for parameter in processed.to_vec() {
            if let Some(s) = parameter.get_schema() {
                self.schema_processor.process_schema(Some(&s))?;
            } else if let Some(content) = parameter.get_content() {
                for mt in content.values() {
                    if let Some(s) = mt.get_schema() {
                        self.schema_processor.process_schema(Some(&s))?;
                    }
                    if let Some(ex) = mt.get_examples() {
                        for e in ex.values() {
                            self.example_processor.process_example(&e)?;
                        }
                    }
                }
            }
        }
        Ok(Some(processed))
    }

    fn process_path_level_params(&self, parameters: &JList<Parameter>, ref_parameters: &mut Vec<Parameter>) -> JResult<JList<Parameter>> {
        let processed: JList<Parameter> = JList::new();
        for parameter in parameters.to_vec() {
            if let Some(r) = parameter.get_ref() {
                let format = compute_ref_format(&r);
                let resolved = load_ref::<Parameter>(self.cache.as_ref(), &r, format)?;
                if r.starts_with('#') && !r.contains("#/components/parameters") {
                    continue;
                }
                let Some(resolved) = resolved else {
                    processed.add(parameter.clone());
                    continue;
                };
                if is_parameter_exist(parameters, &processed, &resolved) {
                    ref_parameters.push(resolved);
                } else {
                    processed.add(resolved);
                }
            } else {
                processed.add(parameter.clone());
            }
        }
        Ok(processed)
    }
}

fn same_name_and_in(param: &Parameter, resolved: &Parameter) -> bool {
    match (param.get_name(), param.get_in()) {
        (Some(n), Some(i)) => Some(n) == resolved.get_name() && Some(i) == resolved.get_in(),
        _ => false,
    }
}

fn is_parameter_exist(parameters: &JList<Parameter>, processed: &JList<Parameter>, resolved: &Parameter) -> bool {
    processed.to_vec().iter().any(|p| same_name_and_in(p, resolved)) || parameters.to_vec().iter().any(|p| same_name_and_in(p, resolved))
}
