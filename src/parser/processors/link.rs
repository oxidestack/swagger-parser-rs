//! `LinkProcessor`

use super::mocks;
use super::{ExternalRefProcessor, HeaderProcessor};
use crate::java::JResult;
use crate::models::*;
use crate::parser::ref_utils::{compute_ref_format, is_an_external_ref_format};
use crate::parser::resolver_cache::Cache;
use std::rc::Rc;

/// Port of `io.swagger.v3.parser.processors.LinkProcessor`: resolves a link's `$ref` and the headers inside it.
pub struct LinkProcessor {
    header_processor: HeaderProcessor,
    external_ref_processor: ExternalRefProcessor,
}

impl LinkProcessor {
    /// `new LinkProcessor(cache, openAPI)`: the OpenAPI 3.0 processor.
    pub fn new(cache: Rc<dyn Cache>, open_api: &OpenAPI) -> LinkProcessor {
        LinkProcessor::new31(cache, open_api, false)
    }

    /// `new LinkProcessor(cache, openAPI, openapi31)`.
    pub fn new31(cache: Rc<dyn Cache>, open_api: &OpenAPI, openapi31: bool) -> LinkProcessor {
        let _constructing = mocks::constructed("LinkProcessor");
        LinkProcessor { header_processor: HeaderProcessor::new31(cache.clone(), open_api, openapi31), external_ref_processor: ExternalRefProcessor::new(cache, open_api) }
    }

    /// `processLink(link)`
    pub fn process_link(&self, link: &Link) -> JResult<()> {
        if let Some(h) = mocks::current().process_link && let Some(r) = h(link) {
            return r;
        }
        if let Some(r) = link.get_ref() {
            let format = compute_ref_format(&r);
            if is_an_external_ref_format(format)
                && let Some(n) = self.external_ref_processor.process_ref_to_external_link(&r, format)?
            {
                link.set_ref(n);
            }
        } else if let Some(headers) = link.get_headers() {
            for (_, h) in headers.entries() {
                self.header_processor.process_header(&h)?;
            }
        }
        Ok(())
    }
}
