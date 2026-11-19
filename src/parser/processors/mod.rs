//! `io.swagger.v3.parser.processors`: the OpenAPI 3.0 reference resolution passes.
//!
//! The upstream tests mock processor classes with JMockit (`@Mocked SchemaProcessor` intercepts every
//! instance). The equivalent seam here is [`mocks`]: thread-local handlers consulted by each processor method
//! (and a constructor observer); with no handler installed the real code runs.

pub mod callback;
pub mod components;
pub mod example;
pub mod external_ref;
pub mod header;
pub mod link;
pub mod operation;
pub mod parameter;
pub mod paths;
pub mod request_body;
pub mod response;
pub mod schema;
pub mod security_scheme;

pub use callback::CallbackProcessor;
pub use components::ComponentsProcessor;
pub use example::ExampleProcessor;
pub use external_ref::ExternalRefProcessor;
pub use header::HeaderProcessor;
pub use link::LinkProcessor;
pub use operation::OperationProcessor;
pub use parameter::ParameterProcessor;
pub use paths::PathsProcessor;
pub use request_body::RequestBodyProcessor;
pub use response::ResponseProcessor;
pub use schema::SchemaProcessor;
pub use security_scheme::SecuritySchemeProcessor;

use crate::java::JMap;
use crate::parser::resolver_cache::Cache;

/// `ReferencePathUtils`
pub mod reference_path_utils {
    use crate::java::net::JavaUri;

    /// `isAbsolute(ref)`
    pub fn is_absolute(r: Option<&str>) -> bool {
        let Some(r) = r else { return false };
        if is_absolute_file_path(r) {
            return true;
        }
        match JavaUri::parse(r) {
            Ok(u) => u.is_absolute(),
            Err(_) => has_scheme(r),
        }
    }

    /// `resolve(source, ref)`
    pub fn resolve(source: Option<&str>, r: Option<&str>) -> Option<String> {
        let (Some(source), Some(r)) = (source, r) else { return source.map(str::to_string) };
        let attempt = || -> Option<String> {
            let mut source_uri = JavaUri::parse(source).ok()?;
            if (!source.ends_with('/') && r.starts_with("./") && source_uri.get_raw_path() == Some(""))
                || (source_uri.get_raw_path() == Some("") && !r.starts_with('/'))
            {
                source_uri = JavaUri::parse(&format!("{source}/")).ok()?;
            }
            // URI.resolve on an opaque base returns the child; `getPath()` of an opaque URI is null
            let child = JavaUri::parse(r).ok()?;
            let resolved = source_uri.resolve(&child).normalize();
            let resolved_ref = resolved.to_java_string();
            if source.starts_with("./") && !resolved.is_absolute() && !resolved_ref.starts_with('.') && !resolved_ref.starts_with('/') {
                return Some(format!("./{resolved_ref}"));
            }
            Some(resolved_ref)
        };
        Some(attempt().unwrap_or_else(|| resolve_raw_path(source, r)))
    }

    fn resolve_raw_path(source: &str, r: &str) -> String {
        if r.is_empty() {
            return strip_fragment(source).to_string();
        }
        if is_absolute(Some(r)) {
            return r.to_string();
        }
        if r.starts_with('#') {
            return format!("{}{r}", strip_fragment(source));
        }
        if r.starts_with('?') {
            return format!("{}{r}", strip_query_and_fragment(source));
        }
        let source_file = strip_query_and_fragment(source);
        let last = [source_file.rfind('/'), source_file.rfind('\\')].into_iter().flatten().max();
        let Some(last) = last else { return normalize_relative_path(r) };
        let resolved = format!("{}{r}", &source_file[..=last]);
        if has_scheme(source_file) {
            return resolved;
        }
        normalize_relative_path(&resolved)
    }

    fn normalize_relative_path(r: &str) -> String {
        let ss = suffix_start(r);
        let (path, suffix) = match ss {
            Some(i) => (&r[..i], &r[i..]),
            None => (r, ""),
        };
        let leading_dot_slash = path.starts_with("./");
        let leading_slash = path.starts_with('/');
        let mut segments: Vec<&str> = Vec::new();
        for seg in path.split('/') {
            if seg.is_empty() || seg == "." {
                continue;
            }
            if seg == ".." {
                if !segments.is_empty() && segments.last() != Some(&"..") {
                    segments.pop();
                } else if !leading_slash {
                    segments.push(seg);
                }
            } else {
                segments.push(seg);
            }
        }
        let mut normalized = segments.join("/");
        if leading_slash {
            normalized = format!("/{normalized}");
        } else if leading_dot_slash && !normalized.starts_with("..") && !normalized.is_empty() {
            normalized = format!("./{normalized}");
        }
        normalized + suffix
    }

    fn is_absolute_file_path(r: &str) -> bool {
        let c: Vec<char> = r.chars().collect();
        r.starts_with('/') || r.starts_with('\\') || (c.len() >= 3 && c[0].is_alphabetic() && c[1] == ':' && (c[2] == '/' || c[2] == '\\'))
    }

    fn has_scheme(r: &str) -> bool {
        let Some(colon) = r.find(':') else { return false };
        let c: Vec<char> = r[..colon].chars().collect();
        if colon == 0 || !c[0].is_alphabetic() {
            return false;
        }
        c[1..].iter().all(|&ch| ch.is_alphanumeric() || ch == '+' || ch == '-' || ch == '.')
    }

    fn strip_fragment(v: &str) -> &str {
        match v.find('#') {
            Some(i) => &v[..i],
            None => v,
        }
    }

    fn strip_query_and_fragment(v: &str) -> &str {
        match suffix_start(v) {
            Some(i) => &v[..i],
            None => v,
        }
    }

    fn suffix_start(v: &str) -> Option<usize> {
        match (v.find('?'), v.find('#')) {
            (None, f) => f,
            (q, None) => q,
            (Some(q), Some(f)) => Some(q.min(f)),
        }
    }
}

/// `String.equalsIgnoreCase` (per UTF-16 unit: equal, or equal after `toUpperCase`, or after `toLowerCase`)
pub(crate) fn java_equals_ignore_case(a: &str, b: &str) -> bool {
    let (x, y): (Vec<u16>, Vec<u16>) = (a.encode_utf16().collect(), b.encode_utf16().collect());
    if x.len() != y.len() {
        return false;
    }
    x.iter().zip(y.iter()).all(|(&c1, &c2)| {
        if c1 == c2 {
            return true;
        }
        let (Some(a), Some(b)) = (char::from_u32(c1 as u32), char::from_u32(c2 as u32)) else { return false };
        let up = |c: char| {
            let mut u = c.to_uppercase();
            match (u.next(), u.next()) {
                (Some(x), None) => x,
                _ => c,
            }
        };
        let low = |c: char| {
            let mut u = c.to_lowercase();
            match (u.next(), u.next()) {
                (Some(x), None) => x,
                _ => c,
            }
        };
        let (ua, ub) = (up(a), up(b));
        ua == ub || low(ua) == low(ub)
    })
}

/// `ComponentNameAllocator`
pub(crate) struct ComponentNameAllocator<'a> {
    pub cache: &'a dyn Cache,
}

impl<'a> ComponentNameAllocator<'a> {
    /// `allocate(components, baseName, incoming, incomingRef, refOf)` (exact key lookup, no reuse policy)
    pub fn allocate<T: Clone + PartialEq>(&self, components: &JMap<T>, base_name: &str, incoming: &T, incoming_ref: &str, ref_of: impl Fn(&T) -> Option<String>) -> String {
        self.allocate_with(components, base_name, incoming, incoming_ref, ref_of, |name| if components.contains_key(name) { Some(name.to_string()) } else { None }, |_, _| false)
    }

    // Upstream's seven-parameter `allocate(components, baseName, incoming, incomingRef, refOf, keyOf, reusePolicy)`,
    // parameter for parameter.
    #[allow(clippy::too_many_arguments)]
    pub fn allocate_with<T: Clone + PartialEq>(
        &self,
        components: &JMap<T>,
        base_name: &str,
        incoming: &T,
        incoming_ref: &str,
        ref_of: impl Fn(&T) -> Option<String>,
        key_of: impl Fn(&str) -> Option<String>,
        reuse_policy: impl Fn(Option<&T>, &T) -> bool,
    ) -> String {
        let mut suffix = 0u64;
        loop {
            let candidate = if suffix == 0 { base_name.to_string() } else { format!("{base_name}_{suffix}") };
            let Some(existing_key) = key_of(&candidate) else { return candidate };
            let existing = components.get(&existing_key);
            if self.can_reuse(existing.as_ref(), incoming, incoming_ref, &ref_of, &reuse_policy) {
                return existing_key;
            }
            suffix += 1;
        }
    }

    fn can_reuse<T: PartialEq>(&self, existing: Option<&T>, incoming: &T, incoming_ref: &str, ref_of: &impl Fn(&T) -> Option<String>, reuse_policy: &impl Fn(Option<&T>, &T) -> bool) -> bool {
        let existing_ref = existing.and_then(ref_of);
        if let Some(er) = existing_ref {
            return self.cache.refs_are_equivalent(Some(&er), Some(incoming_ref));
        }
        existing == Some(incoming) || reuse_policy(existing, incoming)
    }
}

/// `ComponentNameAllocator.caseInsensitiveKey(components)`
pub(crate) fn case_insensitive_key<T: Clone>(components: &JMap<T>) -> impl Fn(&str) -> Option<String> + '_ {
    move |candidate| {
        if components.contains_key(candidate) {
            return Some(candidate.to_string());
        }
        components.keys().into_iter().find(|n| java_equals_ignore_case(n, candidate))
    }
}

/// Thread-local processor mocks (see the module docs). A handler returning `Some` replaces the call.
pub mod mocks {
    use crate::java::JResult;
    use crate::models::*;
    use crate::parser::ref_utils::RefFormat;
    use std::cell::RefCell;
    use std::rc::Rc;

    /// A mock for a `processXxx(object)` call: `Some` replaces the real call with its result, `None` runs the real one.
    pub type Handler<A> = Rc<dyn Fn(&A) -> Option<JResult<()>>>;
    /// A mock for `ExternalRefProcessor.processRefToExternal<Kind>`, called with (kind, ref, format): `Some` replaces the result, `None` runs the real one.
    pub type ExternalHandler = Rc<dyn Fn(&str, &str, RefFormat) -> Option<JResult<Option<String>>>>;
    /// `processParameters(list)`'s mock: the replacement list, or `None` to run the real one.
    pub type ParametersHandler = Rc<dyn Fn(Option<&crate::java::JList<Parameter>>) -> Option<JResult<Option<crate::java::JList<Parameter>>>>>;
    /// `processSecurityScheme`'s mock: the replacement, or `None` to run the real one.
    pub type SecuritySchemeHandler = Rc<dyn Fn(&SecurityScheme) -> Option<JResult<SecurityScheme>>>;

    /// The processor mocks installed on this thread; every field left `None` keeps the real behaviour.
    #[derive(Default, Clone)]
    pub struct ProcessorMocks {
        /// Called with the class name of every processor constructed (`new XxxProcessor(...)`), when its
        /// constructor starts
        pub on_construct: Option<Rc<dyn Fn(&'static str)>>,
        /// Called when that constructor returns (the processors it constructed itself were reported in
        /// between: a `@Mocked` constructor would not have constructed them)
        pub on_construct_end: Option<Rc<dyn Fn(&'static str)>>,
        /// `SchemaProcessor.processSchema(schema)`.
        pub process_schema: Option<Handler<Schema>>,
        /// `ResponseProcessor.processResponse(response)`.
        pub process_response: Option<Handler<ApiResponse>>,
        /// `RequestBodyProcessor.processRequestBody(requestBody)`.
        pub process_request_body: Option<Handler<RequestBody>>,
        /// `ParameterProcessor.processParameter(parameter)`.
        pub process_parameter: Option<Handler<Parameter>>,
        /// `processParameters(list)`: replaces the result
        pub process_parameters: Option<ParametersHandler>,
        /// `HeaderProcessor.processHeader(header)`.
        pub process_header: Option<Handler<Header>>,
        /// `ExampleProcessor.processExample(example)`.
        pub process_example: Option<Handler<Example>>,
        /// `LinkProcessor.processLink(link)`.
        pub process_link: Option<Handler<Link>>,
        /// `CallbackProcessor.processCallback(callback)`.
        pub process_callback: Option<Handler<Callback>>,
        /// `SecuritySchemeProcessor.processSecurityScheme(securityScheme)`.
        pub process_security_scheme: Option<SecuritySchemeHandler>,
        /// `ExternalRefProcessor.processRefToExternal<Kind>(ref, format)`: (kind, ref, format)
        pub external_ref: Option<ExternalHandler>,
    }

    thread_local! {
        static MOCKS: RefCell<ProcessorMocks> = RefCell::new(ProcessorMocks::default());
    }

    /// Installs `m` as this thread's mocks; the previous ones come back when the returned guard drops.
    pub fn install(m: ProcessorMocks) -> MocksGuard {
        let prev = MOCKS.with(|c| std::mem::replace(&mut *c.borrow_mut(), m));
        MocksGuard { prev: Some(prev) }
    }

    /// Restores the previously installed mocks when dropped.
    pub struct MocksGuard {
        prev: Option<ProcessorMocks>,
    }

    impl Drop for MocksGuard {
        fn drop(&mut self) {
            if let Some(p) = self.prev.take() {
                MOCKS.with(|c| *c.borrow_mut() = p);
            }
        }
    }

    pub(crate) fn current() -> ProcessorMocks {
        MOCKS.with(|c| c.borrow().clone())
    }

    /// Reports a constructor start; the returned guard reports its end when dropped.
    pub(crate) fn constructed(name: &'static str) -> Constructing {
        if let Some(f) = current().on_construct {
            f(name);
        }
        Constructing { name }
    }

    pub(crate) struct Constructing {
        name: &'static str,
    }

    impl Drop for Constructing {
        fn drop(&mut self) {
            if let Some(f) = current().on_construct_end {
                f(self.name);
            }
        }
    }
}
