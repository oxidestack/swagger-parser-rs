//! `HeaderProcessor`

use super::mocks;
use super::{ExampleProcessor, ExternalRefProcessor, SchemaProcessor};
use crate::java::JResult;
use crate::models::*;
use crate::parser::ref_utils::{compute_ref_format, is_an_external_ref_format};
use crate::parser::resolver_cache::Cache;
use std::rc::Rc;

/// Port of `io.swagger.v3.parser.processors.HeaderProcessor`: resolves a header's `$ref` and the schemas and examples inside it.
pub struct HeaderProcessor {
    schema_processor: SchemaProcessor,
    example_processor: ExampleProcessor,
    external_ref_processor: ExternalRefProcessor,
}

impl HeaderProcessor {
    /// `new HeaderProcessor(cache, openAPI)`: the OpenAPI 3.0 processor.
    pub fn new(cache: Rc<dyn Cache>, open_api: &OpenAPI) -> HeaderProcessor {
        HeaderProcessor::new31(cache, open_api, false)
    }

    /// `new HeaderProcessor(cache, openAPI, openapi31)`.
    pub fn new31(cache: Rc<dyn Cache>, open_api: &OpenAPI, openapi31: bool) -> HeaderProcessor {
        let _constructing = mocks::constructed("HeaderProcessor");
        HeaderProcessor {
            schema_processor: SchemaProcessor::new31(cache.clone(), open_api, openapi31),
            example_processor: ExampleProcessor::new(cache.clone(), open_api),
            external_ref_processor: ExternalRefProcessor::new(cache, open_api),
        }
    }

    /// `processHeader(header)`
    pub fn process_header(&self, header: &Header) -> JResult<()> {
        if let Some(h) = mocks::current().process_header && let Some(r) = h(header) {
            return r;
        }
        if let Some(r) = header.get_ref() {
            let format = compute_ref_format(&r);
            if is_an_external_ref_format(format)
                && let Some(n) = self.external_ref_processor.process_ref_to_external_header(&r, format)?
            {
                header.set_ref(n);
            }
        }
        if let Some(s) = header.get_schema() {
            self.schema_processor.process_schema(Some(&s))?;
        }
        if let Some(examples) = header.get_examples() {
            for key in examples.keys() {
                match header.get_examples().and_then(|e| e.get(&key)) {
                    Some(e) => self.example_processor.process_example(&e)?,
                    None => return Err(crate::java::JavaException::npe("Cannot invoke \"io.swagger.v3.oas.models.examples.Example.get$ref()\" because \"example\" is null")),
                }
            }
        }
        if let Some(content) = header.get_content() {
            for (_, mt) in content.entries() {
                if let Some(s) = mt.get_schema() {
                    self.schema_processor.process_schema(Some(&s))?;
                }
            }
        }
        Ok(())
    }
}
