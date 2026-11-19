//! `RequestBodyProcessor`

use super::mocks;
use super::{ExampleProcessor, ExternalRefProcessor, SchemaProcessor};
use crate::java::JResult;
use crate::models::*;
use crate::parser::ref_utils::{compute_ref_format, is_an_external_ref_format};
use crate::parser::resolver_cache::Cache;
use std::rc::Rc;

/// Port of `io.swagger.v3.parser.processors.RequestBodyProcessor`: resolves a request body's `$ref` and the schemas and examples in its content.
pub struct RequestBodyProcessor {
    schema_processor: SchemaProcessor,
    external_ref_processor: ExternalRefProcessor,
    example_processor: ExampleProcessor,
}

impl RequestBodyProcessor {
    /// `new RequestBodyProcessor(cache, openAPI)`: the OpenAPI 3.0 processor.
    pub fn new(cache: Rc<dyn Cache>, open_api: &OpenAPI) -> RequestBodyProcessor {
        RequestBodyProcessor::new31(cache, open_api, false)
    }

    /// `new RequestBodyProcessor(cache, openAPI, openapi31)`.
    pub fn new31(cache: Rc<dyn Cache>, open_api: &OpenAPI, openapi31: bool) -> RequestBodyProcessor {
        let _constructing = mocks::constructed("RequestBodyProcessor");
        RequestBodyProcessor {
            schema_processor: SchemaProcessor::new31(cache.clone(), open_api, openapi31),
            example_processor: ExampleProcessor::new(cache.clone(), open_api),
            external_ref_processor: ExternalRefProcessor::new(cache, open_api),
        }
    }

    /// `processRequestBody(requestBody)`
    pub fn process_request_body(&self, body: &RequestBody) -> JResult<()> {
        if let Some(h) = mocks::current().process_request_body && let Some(r) = h(body) {
            return r;
        }
        if body.get_ref().is_some() {
            self.process_reference_request_body(body)?;
        }
        if let Some(content) = body.get_content() {
            for (_, mt) in content.entries() {
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
        Ok(())
    }

    /// `processReferenceRequestBody(requestBody)`
    pub fn process_reference_request_body(&self, body: &RequestBody) -> JResult<()> {
        let r = body.get_ref().unwrap_or_default();
        let format = compute_ref_format(&r);
        if is_an_external_ref_format(format)
            && let Some(n) = self.external_ref_processor.process_ref_to_external_request_body(&r, format)?
        {
            body.set_ref(n);
        }
        Ok(())
    }
}
