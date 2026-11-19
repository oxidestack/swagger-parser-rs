//! `io.swagger.v3.parser.reference`: the OpenAPI 3.1 dereferencer (`OpenAPIDereferencer31`,
//! `OpenAPI31Traverser`, `IdsTraverser`, `ReferenceVisitor`, `ReferenceUtils`).
//!
//! The traversers keep Java's `HashSet<Object> visiting` / `HashMap<Object, Object> visitedMap`
//! semantics: keys are compared with the models' `equals()` and located by the `hashCode()` they had when
//! inserted (see `traverser::HashIndex`), so equal-but-distinct objects hit the caches and mutated keys go
//! stale exactly as in Java.

mod traverser;
mod utils;

pub use traverser::{IdsTraverser, OpenAPI31Traverser};
pub use utils::*;

use crate::core::{AuthorizationValue, ParseOptions, SwaggerParseResult};
use crate::java::{JResult, JavaException};
use crate::models::*;
use indexmap::IndexMap;
use jackson::JsonNode;
use std::cell::RefCell;
use std::rc::Rc;
use swagger_parser_safe_url_resolver::PermittedUrlsChecker;

/// `Reference`: a loaded document (URI, tree) sharing the reference set and message set.
#[derive(Clone)]
pub struct Reference(pub Rc<RefCell<ReferenceData>>);

/// The state behind a [`Reference`] handle.
pub struct ReferenceData {
    /// `getUri()`: the document's base URI, `None` for a fresh reference.
    pub uri: Option<String>,
    /// `getDepth()`: the nesting depth of this reference.
    pub depth: i32,
    /// `getMessages()`: the dereference messages, shared by every reference of the run.
    pub messages: Messages,
    /// `getReferenceSet()`: the documents loaded so far, by base URI.
    pub reference_set: ReferenceSet,
    /// `getJsonNode()`: the document's parsed tree.
    pub json_node: Option<JsonNode>,
    /// `getAuths()`: the authorization values used to fetch remote documents.
    pub auths: Option<Vec<AuthorizationValue>>,
}

/// `LinkedHashMap<String, Reference>` shared by all references of one dereference run
pub type ReferenceSet = Rc<RefCell<IndexMap<String, Reference>>>;

/// `LinkedHashSet<String>` (may hold `null`)
#[derive(Clone, Default)]
pub struct Messages(pub Rc<RefCell<Vec<Option<String>>>>);

impl Messages {
    /// `add(message)`: appends the message unless an equal one is already there.
    pub fn add(&self, m: Option<String>) {
        let mut v = self.0.borrow_mut();
        if !v.contains(&m) {
            v.push(m);
        }
    }

    /// The messages in insertion order (a snapshot).
    pub fn items(&self) -> Vec<Option<String>> {
        self.0.borrow().clone()
    }
}

impl Reference {
    /// `new Reference()`: no URI or tree, depth 0, fresh message and reference sets.
    pub fn new() -> Reference {
        Reference(Rc::new(RefCell::new(ReferenceData {
            uri: None,
            depth: 0,
            messages: Messages::default(),
            reference_set: Rc::new(RefCell::new(IndexMap::new())),
            json_node: None,
            auths: None,
        })))
    }

    /// `getUri()`: the document's base URI.
    pub fn uri(&self) -> Option<String> {
        self.0.borrow().uri.clone()
    }

    /// `getMessages()`: the shared message set.
    pub fn messages(&self) -> Messages {
        self.0.borrow().messages.clone()
    }

    /// `getReferenceSet()`: the shared set of loaded documents.
    pub fn reference_set(&self) -> ReferenceSet {
        self.0.borrow().reference_set.clone()
    }

    /// `getJsonNode()`: the document's parsed tree.
    pub fn json_node(&self) -> Option<JsonNode> {
        self.0.borrow().json_node.clone()
    }

    /// `getAuths()`: the authorization values for remote fetches.
    pub fn auths(&self) -> Option<Vec<AuthorizationValue>> {
        self.0.borrow().auths.clone()
    }
}

impl Default for Reference {
    fn default() -> Self {
        Reference::new()
    }
}

/// `DereferencerContext`
pub struct DereferencerContext {
    /// `getOpenApi()`: the document being dereferenced.
    pub open_api: Option<OpenAPI>,
    /// `getAuths()`: the authorization values for remote fetches.
    pub auths: Option<Vec<AuthorizationValue>>,
    /// `getRootUri()`: the location of the root document.
    pub root_uri: RefCell<Option<String>>,
    /// `getParseOptions()`: the options the parse runs with.
    pub parse_options: ParseOptions,
    /// `getProvidedBaseUri()`: a base URI supplied by the caller, `None` by default.
    pub provided_base_uri: Option<String>,
    /// `getAddParametersToEachOperation()`: copy path-level parameters onto each operation. Default `true`.
    pub add_parameters_to_each_operation: bool,
    /// `getCurrentUri()`: the location of the document being traversed, starting at the root URI.
    pub current_uri: RefCell<Option<String>>,
    /// `idsCache` (`LinkedHashMap<String, String>`)
    pub ids_cache: RefCell<IndexMap<String, String>>,
}

impl DereferencerContext {
    /// `new DereferencerContext(result, auths, rootUri, parseOptions)`: the context for dereferencing `result.openapi`.
    pub fn new(result: &SwaggerParseResult, auths: Option<&[AuthorizationValue]>, root_uri: Option<&str>, parse_options: &ParseOptions) -> DereferencerContext {
        DereferencerContext {
            open_api: result.openapi.clone(),
            auths: auths.map(|a| a.to_vec()),
            root_uri: RefCell::new(root_uri.map(str::to_string)),
            parse_options: parse_options.clone(),
            provided_base_uri: None,
            add_parameters_to_each_operation: true,
            current_uri: RefCell::new(root_uri.map(str::to_string)),
            ids_cache: RefCell::new(IndexMap::new()),
        }
    }

    /// `getRootUri()`: the location of the root document.
    pub fn get_root_uri(&self) -> Option<String> {
        self.root_uri.borrow().clone()
    }

    /// `getCurrentUri()`: the location of the document being traversed.
    pub fn get_current_uri(&self) -> Option<String> {
        self.current_uri.borrow().clone()
    }
}

/// A `readHttp` override (what the upstream tests' `CustomOpenAPIDereferencer` installs through the
/// `OpenAPIDereferencer` service loader): returns `Some` to answer the request.
pub type HttpReader = Rc<dyn Fn(&str, Option<&[AuthorizationValue]>) -> Option<JResult<String>>>;

thread_local! {
    static HTTP_READERS: RefCell<Vec<HttpReader>> = const { RefCell::new(Vec::new()) };
}

/// `DereferencersFactory.getInstance().addDereferencer(...)` for dereferencers that only customize
/// `readHttp` (the only extension point the upstream code exercises). Per thread.
pub fn add_http_reader(r: HttpReader) {
    HTTP_READERS.with(|h| h.borrow_mut().insert(0, r));
}

/// Removes every reader installed with [`add_http_reader`] on this thread.
pub fn clear_http_readers() {
    HTTP_READERS.with(|h| h.borrow_mut().clear());
}

/// `ReferenceVisitor`
pub struct ReferenceVisitor {
    /// The document `$ref`s are resolved against.
    pub reference: Reference,
    /// The traverser that walks each resolved object.
    pub traverser: Rc<OpenAPI31Traverser>,
    /// The dereference context; `None` makes schema `$id` lookups fail with a `NullPointerException`.
    pub context: Option<Rc<DereferencerContext>>,
    permitted_urls_checker: Option<PermittedUrlsChecker>,
}

fn runtime(msg: impl Into<String>) -> JavaException {
    JavaException::new("java.lang.RuntimeException", msg)
}

type Deser<T> = fn(&OpenAPI31Traverser, &Option<JsonNode>, &str, Option<&str>, &Messages) -> JResult<Option<T>>;
type Trav<T> = fn(&OpenAPI31Traverser, Option<T>, &ReferenceVisitor) -> JResult<Option<T>>;

impl ReferenceVisitor {
    /// `new ReferenceVisitor(...)`: a visitor resolving `$ref`s against `reference`; `Err` when the context's remote-ref allow/block lists are invalid.
    pub fn new(reference: Reference, traverser: Rc<OpenAPI31Traverser>, context: Option<Rc<DereferencerContext>>) -> JResult<ReferenceVisitor> {
        let checker = match &context {
            Some(c) => Some(crate::parser::hooks::permitted_urls_checker(c.parse_options.remote_ref_allow_list.as_deref(), c.parse_options.remote_ref_block_list.as_deref())?),
            None => None,
        };
        Ok(ReferenceVisitor { reference, traverser, context, permitted_urls_checker: checker })
    }

    fn ctx(&self) -> JResult<&Rc<DereferencerContext>> {
        self.context.as_ref().ok_or_else(|| JavaException::npe("Cannot invoke \"io.swagger.v3.parser.reference.DereferencerContext.getIdsCache()\" because \"this.context\" is null"))
    }

    /// `toBaseURI(uri)`
    pub fn to_base_uri(&self, uri: &str) -> JResult<String> {
        let base = to_base_uri(uri)?;
        resolve(Some(&base), self.reference.uri().as_deref())
    }

    /// `toReference(uri)`
    pub fn to_reference(&self, uri: &str) -> JResult<Reference> {
        let base_uri = self.to_base_uri(uri)?;
        let set = self.reference.reference_set();
        let existing = set.borrow().get(&base_uri).cloned();
        if let Some(r) = existing {
            return Ok(r);
        }
        let node = self.parse(&base_uri, self.reference.auths().as_deref())?;
        let r = Reference::new();
        {
            let mut d = r.0.borrow_mut();
            d.auths = self.reference.auths();
            d.json_node = Some(node);
            d.uri = Some(base_uri.clone());
            d.reference_set = set.clone();
            d.messages = self.reference.messages();
        }
        set.borrow_mut().insert(base_uri, r.clone());
        Ok(r)
    }

    /// `toSchemaReference(baseUri, node)`
    pub fn to_schema_reference(&self, base_uri: &str, node: JsonNode) -> Reference {
        let set = self.reference.reference_set();
        let existing = set.borrow().get(base_uri).cloned();
        if let Some(r) = existing {
            return r;
        }
        let r = Reference::new();
        {
            let mut d = r.0.borrow_mut();
            d.auths = self.reference.auths();
            d.json_node = Some(node);
            d.uri = Some(base_uri.to_string());
            d.reference_set = set.clone();
            d.messages = self.reference.messages();
        }
        set.borrow_mut().insert(base_uri.to_string(), r.clone());
        r
    }

    fn child(&self, reference: Reference) -> JResult<ReferenceVisitor> {
        ReferenceVisitor::new(reference, self.traverser.clone(), self.context.clone())
    }

    /// `resolveRef(visiting, ref, clazz, traverseFunction)`: any exception becomes a message and `null`.
    fn resolve_ref<T>(&self, r: &str, deserialize: Deser<T>, traverse: Trav<T>) -> JResult<Option<T>> {
        let attempt = || -> JResult<Option<T>> {
            let reference_object = self.to_reference(r)?;
            let fragment = get_fragment(r)?;
            let node = json_pointer_evaluate(fragment.as_deref(), reference_object.json_node().as_ref(), r)?;
            let resolved = deserialize(&self.traverser, &node, r, fragment.as_deref(), &reference_object.messages())?;
            let visitor = self.child(reference_object)?;
            traverse(&self.traverser, resolved, &visitor)
        };
        match attempt() {
            Ok(v) => Ok(v),
            Err(e) => {
                self.reference.messages().add(e.message.clone());
                Ok(None)
            }
        }
    }

    /// `visitPathItem(pathItem)`: the path item its `$ref` points to, `None` without a `$ref` or when resolving fails (the failure becomes a message).
    pub fn visit_path_item(&self, p: &PathItem) -> JResult<Option<PathItem>> {
        let Some(r) = p.get_ref().filter(|r| !r.trim().is_empty()) else { return Ok(None) };
        self.resolve_ref(&r, OpenAPI31Traverser::deserialize_fragment::<PathItem>, OpenAPI31Traverser::traverse_path_item)
    }

    /// `visitParameter(parameter)`: the parameter its `$ref` points to, `None` without a `$ref` or when resolving fails (the failure becomes a message).
    pub fn visit_parameter(&self, p: &Parameter) -> JResult<Option<Parameter>> {
        let Some(r) = p.get_ref().filter(|r| !r.trim().is_empty()) else { return Ok(None) };
        self.resolve_ref(&r, OpenAPI31Traverser::deserialize_fragment::<Parameter>, OpenAPI31Traverser::traverse_parameter)
    }

    /// `visitExample(example)`: the example its `$ref` points to, `None` without a `$ref` or when resolving fails (the failure becomes a message).
    pub fn visit_example(&self, e: &Example) -> JResult<Option<Example>> {
        let Some(r) = e.get_ref().filter(|r| !r.trim().is_empty()) else { return Ok(None) };
        self.resolve_ref(&r, OpenAPI31Traverser::deserialize_fragment::<Example>, OpenAPI31Traverser::traverse_example)
    }

    /// `visitSchema(schema, inheritedIds)`: the schema its `$ref` points to, resolved against the enclosing `$id`s in `inherited_ids`; `None` without a `$ref`.
    pub fn visit_schema(&self, s: &Schema, inherited_ids: &mut Vec<String>) -> JResult<Option<Schema>> {
        let Some(r) = s.get_ref().filter(|r| !r.trim().is_empty()) else { return Ok(None) };
        self.resolve_schema_ref(&r, inherited_ids)
    }

    /// `visitResponse(response)`: the response its `$ref` points to, `None` without a `$ref` or when resolving fails (the failure becomes a message).
    pub fn visit_response(&self, x: &ApiResponse) -> JResult<Option<ApiResponse>> {
        let Some(r) = x.get_ref().filter(|r| !r.trim().is_empty()) else { return Ok(None) };
        self.resolve_ref(&r, OpenAPI31Traverser::deserialize_fragment::<ApiResponse>, OpenAPI31Traverser::traverse_response)
    }

    /// `visitRequestBody(requestBody)`: the request body its `$ref` points to, `None` without a `$ref` or when resolving fails (the failure becomes a message).
    pub fn visit_request_body(&self, x: &RequestBody) -> JResult<Option<RequestBody>> {
        let Some(r) = x.get_ref().filter(|r| !r.trim().is_empty()) else { return Ok(None) };
        self.resolve_ref(&r, OpenAPI31Traverser::deserialize_fragment::<RequestBody>, OpenAPI31Traverser::traverse_request_body)
    }

    /// `visitLink(link)`: the link its `$ref` points to, `None` without a `$ref` or when resolving fails (the failure becomes a message).
    pub fn visit_link(&self, x: &Link) -> JResult<Option<Link>> {
        let Some(r) = x.get_ref().filter(|r| !r.trim().is_empty()) else { return Ok(None) };
        self.resolve_ref(&r, OpenAPI31Traverser::deserialize_fragment::<Link>, OpenAPI31Traverser::traverse_link)
    }

    /// `visitSecurityScheme(securityScheme)`: the scheme its `$ref` points to, `None` without a `$ref` or when resolving fails (the failure becomes a message).
    pub fn visit_security_scheme(&self, x: &SecurityScheme) -> JResult<Option<SecurityScheme>> {
        let Some(r) = x.get_ref().filter(|r| !r.trim().is_empty()) else { return Ok(None) };
        self.resolve_ref(&r, OpenAPI31Traverser::deserialize_fragment::<SecurityScheme>, OpenAPI31Traverser::traverse_security_scheme)
    }

    /// `visitHeader(header)`: the header its `$ref` points to, `None` without a `$ref` or when resolving fails (the failure becomes a message).
    pub fn visit_header(&self, x: &Header) -> JResult<Option<Header>> {
        let Some(r) = x.get_ref().filter(|r| !r.trim().is_empty()) else { return Ok(None) };
        self.resolve_ref(&r, OpenAPI31Traverser::deserialize_fragment::<Header>, OpenAPI31Traverser::traverse_header)
    }

    /// `resolveSchemaRef(visiting, ref, inheritedIds)`
    pub fn resolve_schema_ref(&self, r: &str, inherited_ids: &mut Vec<String>) -> JResult<Option<Schema>> {
        let attempt = |inherited_ids: &mut Vec<String>| -> JResult<Option<Schema>> {
            let mut base_uri = self.reference.uri();
            for id in inherited_ids.iter() {
                let url_without_hash = to_base_uri(id)?;
                let b = resolve(Some(&url_without_hash), base_uri.as_deref())?;
                base_uri = Some(to_base_uri(&b)?);
            }
            let mut base_uri = to_base_uri(&resolve(Some(r), base_uri.as_deref())?)?;
            let mut is_anchor = false;
            let existing = self.reference.reference_set().borrow().get(&base_uri).cloned();
            let reference_object = match existing {
                Some(x) => x,
                None => {
                    let node = match self.parse(&base_uri, self.reference.auths().as_deref()) {
                        Ok(n) => n,
                        Err(_) => {
                            base_uri = self.to_base_uri(r)?;
                            self.parse(&base_uri, self.reference.auths().as_deref())?
                        }
                    };
                    self.to_schema_reference(&base_uri, node)
                }
            };
            let fragment = get_fragment(r)?;
            let evaluated = match json_pointer_evaluate(fragment.as_deref(), reference_object.json_node().as_ref(), r) {
                Ok(n) => n,
                Err(_) => {
                    let found = match reference_object.json_node() {
                        Some(root) => find_anchor(&root, fragment.as_deref()),
                        None => return Err(JavaException::npe("Cannot invoke \"com.fasterxml.jackson.databind.JsonNode.isObject()\" because \"root\" is null")),
                    };
                    match found {
                        Some(n) => {
                            is_anchor = true;
                            Some(n)
                        }
                        None => return Err(runtime(format!("Could not find {} in contents of {r}", fragment.as_deref().unwrap_or("null")))),
                    }
                }
            };
            let resolved = self.traverser.deserialize_fragment::<Schema>(&evaluated, r, fragment.as_deref(), &reference_object.messages())?;
            if is_anchor {
                match &resolved {
                    Some(s) => s.set_anchor(None::<String>),
                    None => return Err(JavaException::npe("Cannot invoke \"io.swagger.v3.oas.models.media.Schema.$anchor(String)\" because \"resolved\" is null")),
                }
            }
            let visitor = self.child(reference_object)?;
            self.traverser.traverse_schema(resolved, &visitor, inherited_ids)
        };
        match attempt(inherited_ids) {
            Ok(v) => Ok(v),
            Err(e) => {
                self.reference.messages().add(e.message.clone());
                Ok(None)
            }
        }
    }

    /// `readHttp(uri, auths, checker)`
    pub fn read_http(&self, uri: &str, auths: Option<&[AuthorizationValue]>) -> JResult<String> {
        for r in HTTP_READERS.with(|h| h.borrow().clone()) {
            if let Some(res) = r(uri, auths) {
                return res;
            }
        }
        let ctx = self.ctx()?;
        if ctx.parse_options.safely_resolve_url {
            let checker = self.permitted_urls_checker.as_ref();
            if let Some(c) = checker {
                c.verify(uri)?;
            }
            return crate::parser::remote_url::url_to_string(uri, auths.unwrap_or(&[]), checker);
        }
        crate::parser::remote_url::url_to_string(uri, auths.unwrap_or(&[]), None)
    }

    /// `Visitor.readURI(absoluteUri, auths, checker)`
    pub fn read_uri(&self, absolute_uri: &str, auths: Option<&[AuthorizationValue]>) -> JResult<Option<String>> {
        let resolved = crate::java::net::JavaUri::parse(absolute_uri)?;
        if let Some(scheme) = resolved.get_scheme().filter(|s| !s.trim().is_empty()) {
            let s = scheme.to_lowercase();
            if s.starts_with("http") {
                return self.read_http(absolute_uri, auths).map(Some);
            } else if s.starts_with("file") {
                return read_file(resolved.get_decoded_path().as_deref()).map(Some);
            } else if s.starts_with("classpath") {
                return read_classpath(resolved.get_decoded_path().as_deref()).map(Some);
            }
        }
        let mut content = read_file(Some(absolute_uri)).ok();
        if content.as_deref().is_none_or(|c| c.trim().is_empty()) {
            content = Some(read_classpath(Some(absolute_uri))?);
        }
        Ok(content)
    }

    /// `deserializeIntoTree(content)`
    pub fn deserialize_into_tree(&self, content: Option<&str>) -> JResult<JsonNode> {
        let Some(content) = content else {
            return Err(JavaException::npe("Cannot invoke \"String.trim()\" because \"content\" is null"));
        };
        if content.trim().starts_with('{') {
            return jackson::json::read_tree(content, Default::default()).map_err(JavaException::from);
        }
        let mut opts = snakeyaml::LoaderOptions::default();
        if let Ok(cp) = std::env::var("maxYamlCodePoints")
            && !cp.is_empty() && cp.chars().all(|c| c.is_ascii_digit())
            && let Ok(v) = cp.parse::<usize>()
        {
            opts.code_point_limit = v;
        }
        let yaml = snakeyaml::Yaml::new(opts, snakeyaml::Resolver::default());
        let o = yaml.load(content).map_err(|e| JavaException::new(e.class, e.message()))?;
        jackson::yamlobj::yaml_convert_to_node(&o).map_err(JavaException::from)
    }

    /// `parse(absoluteUri, auths)`
    pub fn parse(&self, absolute_uri: &str, auths: Option<&[AuthorizationValue]>) -> JResult<JsonNode> {
        let ctx = self.ctx()?.clone();
        let cached = ctx.ids_cache.borrow().get(absolute_uri).cloned();
        if let Some(c) = cached {
            return self.deserialize_into_tree(Some(&c));
        }
        let ids = IdsTraverser::new(ctx.clone());
        ids.traverse(ctx.open_api.clone())?;
        let cached = ctx.ids_cache.borrow().get(absolute_uri).cloned();
        if let Some(c) = cached {
            return self.deserialize_into_tree(Some(&c));
        }
        let content = self.read_uri(absolute_uri, auths)?;
        self.deserialize_into_tree(content.as_deref())
    }
}

/// `Visitor.readFile(path)`
fn read_file(path: Option<&str>) -> JResult<String> {
    let Some(path) = path else {
        return Err(JavaException::npe("Cannot invoke \"String.length()\" because \"name\" is null"));
    };
    if std::path::Path::new(path).is_dir() {
        return Err(JavaException::new("java.io.FileNotFoundException", format!("{path} (Is a directory)")));
    }
    match std::fs::read(path) {
        Ok(b) => Ok(String::from_utf8_lossy(&b).into_owned()),
        Err(e) => {
            let reason = match e.kind() {
                std::io::ErrorKind::NotFound => "No such file or directory".to_string(),
                std::io::ErrorKind::PermissionDenied => "Permission denied".to_string(),
                _ => e.to_string(),
            };
            Err(JavaException::new("java.io.FileNotFoundException", format!("{path} ({reason})")))
        }
    }
}

/// `Visitor.readClasspath(path)`
fn read_classpath(path: Option<&str>) -> JResult<String> {
    let Some(path) = path else {
        return Err(JavaException::npe("Cannot invoke \"String.replace(char, char)\" because \"path\" is null"));
    };
    crate::parser::ref_utils::load_file_from_classpath(path)
}

/// `ReferenceVisitor.findAnchor(root, anchor)`
pub fn find_anchor(root: &JsonNode, anchor: Option<&str>) -> Option<JsonNode> {
    match root {
        JsonNode::Object(o) => {
            if let Some(a) = root.get("$anchor") && is_value_node(a) {
                match anchor {
                    Some(an) if an == a.as_text() => return Some(root.clone()),
                    // anchor.equals(...) on a null fragment
                    None => return None,
                    _ => {}
                }
            }
            for (_, v) in o.iter() {
                if let Some(n) = find_anchor(v, anchor) {
                    return Some(n);
                }
            }
            None
        }
        JsonNode::Array(a) => a.iter().find_map(|e| find_anchor(e, anchor)),
        _ => None,
    }
}

fn is_value_node(n: &JsonNode) -> bool {
    !matches!(n, JsonNode::Object(_) | JsonNode::Array(_) | JsonNode::Missing)
}

/// `OpenAPIDereferencer31.dereference(context, chain)` as `OpenAPIV3Parser.resolve` invokes it.
pub fn dereference_31(result: &mut SwaggerParseResult, auth: Option<&[AuthorizationValue]>, location: Option<&str>, options: &ParseOptions) -> JResult<()> {
    let context = Rc::new(DereferencerContext::new(result, auth, location, options));
    let Some(open_api) = context.open_api.clone() else { return Ok(()) };
    let ref_set: ReferenceSet = Rc::new(RefCell::new(IndexMap::new()));
    let msgs = Messages::default();
    if context.get_root_uri().is_none_or(|r| r.trim().is_empty()) {
        *context.root_uri.borrow_mut() = Some("local".into());
        *context.current_uri.borrow_mut() = Some("local".into());
    }
    if context.get_root_uri().as_deref() == Some("local") {
        let local = Reference::new();
        {
            let mut d = local.0.borrow_mut();
            d.reference_set = ref_set.clone();
            d.uri = context.get_current_uri();
            d.messages = msgs.clone();
            d.json_node = Some(value_to_tree(&open_api, Mapper::Json31).map_err(JavaException::from)?);
            d.auths = context.auths.clone();
        }
        ref_set.borrow_mut().insert("local".into(), local);
    }
    let reference = Reference::new();
    {
        let mut d = reference.0.borrow_mut();
        d.reference_set = ref_set.clone();
        d.uri = context.get_current_uri();
        d.messages = msgs.clone();
        d.auths = context.auths.clone();
    }
    let traverser = Rc::new(OpenAPI31Traverser::new(context.clone()));
    let visitor = ReferenceVisitor::new(reference.clone(), traverser.clone(), Some(context.clone()))?;
    let resolved = match traverser.traverse_open_api(Some(open_api), &visitor) {
        Ok(r) => r,
        // throw new RuntimeException(e)
        Err(e) => return Err(JavaException::runtime_wrapping(e)),
    };
    let Some(resolved) = resolved else { return Ok(()) };
    result.openapi = Some(resolved);
    let msgs: Vec<String> = reference.messages().items().into_iter().map(|m| m.unwrap_or_else(|| "null".into())).collect();
    match &mut result.messages {
        Some(m) => m.extend(msgs),
        None => {
            return Err(JavaException::npe(
                "Cannot invoke \"java.util.List.addAll(java.util.Collection)\" because the return value of \"io.swagger.v3.parser.core.models.SwaggerParseResult.getMessages()\" is null",
            ));
        }
    }
    Ok(())
}
