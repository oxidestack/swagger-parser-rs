//! `ResponseProcessor`

use super::mocks;
use super::{ExampleProcessor, ExternalRefProcessor, HeaderProcessor, LinkProcessor, SchemaProcessor};
use crate::java::JResult;
use crate::models::*;
use crate::parser::ref_utils::{compute_ref_format, is_an_external_ref_format};
use crate::parser::resolver_cache::Cache;
use std::rc::Rc;

/// Port of `io.swagger.v3.parser.processors.ResponseProcessor`: resolves a response's `$ref` and the schemas, examples, headers and links inside it.
pub struct ResponseProcessor {
    schema_processor: SchemaProcessor,
    header_processor: HeaderProcessor,
    link_processor: LinkProcessor,
    external_ref_processor: ExternalRefProcessor,
    example_processor: ExampleProcessor,
}

impl ResponseProcessor {
    /// `new ResponseProcessor(cache, openAPI)`: the OpenAPI 3.0 processor.
    pub fn new(cache: Rc<dyn Cache>, open_api: &OpenAPI) -> ResponseProcessor {
        ResponseProcessor::new31(cache, open_api, false)
    }

    /// `new ResponseProcessor(cache, openAPI, openapi31)`.
    pub fn new31(cache: Rc<dyn Cache>, open_api: &OpenAPI, openapi31: bool) -> ResponseProcessor {
        let _constructing = mocks::constructed("ResponseProcessor");
        ResponseProcessor {
            schema_processor: SchemaProcessor::new31(cache.clone(), open_api, openapi31),
            header_processor: HeaderProcessor::new31(cache.clone(), open_api, openapi31),
            link_processor: LinkProcessor::new31(cache.clone(), open_api, openapi31),
            example_processor: ExampleProcessor::new(cache.clone(), open_api),
            external_ref_processor: ExternalRefProcessor::new(cache, open_api),
        }
    }

    /// `processResponse(response)`
    pub fn process_response(&self, response: &ApiResponse) -> JResult<()> {
        if let Some(h) = mocks::current().process_response && let Some(r) = h(response) {
            return r;
        }
        if response.get_ref().is_some() {
            self.process_reference_response(response)?;
        }
        if let Some(content) = response.get_content() {
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
        if let Some(headers) = response.get_headers() {
            for (_, h) in headers.entries() {
                self.header_processor.process_header(&h)?;
            }
        }
        if let Some(links) = response.get_links() {
            for (_, l) in links.entries() {
                self.link_processor.process_link(&l)?;
            }
        }
        Ok(())
    }

    /// `processReferenceResponse(response)`
    pub fn process_reference_response(&self, response: &ApiResponse) -> JResult<()> {
        let r = response.get_ref().unwrap_or_default();
        let format = compute_ref_format(&r);
        if is_an_external_ref_format(format)
            && let Some(n) = self.external_ref_processor.process_ref_to_external_response(&r, format)?
        {
            response.set_ref(n);
        }
        Ok(())
    }
}
