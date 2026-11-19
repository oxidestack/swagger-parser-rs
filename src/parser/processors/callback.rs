//! `CallbackProcessor`

use super::mocks;
use super::{ExternalRefProcessor, OperationProcessor, ParameterProcessor};
use crate::java::{JResult, JavaException};
use crate::models::*;
use crate::parser::ref_utils::{compute_ref_format, is_an_external_ref_format};
use crate::parser::resolver_cache::Cache;
use std::rc::Rc;

/// Port of `io.swagger.v3.parser.processors.CallbackProcessor`: resolves a callback's `$ref` and the references inside its path items.
pub struct CallbackProcessor {
    operation_processor: OperationProcessor,
    parameter_processor: ParameterProcessor,
    external_ref_processor: ExternalRefProcessor,
}

impl CallbackProcessor {
    /// `new CallbackProcessor(cache, openAPI)`: the OpenAPI 3.0 processor.
    pub fn new(cache: Rc<dyn Cache>, open_api: &OpenAPI) -> CallbackProcessor {
        CallbackProcessor::new31(cache, open_api, false)
    }

    /// `new CallbackProcessor(cache, openAPI, openapi31)`.
    pub fn new31(cache: Rc<dyn Cache>, open_api: &OpenAPI, openapi31: bool) -> CallbackProcessor {
        let _constructing = mocks::constructed("CallbackProcessor");
        CallbackProcessor {
            operation_processor: OperationProcessor::new31(cache.clone(), open_api, openapi31),
            parameter_processor: ParameterProcessor::new31(cache.clone(), open_api, openapi31),
            external_ref_processor: ExternalRefProcessor::new(cache, open_api),
        }
    }

    /// `processCallback(callback)`
    pub fn process_callback(&self, callback: &Callback) -> JResult<()> {
        if let Some(h) = mocks::current().process_callback && let Some(r) = h(callback) {
            return r;
        }
        if callback.get_ref().is_some() {
            self.process_reference_callback(callback)?;
        }
        for name in callback.keys() {
            let Some(path_item) = callback.get(&name) else {
                return Err(JavaException::npe("Cannot invoke \"io.swagger.v3.oas.models.PathItem.readOperationsMap()\" because \"pathItem\" is null"));
            };
            for (_, op) in path_item.read_operations_map() {
                self.operation_processor.process_operation(&op)?;
            }
            if let Some(params) = path_item.get_parameters() {
                for p in params.to_vec() {
                    self.parameter_processor.process_parameter(&p)?;
                }
            }
        }
        Ok(())
    }

    /// `processReferenceCallback(callback)`
    pub fn process_reference_callback(&self, callback: &Callback) -> JResult<()> {
        let r = callback.get_ref().unwrap_or_default();
        let format = compute_ref_format(&r);
        if is_an_external_ref_format(format)
            && let Some(n) = self.external_ref_processor.process_ref_to_external_callback(&r, format)?
        {
            callback.set_ref(format!("#/components/callbacks/{n}"));
        }
        Ok(())
    }
}
