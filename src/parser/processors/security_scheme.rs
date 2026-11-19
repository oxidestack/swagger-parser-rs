//! `SecuritySchemeProcessor`

use super::mocks;
use crate::java::JResult;
use crate::models::*;
use crate::parser::ref_utils::compute_ref_format;
use crate::parser::resolver_cache::{load_ref, Cache};
use std::rc::Rc;

/// Port of `io.swagger.v3.parser.processors.SecuritySchemeProcessor`: replaces a security scheme `$ref` with the scheme it points to.
pub struct SecuritySchemeProcessor {
    cache: Rc<dyn Cache>,
}

impl SecuritySchemeProcessor {
    /// `new SecuritySchemeProcessor(cache, openAPI)`; the document is not used.
    pub fn new(cache: Rc<dyn Cache>, _open_api: &OpenAPI) -> SecuritySchemeProcessor {
        let _constructing = mocks::constructed("SecuritySchemeProcessor");
        SecuritySchemeProcessor { cache }
    }

    /// `processSecurityScheme(securityScheme)`
    pub fn process_security_scheme(&self, ss: &SecurityScheme) -> JResult<SecurityScheme> {
        if let Some(h) = mocks::current().process_security_scheme && let Some(r) = h(ss) {
            return r;
        }
        if let Some(r) = ss.get_ref() {
            let format = compute_ref_format(&r);
            if let Some(n) = load_ref::<SecurityScheme>(self.cache.as_ref(), &r, format)? {
                return Ok(n);
            }
        }
        Ok(ss.clone())
    }
}
