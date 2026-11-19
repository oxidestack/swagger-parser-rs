//! `OperationProcessor`

use super::mocks;
use super::{ExternalRefProcessor, ParameterProcessor, RequestBodyProcessor, ResponseProcessor};
use crate::java::{JResult, JavaException};
use crate::models::*;
use crate::parser::ref_utils::{compute_ref_format, is_an_external_ref_format};
use crate::parser::resolver_cache::{load_ref, Cache};
use std::rc::Rc;

/// Port of `io.swagger.v3.parser.processors.OperationProcessor`: resolves the references in an operation's parameters, request body, responses and callbacks.
pub struct OperationProcessor {
    parameter_processor: ParameterProcessor,
    request_body_processor: RequestBodyProcessor,
    response_processor: ResponseProcessor,
    external_ref_processor: ExternalRefProcessor,
    cache: Rc<dyn Cache>,
}

impl OperationProcessor {
    /// `new OperationProcessor(cache, openAPI)`: the OpenAPI 3.0 processor.
    pub fn new(cache: Rc<dyn Cache>, open_api: &OpenAPI) -> OperationProcessor {
        OperationProcessor::new31(cache, open_api, false)
    }

    /// `new OperationProcessor(cache, openAPI, openapi31)`.
    pub fn new31(cache: Rc<dyn Cache>, open_api: &OpenAPI, openapi31: bool) -> OperationProcessor {
        let _constructing = mocks::constructed("OperationProcessor");
        OperationProcessor {
            parameter_processor: ParameterProcessor::new31(cache.clone(), open_api, openapi31),
            response_processor: ResponseProcessor::new31(cache.clone(), open_api, openapi31),
            request_body_processor: RequestBodyProcessor::new31(cache.clone(), open_api, openapi31),
            external_ref_processor: ExternalRefProcessor::new(cache.clone(), open_api),
            cache,
        }
    }

    /// `processOperation(operation)`
    pub fn process_operation(&self, operation: &Operation) -> JResult<()> {
        if let Some(params) = operation.get_parameters() {
            for p in params.to_vec() {
                self.parameter_processor.process_parameter(&p)?;
            }
        }
        let processed = self.parameter_processor.process_parameters(operation.get_parameters().as_ref())?;
        if let Some(p) = processed {
            operation.set_parameters(p);
        }

        if let Some(mut request_body) = operation.get_request_body() {
            let options = self.cache.get_parse_options();
            if let Some(r) = request_body.get_ref() && options.as_ref().is_some_and(|o| o.resolve_request_body) {
                self.request_body_processor.process_request_body(&request_body)?;
                let r = request_body.get_ref().unwrap_or(r);
                let format = compute_ref_format(&r);
                if let Some(resolved) = load_ref::<RequestBody>(self.cache.as_ref(), &r, format)? {
                    request_body = resolved.clone();
                    operation.set_request_body(resolved);
                }
            }
            self.request_body_processor.process_request_body(&request_body)?;
        }

        if let Some(responses) = operation.get_responses() {
            for code in responses.keys() {
                let Some(mut response) = responses.get(&code) else { continue };
                if response.get_ref().is_some() && self.cache.get_parse_options().is_some_and(|o| o.resolve_responses) {
                    self.response_processor.process_response(&response)?;
                    let r = response.get_ref().unwrap_or_default();
                    let format = compute_ref_format(&r);
                    if let Some(resolved) = load_ref::<ApiResponse>(self.cache.as_ref(), &r, format)? {
                        response = resolved.clone();
                        responses.put(code.clone(), resolved);
                    }
                }
                self.response_processor.process_response(&response)?;
            }
        }

        if let Some(callbacks) = operation.get_callbacks() {
            for name in callbacks.keys() {
                let Some(callback) = callbacks.get(&name) else { continue };
                if let Some(r) = callback.get_ref() {
                    let format = compute_ref_format(&r);
                    if is_an_external_ref_format(format)
                        && let Some(n) = self.external_ref_processor.process_ref_to_external_callback(&r, format)?
                    {
                        callback.set_ref(n);
                    }
                }
                for callback_name in callback.keys() {
                    let Some(path_item) = callback.get(&callback_name) else {
                        return Err(JavaException::npe("Cannot invoke \"io.swagger.v3.oas.models.PathItem.readOperationsMap()\" because \"pathItem\" is null"));
                    };
                    for (_, op) in path_item.read_operations_map() {
                        self.process_operation(&op)?;
                    }
                    if let Some(params) = path_item.get_parameters() {
                        for p in params.to_vec() {
                            self.parameter_processor.process_parameter(&p)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
