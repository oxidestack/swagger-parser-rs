//! `ExampleProcessor`

use super::mocks;
use super::ExternalRefProcessor;
use crate::java::JResult;
use crate::models::*;
use crate::parser::ref_utils::{compute_ref_format, is_an_external_ref_format};
use crate::parser::resolver_cache::Cache;
use std::rc::Rc;

/// Port of `io.swagger.v3.parser.processors.ExampleProcessor`: resolves an example's external `$ref`.
pub struct ExampleProcessor {
    external_ref_processor: ExternalRefProcessor,
}

impl ExampleProcessor {
    /// `new ExampleProcessor(cache, openAPI)`.
    pub fn new(cache: Rc<dyn Cache>, open_api: &OpenAPI) -> ExampleProcessor {
        let _constructing = mocks::constructed("ExampleProcessor");
        ExampleProcessor { external_ref_processor: ExternalRefProcessor::new(cache, open_api) }
    }

    /// `processExample(example)`
    pub fn process_example(&self, example: &Example) -> JResult<()> {
        if let Some(h) = mocks::current().process_example && let Some(r) = h(example) {
            return r;
        }
        if example.get_ref().is_some() {
            self.process_reference_example(example)?;
        }
        Ok(())
    }

    /// `processExample(List<Example>)`
    pub fn process_examples(&self, examples: &[Example]) -> JResult<()> {
        for e in examples {
            if e.get_ref().is_some() {
                self.process_reference_example(e)?;
            }
        }
        Ok(())
    }

    fn process_reference_example(&self, example: &Example) -> JResult<()> {
        let r = example.get_ref().unwrap_or_default();
        let format = compute_ref_format(&r);
        if is_an_external_ref_format(format)
            && let Some(n) = self.external_ref_processor.process_ref_to_external_example(&r, format)?
        {
            example.set_ref(n);
        }
        Ok(())
    }
}
