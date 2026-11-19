//! `io.swagger.v3.parser.ResolverCache`: loads internal and external references, caching files, parsed
//! fragments and renamed references.
//!
//! Processors see the cache through the [`Cache`] trait (the upstream tests substitute the cache with JMockit),
//! with [`ResolverCache`] the real implementation. Java's `loadRef(ref, format, Class<T>)` is
//! [`load_ref`]`::<T>` over [`Cache::load_ref_dyn`], the component types being [`Component`].

use crate::core::{AuthorizationValue, ParseOptions, SwaggerParseResult};
use crate::java::net::JavaUri;
use crate::java::path::{url_decode_strict, JavaPath};
use crate::java::{JMap, JResult, JSet, JavaException};
use crate::models::*;
use crate::parser::deserialization_utils::{deserialize_into_tree, is_json};
use crate::parser::deserializer::{OpenAPIDeserializer, ParseResult};
use crate::parser::path_utils;
use crate::parser::ref_utils::{self, java_split, RefFormat};
use jackson::JsonNode;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use swagger_parser_safe_url_resolver::PermittedUrlsChecker;

/// A resolvable OpenAPI component (the objects `loadRef` returns).
#[derive(Clone, Debug)]
pub enum Component {
    /// A `Schema` (any subclass).
    Schema(Schema),
    /// An `ApiResponse`.
    ApiResponse(ApiResponse),
    /// A `Parameter` (any subclass).
    Parameter(Parameter),
    /// A `RequestBody`.
    RequestBody(RequestBody),
    /// An `Example`.
    Example(Example),
    /// A `Link`.
    Link(Link),
    /// A `Callback`.
    Callback(Callback),
    /// A `Header`.
    Header(Header),
    /// A `SecurityScheme`.
    SecurityScheme(SecurityScheme),
    /// A `PathItem`.
    PathItem(PathItem),
}

/// The `Class<T>` passed to `loadRef`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ComponentKind {
    /// `Schema.class`.
    Schema,
    /// `ApiResponse.class`.
    ApiResponse,
    /// `Parameter.class`.
    Parameter,
    /// `RequestBody.class`.
    RequestBody,
    /// `Example.class`.
    Example,
    /// `Link.class`.
    Link,
    /// `Callback.class`.
    Callback,
    /// `Header.class`.
    Header,
    /// `SecurityScheme.class`.
    SecurityScheme,
    /// `PathItem.class`.
    PathItem,
}

impl ComponentKind {
    /// `Class.getName()`
    pub fn class_name(&self) -> &'static str {
        match self {
            ComponentKind::Schema => "io.swagger.v3.oas.models.media.Schema",
            ComponentKind::ApiResponse => "io.swagger.v3.oas.models.responses.ApiResponse",
            ComponentKind::Parameter => "io.swagger.v3.oas.models.parameters.Parameter",
            ComponentKind::RequestBody => "io.swagger.v3.oas.models.parameters.RequestBody",
            ComponentKind::Example => "io.swagger.v3.oas.models.examples.Example",
            ComponentKind::Link => "io.swagger.v3.oas.models.links.Link",
            ComponentKind::Callback => "io.swagger.v3.oas.models.callbacks.Callback",
            ComponentKind::Header => "io.swagger.v3.oas.models.headers.Header",
            ComponentKind::SecurityScheme => "io.swagger.v3.oas.models.security.SecurityScheme",
            ComponentKind::PathItem => "io.swagger.v3.oas.models.PathItem",
        }
    }
}

impl Component {
    /// The declared class of this component (its `ComponentKind`).
    pub fn kind(&self) -> ComponentKind {
        match self {
            Component::Schema(_) => ComponentKind::Schema,
            Component::ApiResponse(_) => ComponentKind::ApiResponse,
            Component::Parameter(_) => ComponentKind::Parameter,
            Component::RequestBody(_) => ComponentKind::RequestBody,
            Component::Example(_) => ComponentKind::Example,
            Component::Link(_) => ComponentKind::Link,
            Component::Callback(_) => ComponentKind::Callback,
            Component::Header(_) => ComponentKind::Header,
            Component::SecurityScheme(_) => ComponentKind::SecurityScheme,
            Component::PathItem(_) => ComponentKind::PathItem,
        }
    }

    /// `getClass().getName()` of the runtime object
    pub fn runtime_class_name(&self) -> String {
        match self {
            Component::Schema(s) => s.kind().class_name(),
            Component::Parameter(p) => p.kind().class_name().to_string(),
            other => other.kind().class_name().to_string(),
        }
    }
}

/// A component type usable with [`load_ref`].
pub trait ComponentType: Sized + Clone {
    /// The `Class<T>` this type stands for.
    const KIND: ComponentKind;
    /// Unwraps `c`, `None` when it holds another component type.
    fn from_component(c: Component) -> Option<Self>;
    /// Wraps this value as a [`Component`].
    fn into_component(self) -> Component;
}

macro_rules! component_type {
    ($($t:ident),*) => {
        $(impl ComponentType for $t {
            const KIND: ComponentKind = ComponentKind::$t;
            fn from_component(c: Component) -> Option<Self> {
                match c {
                    Component::$t(x) => Some(x),
                    _ => None,
                }
            }
            fn into_component(self) -> Component {
                Component::$t(self)
            }
        })*
    };
}

component_type!(Schema, ApiResponse, Parameter, RequestBody, Example, Link, Callback, Header, SecurityScheme, PathItem);

/// The cache operations the processors use.
pub trait Cache {
    /// `loadRef(ref, refFormat, expectedType)` with the type as a value; the result is already cast (an
    /// internal ref of another type gives `None`, an external one a `ClassCastException`).
    fn load_ref_dyn(&self, r: &str, format: RefFormat, kind: ComponentKind) -> JResult<Option<Component>>;
    /// `getRenamedRef(originalRef)`: the new name given to an external reference, matched by its canonical form; `None` when not renamed.
    fn get_renamed_ref(&self, original_ref: Option<&str>) -> Option<String>;
    /// `putRenamedRef(originalRef, newRef)`: records that `original_ref` was imported as `new_ref`.
    fn put_renamed_ref(&self, original_ref: &str, new_ref: &str);
    /// Whether `key` was recorded with `add_referenced_key` (`referencedModelKeys.contains(key)`).
    fn has_referenced_key(&self, key: &str) -> bool;
    /// `addReferencedKey(key)`: records a model key already imported from an external reference.
    fn add_referenced_key(&self, key: &str);
    /// Whether two references point to the same place once canonicalized.
    fn refs_are_equivalent(&self, first: Option<&str>, second: Option<&str>) -> bool;
    /// `getParseOptions()`: the options the cache was created with.
    fn get_parse_options(&self) -> Option<ParseOptions>;
}

/// `cache.loadRef(ref, format, T.class)`
pub fn load_ref<T: ComponentType>(cache: &dyn Cache, r: &str, format: RefFormat) -> JResult<Option<T>> {
    Ok(cache.load_ref_dyn(r, format, T::KIND)?.and_then(T::from_component))
}

fn class_cast(c: &Component, kind: ComponentKind) -> JavaException {
    JavaException::new("java.lang.ClassCastException", format!("Cannot cast {} to {}", c.runtime_class_name(), kind.class_name()))
}

fn runtime(msg: impl Into<String>) -> JavaException {
    JavaException::new("java.lang.RuntimeException", msg)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pattern {
    Schemas,
    Responses,
    Parameters,
    RequestBodies,
    Examples,
    Links,
    Callbacks,
    Headers,
    SecuritySchemes,
    Paths,
}

impl Pattern {
    fn prefix(&self) -> &'static str {
        match self {
            Pattern::Schemas => "#/components/schemas/",
            Pattern::Responses => "#/components/responses/",
            Pattern::Parameters => "#/components/parameters/",
            Pattern::RequestBodies => "#/components/requestBodies/",
            Pattern::Examples => "#/components/examples/",
            Pattern::Links => "#/components/links/",
            Pattern::Callbacks => "#/components/callbacks/",
            Pattern::Headers => "#/components/headers/",
            Pattern::SecuritySchemes => "#/components/securitySchemes/",
            Pattern::Paths => "#/paths/",
        }
    }

    /// `^prefix(?<name>.+)`: the name (at least one character; `.` does not match line terminators)
    fn matches<'a>(&self, r: &'a str) -> Option<&'a str> {
        let name = r.strip_prefix(self.prefix())?;
        if name.is_empty() || name.chars().any(|c| matches!(c, '\n' | '\r' | '\u{85}' | '\u{2028}' | '\u{2029}')) {
            return None;
        }
        Some(name)
    }
}

/// `unescapePointer`: URL-decode, then `~1` → `/`, `~0` → `~`
pub fn unescape_pointer(e: &str) -> JResult<String> {
    let d = url_decode_strict(e)?;
    Ok(d.replace("~1", "/").replace("~0", "~"))
}

/// `canonicalize(ref)`: `new URI(ref).normalize().toString()`, the ref itself when it is not a URI
pub fn canonicalize(r: Option<&str>) -> Option<String> {
    let r = r?;
    if r.is_empty() {
        return Some(r.to_string());
    }
    match JavaUri::parse(r) {
        Ok(u) => Some(u.normalize().to_java_string()),
        Err(_) => Some(r.to_string()),
    }
}

/// A snapshot of one root component map (`Collections.unmodifiableMap(new LinkedHashMap<>(references))`).
type Snapshot = Vec<(Pattern, Vec<(String, Component)>)>;

#[derive(Default)]
struct State {
    resolution_cache: HashMap<String, Option<Component>>,
    external_file_cache: HashMap<String, Option<String>>,
    canonical_resolution_cache: HashMap<String, Option<Component>>,
    canonical_external_file_cache: HashMap<String, Option<String>>,
    referenced_model_keys: Vec<String>,
    rename_cache: HashMap<String, String>,
    canonical_rename_cache: HashMap<Option<String>, String>,
}

/// A test seam's observer of the URLs `checkUrlIsPermitted` is asked about.
type UrlObserver = Rc<dyn Fn(&str)>;

/// `ResolverCache`
pub struct ResolverCache {
    open_api: Option<OpenAPI>,
    auths: Vec<AuthorizationValue>,
    parent_directory: Option<JavaPath>,
    root_path: Option<String>,
    root_document_uri: Option<JavaUri>,
    root_reference_snapshot: Snapshot,
    state: RefCell<State>,
    /// `resolveValidationMessages` (shared with the `OpenAPIResolver`, a `HashSet`)
    resolve_validation_messages: Option<JSet>,
    parse_options: ParseOptions,
    pub(crate) openapi31: bool,
    permitted_urls_checker: PermittedUrlsChecker,
    /// Test seam for a subclass overriding `checkUrlIsPermitted` (called before the real check)
    check_url_observer: RefCell<Option<UrlObserver>>,
}

impl ResolverCache {
    /// `new ResolverCache(openApi, auths, parentFileLocation)`
    pub fn new(open_api: Option<&OpenAPI>, auths: Option<&[AuthorizationValue]>, parent_file_location: Option<&str>) -> JResult<ResolverCache> {
        ResolverCache::with_options(open_api, auths, parent_file_location, Some(JSet::new_hash()), ParseOptions::default())
    }

    /// `new ResolverCache(openApi, auths, parentFileLocation, resolveValidationMessages, parseOptions)`
    pub fn with_options(
        open_api: Option<&OpenAPI>,
        auths: Option<&[AuthorizationValue]>,
        parent_file_location: Option<&str>,
        resolve_validation_messages: Option<JSet>,
        parse_options: ParseOptions,
    ) -> JResult<ResolverCache> {
        let openapi31 = open_api.and_then(|o| o.get_openapi()).is_some_and(|v| v.starts_with("3.1"));
        let root_document_uri = path_utils::root_document_uri(parent_file_location);
        let root_reference_snapshot = snapshot_root_references(open_api);
        let permitted_urls_checker = crate::parser::hooks::permitted_urls_checker(parse_options.remote_ref_allow_list.as_deref(), parse_options.remote_ref_block_list.as_deref())?;
        let parent_directory = match parent_file_location {
            Some(loc) => {
                if path_utils::is_http_uri(root_document_uri.as_ref()) || loc.starts_with("jar") {
                    None
                } else if root_document_uri.as_ref().and_then(|u| u.get_scheme()).is_some_and(|s| s.eq_ignore_ascii_case("file")) {
                    path_utils::parent_directory_of_uri(root_document_uri.as_ref().unwrap())
                } else {
                    path_utils::get_parent_directory_of_file(loc)?
                }
            }
            None => Some(JavaPath::new(".")),
        };
        Ok(ResolverCache {
            open_api: open_api.cloned(),
            auths: auths.map(|a| a.to_vec()).unwrap_or_default(),
            parent_directory,
            root_path: parent_file_location.map(str::to_string),
            root_document_uri,
            root_reference_snapshot,
            state: RefCell::new(State::default()),
            resolve_validation_messages,
            parse_options,
            openapi31,
            permitted_urls_checker,
            check_url_observer: RefCell::new(None),
        })
    }

    /// The private `rootDocumentUri` (read reflectively by the upstream tests)
    pub fn root_document_uri(&self) -> Option<&JavaUri> {
        self.root_document_uri.as_ref()
    }

    /// The private `parentDirectory`: the directory relative file references resolve against, `None` when the root is a remote URL or inside a jar, `.` when there is no root location.
    pub fn parent_directory(&self) -> Option<&JavaPath> {
        self.parent_directory.as_ref()
    }

    /// `loadRef(ref, refFormat, T.class)`
    pub fn load_ref<T: ComponentType>(&self, r: &str, format: RefFormat) -> JResult<Option<T>> {
        load_ref(self, r, format)
    }

    fn load_ref_impl(&self, r: &str, format: RefFormat, kind: ComponentKind) -> JResult<Option<Component>> {
        if format == RefFormat::Internal {
            let loaded = self.load_internal_ref(r)?;
            return Ok(loaded.filter(|c| c.kind() == kind));
        }
        let parts = java_split(r, "#/");
        if parts.len() > 2 {
            return Err(runtime(format!("Invalid ref format: {r}")));
        }
        let file = parts.first().cloned().unwrap_or_default();
        let definition_path = if parts.len() == 2 { Some(parts[1].clone()) } else { None };
        let canonical_ref = canonicalize(Some(r)).unwrap();
        let canonical_file = canonicalize(Some(&file)).unwrap();

        let previously = self.state.borrow().canonical_resolution_cache.get(&canonical_ref).cloned().flatten();
        if let Some(prev) = previously {
            {
                let mut st = self.state.borrow_mut();
                st.resolution_cache.entry(r.to_string()).or_insert_with(|| Some(prev.clone()));
                let cf = st.canonical_external_file_cache.get(&canonical_file).cloned().flatten();
                if !st.external_file_cache.get(&file).is_some_and(|v| v.is_some()) {
                    st.external_file_cache.insert(file.clone(), cf);
                }
            }
            // `expectedType.getClass().equals(previouslyResolvedEntity.getClass())` is never true for Header
            if kind != ComponentKind::Header {
                if prev.kind() != kind {
                    return Err(class_cast(&prev, kind));
                }
                return Ok(Some(prev));
            }
        }

        let mut contents = self.state.borrow().canonical_external_file_cache.get(&canonical_file).cloned().flatten();
        if contents.is_none() && self.parse_options.safely_resolve_url {
            self.check_url_is_permitted(&file)?;
        }
        if let Some(t) = self.resolve_root_reference(&file, definition_path.as_deref(), r, &canonical_ref, kind)? {
            return Ok(Some(t));
        }
        if contents.is_none() {
            let checker = Some(&self.permitted_urls_checker);
            if self.parent_directory.is_some() {
                contents = ref_utils::read_external_ref(&file, format, &self.auths, self.parent_directory.as_ref(), checker)?;
            } else if self.root_path.is_some()
                && (path_utils::is_http_uri(self.root_document_uri.as_ref()) || self.root_path.as_deref().unwrap().starts_with("http"))
            {
                contents = ref_utils::read_external_url_ref(&file, format, &self.auths, self.root_path.as_deref(), checker)?;
            } else if self.root_path.is_some() {
                contents = ref_utils::read_external_classpath_ref(&file, format, &self.auths, self.root_path.as_deref(), checker)?;
            }
            self.state.borrow_mut().canonical_external_file_cache.insert(canonical_file.clone(), contents.clone());
        }
        {
            let mut st = self.state.borrow_mut();
            if !st.external_file_cache.get(&file).is_some_and(|v| v.is_some()) {
                st.external_file_cache.insert(file.clone(), contents.clone());
            }
        }

        let mut du_result = SwaggerParseResult::new();
        let contents_str = contents.clone();
        let mut tree = match &contents_str {
            Some(c) => deserialize_into_tree(c, Some(&file), Some(&self.parse_options), &mut du_result)?,
            // `isJson(null)` → NPE
            None => return Err(JavaException::npe("Cannot invoke \"String.trim()\" because \"contents\" is null")),
        };

        let Some(definition_path) = definition_path else {
            let result = if self.parse_options.validate_external_refs {
                self.deserialize_fragment(&tree, kind, &file, "/")?
            } else {
                deserialize_value(DeserializeSource::Text(contents_str.as_deref().unwrap()), &file, kind, self.openapi31)?
            };
            {
                let mut st = self.state.borrow_mut();
                st.resolution_cache.insert(r.to_string(), result.clone());
                st.canonical_resolution_cache.insert(canonical_ref.clone(), result.clone());
            }
            self.add_validation_messages(&du_result);
            return Ok(result);
        };

        for element in java_split(&definition_path, "/") {
            let next = if let JsonNode::Array(a) = &tree {
                match element.parse::<i32>() {
                    Ok(i) if i >= 0 => a.get(i as usize).cloned(),
                    Ok(_) => None,
                    // NumberFormatException: tree unchanged
                    Err(_) => Some(tree.clone()),
                }
            } else {
                let key = unescape_pointer(&element)?;
                tree.get(&key).cloned()
            };
            match next {
                Some(n) => tree = n,
                None => return Err(runtime(format!("Could not find {definition_path} in contents of {file}"))),
            }
        }
        let result = if self.parse_options.validate_external_refs {
            self.deserialize_fragment(&tree, kind, &file, &definition_path)?
        } else if kind == ComponentKind::Schema {
            let obj = as_object_node(&tree)?;
            let mut pr = ParseResult::new().openapi31(self.openapi31);
            OpenAPIDeserializer::new().get_schema(Some(obj), &definition_path.replace('/', "."), Some(&mut pr))?.map(Component::Schema)
        } else {
            deserialize_value(DeserializeSource::Tree(&tree), &file, kind, self.openapi31)?
        };
        if let Some(res) = &result {
            update_local_refs(&file, res);
        }
        {
            let mut st = self.state.borrow_mut();
            st.resolution_cache.insert(r.to_string(), result.clone());
            st.canonical_resolution_cache.insert(canonical_ref, result.clone());
        }
        self.add_validation_messages(&du_result);
        Ok(result)
    }

    fn add_validation_messages(&self, du_result: &SwaggerParseResult) {
        if let (Some(msgs), Some(set)) = (&du_result.messages, &self.resolve_validation_messages) {
            for m in msgs {
                set.add(m.clone());
            }
        }
    }

    fn resolve_root_reference(&self, file: &str, definition_path: Option<&str>, r: &str, canonical_ref: &str, kind: ComponentKind) -> JResult<Option<Component>> {
        let Some(dp) = definition_path else { return Ok(None) };
        if !self.is_root_document(file) {
            return Ok(None);
        }
        let Some(target) = self.load_root_reference(&format!("#/{dp}"))? else { return Ok(None) };
        if target.kind() != kind {
            return Ok(None);
        }
        let mut st = self.state.borrow_mut();
        st.resolution_cache.insert(r.to_string(), Some(target.clone()));
        if !st.canonical_resolution_cache.get(canonical_ref).is_some_and(|v| v.is_some()) {
            st.canonical_resolution_cache.insert(canonical_ref.to_string(), Some(target.clone()));
        }
        Ok(Some(target))
    }

    fn is_root_document(&self, file: &str) -> bool {
        let Some(root) = &self.root_document_uri else { return false };
        if file.is_empty() {
            return false;
        }
        let Ok(f) = JavaUri::parse(file) else { return false };
        let target = root.resolve(&f).normalize();
        if target.is_opaque() {
            return false;
        }
        match path_utils::without_fragment(Some(&target)) {
            Some(t) => root.java_equals(&t),
            None => false,
        }
    }

    fn load_root_reference(&self, r: &str) -> JResult<Option<Component>> {
        for (pattern, entries) in &self.root_reference_snapshot {
            if let Some(name) = pattern.matches(r) {
                let name = unescape_pointer(name)?;
                if let Some((_, c)) = entries.iter().find(|(k, _)| *k == name) {
                    return Ok(Some(c.clone()));
                }
            }
        }
        Ok(None)
    }

    fn deserialize_fragment(&self, node: &JsonNode, kind: ComponentKind, file: &str, definition_path: &str) -> JResult<Option<Component>> {
        let mut d = OpenAPIDeserializer::new();
        let mut pr = ParseResult::new();
        let loc = definition_path.replace('/', ".");
        let obj = as_object_node(node)?;
        let result: Option<Component> = match kind {
            ComponentKind::Schema => d.get_schema(Some(obj), &loc, Some(&mut pr))?.map(Component::Schema),
            ComponentKind::RequestBody => d.get_request_body(Some(obj), &loc, &mut pr)?.map(Component::RequestBody),
            ComponentKind::ApiResponse => d.get_response(Some(obj), &loc, &mut pr)?.map(Component::ApiResponse),
            ComponentKind::Callback => d.get_callback(Some(obj), &loc, &mut pr)?.map(Component::Callback),
            ComponentKind::Example => d.get_example(Some(obj), &loc, &mut pr)?.map(Component::Example),
            ComponentKind::Header => d.get_header(Some(obj), &loc, &mut pr)?.map(Component::Header),
            ComponentKind::Link => d.get_link(Some(obj), &loc, &mut pr)?.map(Component::Link),
            ComponentKind::Parameter => d.get_parameter(Some(obj), &loc, &mut pr)?.map(Component::Parameter),
            ComponentKind::SecurityScheme => d.get_security_scheme(Some(obj), &loc, &mut pr)?.map(Component::SecurityScheme),
            ComponentKind::PathItem => d.get_path_item(obj, &loc, &mut pr)?.map(Component::PathItem),
        };
        if let Some(set) = &self.resolve_validation_messages {
            for m in pr.get_messages() {
                set.add(format!("{m} ({file})"));
            }
        } else if !pr.get_messages().is_empty() {
            return Err(JavaException::npe("Cannot invoke \"java.util.Set.add(Object)\" because \"this.resolveValidationMessages\" is null"));
        }
        if result.is_some() {
            return Ok(result);
        }
        deserialize_value(DeserializeSource::Tree(node), file, kind, false)
    }

    fn load_internal_ref(&self, r: &str) -> JResult<Option<Component>> {
        let Some(api) = &self.open_api else {
            let getter = if r.starts_with("#/paths") { "getPaths" } else { "getComponents" };
            return Err(JavaException::npe(format!("Cannot invoke \"io.swagger.v3.oas.models.OpenAPI.{getter}()\" because \"this.openApi\" is null")));
        };
        if r.starts_with("#/paths") {
            let paths = api.get_paths();
            return get_from_map(r, Pattern::Paths, |k| paths.as_ref().map(|p| p.get(k).map(Component::PathItem)));
        }
        let Some(c) = api.get_components() else { return Ok(None) };
        macro_rules! lookup {
            ($getter:ident, $pat:expr, $variant:ident) => {{
                let m = c.$getter();
                get_from_map(r, $pat, |k| m.as_ref().map(|m| m.get(k).map(Component::$variant)))
            }};
        }
        if r.starts_with("#/components/schemas") {
            lookup!(get_schemas, Pattern::Schemas, Schema)
        } else if r.starts_with("#/components/requestBodies") {
            lookup!(get_request_bodies, Pattern::RequestBodies, RequestBody)
        } else if r.starts_with("#/components/examples") {
            lookup!(get_examples, Pattern::Examples, Example)
        } else if r.starts_with("#/components/responses") {
            lookup!(get_responses, Pattern::Responses, ApiResponse)
        } else if r.starts_with("#/components/parameters") {
            lookup!(get_parameters, Pattern::Parameters, Parameter)
        } else if r.starts_with("#/components/links") {
            lookup!(get_links, Pattern::Links, Link)
        } else if r.starts_with("#/components/headers") {
            lookup!(get_headers, Pattern::Headers, Header)
        } else if r.starts_with("#/components/callbacks") {
            lookup!(get_callbacks, Pattern::Callbacks, Callback)
        } else if r.starts_with("#/components/securitySchemes") {
            lookup!(get_security_schemes, Pattern::SecuritySchemes, SecurityScheme)
        } else {
            Ok(None)
        }
    }

    /// `checkUrlIsPermitted(refSet)`
    fn check_url_is_permitted(&self, ref_set: &str) -> JResult<()> {
        let observer = self.check_url_observer.borrow().clone();
        if let Some(f) = observer {
            f(ref_set);
        }
        match self.permitted_urls_checker.verify(ref_set) {
            Ok(_) => Ok(()),
            Err(e) if e.class == swagger_parser_safe_url_resolver::HOST_DENIED_EXCEPTION => Err(JavaException {
                class: "java.lang.RuntimeException".into(),
                message: e.message.clone(),
                cause: None,
            }),
            Err(e) => Err(e),
        }
    }

    /// Replaces the `PermittedUrlsChecker` built from the parse options (e.g. one with a stub `NetUtils`, so a
    /// test resolves host names without the network)
    pub fn with_permitted_urls_checker(mut self, checker: PermittedUrlsChecker) -> ResolverCache {
        self.permitted_urls_checker = checker;
        self
    }

    /// Observes `checkUrlIsPermitted(refSet)` calls (what an overriding subclass calling `super` sees)
    pub fn set_check_url_is_permitted_observer(&self, f: Rc<dyn Fn(&str)>) {
        *self.check_url_observer.borrow_mut() = Some(f);
    }

    /// The private `canonicalResolutionCache` (read reflectively by the upstream tests)
    pub fn canonical_resolution_cache(&self) -> HashMap<String, Option<Component>> {
        self.state.borrow().canonical_resolution_cache.clone()
    }

    /// `canonicalResolutionCache.put(key, value)` (written reflectively by the upstream tests)
    pub fn put_canonical_resolution(&self, key: &str, value: Option<Component>) {
        self.state.borrow_mut().canonical_resolution_cache.insert(key.to_string(), value);
    }

    /// `getResolutionCache()`
    pub fn get_resolution_cache(&self) -> HashMap<String, Option<Component>> {
        self.state.borrow().resolution_cache.clone()
    }

    /// `getExternalFileCache()`
    pub fn get_external_file_cache(&self) -> HashMap<String, Option<String>> {
        self.state.borrow().external_file_cache.clone()
    }

    /// `getRenameCache()`
    pub fn get_rename_cache(&self) -> HashMap<String, String> {
        self.state.borrow().rename_cache.clone()
    }

    /// The options the cache was created with.
    pub fn parse_options(&self) -> &ParseOptions {
        &self.parse_options
    }
}

impl Cache for ResolverCache {
    fn load_ref_dyn(&self, r: &str, format: RefFormat, kind: ComponentKind) -> JResult<Option<Component>> {
        self.load_ref_impl(r, format, kind)
    }

    fn get_renamed_ref(&self, original_ref: Option<&str>) -> Option<String> {
        self.state.borrow().canonical_rename_cache.get(&canonicalize(original_ref)).cloned()
    }

    fn put_renamed_ref(&self, original_ref: &str, new_ref: &str) {
        let mut st = self.state.borrow_mut();
        st.rename_cache.insert(original_ref.to_string(), new_ref.to_string());
        st.canonical_rename_cache.insert(canonicalize(Some(original_ref)), new_ref.to_string());
    }

    fn has_referenced_key(&self, key: &str) -> bool {
        self.state.borrow().referenced_model_keys.iter().any(|k| k == key)
    }

    fn add_referenced_key(&self, key: &str) {
        self.state.borrow_mut().referenced_model_keys.push(key.to_string());
    }

    fn refs_are_equivalent(&self, first: Option<&str>, second: Option<&str>) -> bool {
        canonicalize(first) == canonicalize(second)
    }

    fn get_parse_options(&self) -> Option<ParseOptions> {
        Some(self.parse_options.clone())
    }
}

impl Cache for Rc<ResolverCache> {
    fn load_ref_dyn(&self, r: &str, format: RefFormat, kind: ComponentKind) -> JResult<Option<Component>> {
        (**self).load_ref_dyn(r, format, kind)
    }
    fn get_renamed_ref(&self, original_ref: Option<&str>) -> Option<String> {
        (**self).get_renamed_ref(original_ref)
    }
    fn put_renamed_ref(&self, original_ref: &str, new_ref: &str) {
        (**self).put_renamed_ref(original_ref, new_ref)
    }
    fn has_referenced_key(&self, key: &str) -> bool {
        (**self).has_referenced_key(key)
    }
    fn add_referenced_key(&self, key: &str) {
        (**self).add_referenced_key(key)
    }
    fn refs_are_equivalent(&self, first: Option<&str>, second: Option<&str>) -> bool {
        (**self).refs_are_equivalent(first, second)
    }
    fn get_parse_options(&self) -> Option<ParseOptions> {
        (**self).get_parse_options()
    }
}

fn get_from_map(r: &str, pattern: Pattern, lookup: impl FnOnce(&str) -> Option<Option<Component>>) -> JResult<Option<Component>> {
    match pattern.matches(r) {
        Some(name) => {
            let name = unescape_pointer(name)?;
            Ok(lookup(&name).flatten())
        }
        None => Ok(None),
    }
}

fn snapshot_root_references(api: Option<&OpenAPI>) -> Snapshot {
    let mut snap: Snapshot = Vec::new();
    let Some(api) = api else { return snap };
    if let Some(p) = api.get_paths() && !p.is_empty() {
        snap.push((Pattern::Paths, p.entries().into_iter().map(|(k, v)| (k, Component::PathItem(v))).collect()));
    }
    if let Some(c) = api.get_components() {
        fn add<V: Clone>(snap: &mut Snapshot, p: Pattern, m: Option<JMap<V>>, f: fn(V) -> Component) {
            if let Some(m) = m && !m.is_empty() {
                snap.push((p, m.entries().into_iter().map(|(k, v)| (k, f(v))).collect()));
            }
        }
        add(&mut snap, Pattern::Schemas, c.get_schemas(), Component::Schema);
        add(&mut snap, Pattern::Responses, c.get_responses(), Component::ApiResponse);
        add(&mut snap, Pattern::Parameters, c.get_parameters(), Component::Parameter);
        add(&mut snap, Pattern::RequestBodies, c.get_request_bodies(), Component::RequestBody);
        add(&mut snap, Pattern::Examples, c.get_examples(), Component::Example);
        add(&mut snap, Pattern::Links, c.get_links(), Component::Link);
        add(&mut snap, Pattern::Callbacks, c.get_callbacks(), Component::Callback);
        add(&mut snap, Pattern::Headers, c.get_headers(), Component::Header);
        add(&mut snap, Pattern::SecuritySchemes, c.get_security_schemes(), Component::SecurityScheme);
    }
    snap
}

/// `(ObjectNode) node`
fn as_object_node(node: &JsonNode) -> JResult<&JsonNode> {
    if matches!(node, JsonNode::Object(_)) {
        Ok(node)
    } else {
        let from = format!("com.fasterxml.jackson.databind.node.{}", node.node_class_name());
        Err(JavaException::class_cast(&from, "com.fasterxml.jackson.databind.node.ObjectNode"))
    }
}

/// `updateLocalRefs(file, result)`
fn update_local_refs(file: &str, result: &Component) {
    match result {
        Component::Parameter(p) => {
            if let Some(s) = p.get_schema() {
                update_local_refs_schema(file, &s);
            }
        }
        Component::Schema(s) => update_local_refs_schema(file, s),
        _ => {}
    }
}

fn update_local_refs_schema(file: &str, schema: &Schema) {
    if let Some(r) = schema.get_ref() {
        schema.set_ref(merge(file, &r));
    } else if let Some(props) = schema.get_properties() {
        for p in props.values() {
            update_local_refs_schema(file, &p);
        }
    }
}

/// `merge(host, ref)`
fn merge(host: &str, r: &str) -> String {
    if host.trim().is_empty() {
        return r.to_string();
    }
    if r.starts_with("http:") || r.starts_with("https:") {
        return r.to_string();
    }
    if !host.starts_with("http:") && !host.starts_with("https:") && !r.starts_with("#/components") {
        return r.to_string();
    }
    if r.starts_with('.') {
        return r.to_string();
    }
    if host.ends_with('/') && r.starts_with('/') {
        return format!("{host}{}", &r[1..]);
    }
    format!("{host}{r}")
}

/// What `DeserializationUtils.deserialize` receives.
pub enum DeserializeSource<'a> {
    /// The document text, still to be parsed.
    Text(&'a str),
    /// An already parsed tree.
    Tree(&'a JsonNode),
}

/// `DeserializationUtils.deserialize(contents, fileOrHost, expectedType, openapi31)`
pub fn deserialize_value(src: DeserializeSource<'_>, file_or_host: &str, kind: ComponentKind, openapi31: bool) -> JResult<Option<Component>> {
    let mapper = if openapi31 { Mapper::Json31 } else { Mapper::Json30 };
    let attempt = || -> Result<Option<Component>, JavaException> {
        let tree = match src {
            DeserializeSource::Tree(t) => t.clone(),
            DeserializeSource::Text(s) => {
                if is_json(s) {
                    jackson::json::read_tree(s, Default::default())?
                } else {
                    jackson::yaml::read_tree(s, jackson::yaml::YamlReadFeatures::swagger_core())?
                }
            }
        };
        Ok(convert_value(&tree, kind, mapper)?)
    };
    attempt().map_err(|e| {
        runtime(format!("An exception was thrown while trying to deserialize the contents of {file_or_host} into type class {}", kind.class_name())).with_cause(e)
    })
}

/// `mapper.convertValue(node, T.class)` (a missing/null node gives `null`)
pub fn convert_value(tree: &JsonNode, kind: ComponentKind, m: Mapper) -> Result<Option<Component>, jackson::JacksonError> {
    use crate::models::deserialize::FromTree;
    if matches!(tree, JsonNode::Null | JsonNode::Missing) {
        return Ok(None);
    }
    Ok(match kind {
        ComponentKind::Schema => Schema::from_tree_opt(tree, m)?.map(Component::Schema),
        ComponentKind::ApiResponse => ApiResponse::from_tree_opt(tree, m)?.map(Component::ApiResponse),
        ComponentKind::Parameter => Parameter::from_tree_opt(tree, m)?.map(Component::Parameter),
        ComponentKind::RequestBody => RequestBody::from_tree_opt(tree, m)?.map(Component::RequestBody),
        ComponentKind::Example => Example::from_tree_opt(tree, m)?.map(Component::Example),
        ComponentKind::Link => Link::from_tree_opt(tree, m)?.map(Component::Link),
        ComponentKind::Callback => Callback::from_tree_opt(tree, m)?.map(Component::Callback),
        ComponentKind::Header => Header::from_tree_opt(tree, m)?.map(Component::Header),
        ComponentKind::SecurityScheme => SecurityScheme::from_tree_opt(tree, m)?.map(Component::SecurityScheme),
        ComponentKind::PathItem => PathItem::from_tree_opt(tree, m)?.map(Component::PathItem),
    })
}
