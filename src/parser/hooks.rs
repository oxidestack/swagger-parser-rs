//! Replaceable static entry points.
//!
//! The upstream tests replace static methods (`RemoteUrl.urlToString`, `RefUtils.readExternal*Ref`,
//! `DeserializationUtils.deserializeIntoTree`, `ClasspathHelper.loadFileFromClasspath`) with JMockit
//! expectations. Here each of those functions first consults a thread-local hook: a hook returning `Some`
//! replaces the real call, `None` lets it through. Hooks are per thread, so parallel tests do not interfere.
//! `net_utils` replaces the `NetUtils` (DNS) the parser's `PermittedUrlsChecker`s use, so safe-resolution
//! tests need no network.

use crate::core::{AuthorizationValue, ParseOptions, SwaggerParseResult};
use crate::java::JResult;
use crate::parser::ref_utils::RefFormat;
use jackson::JsonNode;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use swagger_parser_safe_url_resolver::{NetUtils, PermittedUrlsChecker, SystemNetUtils};

/// `RemoteUrl.urlToString(url, auths[, checker])`
pub type UrlToStringHook = Rc<dyn Fn(&str, &[AuthorizationValue]) -> Option<JResult<String>>>;
/// `RefUtils.readExternalRef` / `readExternalUrlRef` / `readExternalClasspathRef`: (file, format, auths, root or
/// parent directory, checker)
pub type ReadExternalRefHook = Rc<dyn Fn(&str, RefFormat, &[AuthorizationValue], Option<&str>, Option<&PermittedUrlsChecker>) -> Option<JResult<Option<String>>>>;
/// `DeserializationUtils.deserializeIntoTree(contents, uri, options, result)`
pub type DeserializeIntoTreeHook = Rc<dyn Fn(&str, Option<&str>, Option<&ParseOptions>, &mut SwaggerParseResult) -> Option<JResult<JsonNode>>>;
/// `ClasspathHelper.loadFileFromClasspath(location)`
pub type ClasspathHook = Rc<dyn Fn(&str) -> Option<JResult<String>>>;
/// `ProxySelector.getDefault().select(uri)`: (the URL, its host), asked before every connection a remote read opens,
/// each redirect it follows included; an `Err` is thrown out of the read, as an exception `select` throws is.
pub type SelectProxyHook = Rc<dyn Fn(&str, &str) -> JResult<()>>;

/// The hooks of one thread; every field left `None` keeps the real behaviour.
#[derive(Default, Clone)]
pub struct Hooks {
    /// Replaces `RemoteUrl.urlToString`.
    pub url_to_string: Option<UrlToStringHook>,
    /// Replaces `RefUtils.readExternalRef`.
    pub read_external_ref: Option<ReadExternalRefHook>,
    /// Replaces `RefUtils.readExternalUrlRef`.
    pub read_external_url_ref: Option<ReadExternalRefHook>,
    /// Replaces `RefUtils.readExternalClasspathRef`.
    pub read_external_classpath_ref: Option<ReadExternalRefHook>,
    /// Replaces `DeserializationUtils.deserializeIntoTree`.
    pub deserialize_into_tree: Option<DeserializeIntoTreeHook>,
    /// Replaces `ClasspathHelper.loadFileFromClasspath`.
    pub load_file_from_classpath: Option<ClasspathHook>,
    /// The `NetUtils` behind every `PermittedUrlsChecker` the parser creates (host name resolution);
    /// lets tests resolve host names without DNS.
    pub net_utils: Option<Arc<dyn NetUtils>>,
    /// The default `ProxySelector`, which an embedding application may replace to check each connection.
    pub select_proxy: Option<SelectProxyHook>,
}

thread_local! {
    static HOOKS: RefCell<Hooks> = RefCell::new(Hooks::default());
}

/// Installs hooks for the current thread; the previous hooks come back when the guard drops.
pub fn install(h: Hooks) -> HooksGuard {
    let prev = HOOKS.with(|c| std::mem::replace(&mut *c.borrow_mut(), h));
    HooksGuard { prev: Some(prev) }
}

/// Restores the previously installed hooks when dropped (see [`install`]).
pub struct HooksGuard {
    prev: Option<Hooks>,
}

impl Drop for HooksGuard {
    fn drop(&mut self) {
        if let Some(p) = self.prev.take() {
            HOOKS.with(|c| *c.borrow_mut() = p);
        }
    }
}

/// The hooks installed for the current thread, to install again with one changed.
pub fn installed() -> Hooks {
    current()
}

pub(crate) fn current() -> Hooks {
    HOOKS.with(|c| c.borrow().clone())
}

/// `new PermittedUrlsChecker(allowlist, denylist)`, using the [`Hooks::net_utils`] replacement if one is installed.
pub(crate) fn permitted_urls_checker(allowlist: Option<&[String]>, denylist: Option<&[String]>) -> JResult<PermittedUrlsChecker> {
    let net: Arc<dyn NetUtils> = current().net_utils.unwrap_or_else(|| Arc::new(SystemNetUtils));
    PermittedUrlsChecker::with_net_utils(allowlist, denylist, net)
}
