//! `OpenAPI31Traverser` and `IdsTraverser` (one implementation; the ids pass runs without a visitor).

use super::{get_ref_name, is_anchor_ref, is_local_ref, is_local_ref_to_components, resolve, to_base_uri, DereferencerContext, Messages, ReferenceVisitor};
use crate::java::{JList, JMap, JResult, JavaException};
use crate::models::deserialize::FromTree;
use crate::models::*;
use crate::parser::deserializer::{OpenAPIDeserializer, ParseResult};
use crate::parser::resolver_cache::{convert_value, Component, ComponentKind};
use jackson::JsonNode;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// A key of the traversers' `visiting` set / `visitedMap` (Java `Object`).
#[derive(Clone)]
pub enum Key {
    OpenApi(OpenAPI),
    Components(Components),
    Paths(Paths),
    Operation(Operation),
    Responses(ApiResponses),
    Response(ApiResponse),
    RequestBody(RequestBody),
    PathItem(PathItem),
    Parameter(Parameter),
    Example(Example),
    Callback(Callback),
    MediaType(MediaType),
    Encoding(Encoding),
    Header(Header),
    SecurityScheme(SecurityScheme),
    Link(Link),
    Schema(Schema),
}

macro_rules! keys {
    ($($v:ident => $t:ident),* $(,)?) => {
        impl Key {
            fn java_hash(&self) -> i32 {
                match self { $(Key::$v(x) => x.java_hash_code()),* }
            }
            fn java_eq(&self, o: &Key) -> bool {
                match (self, o) {
                    $((Key::$v(a), Key::$v(b)) => a == b,)*
                    _ => false,
                }
            }
        }
        $(impl Keyed for $t {
            fn key(&self) -> Key { Key::$v(self.clone()) }
            fn from_key(k: Key) -> Option<Self> { match k { Key::$v(x) => Some(x), _ => None } }
        })*
    };
}

pub trait Keyed: Clone {
    fn key(&self) -> Key;
    fn from_key(k: Key) -> Option<Self>;
}

keys!(
    OpenApi => OpenAPI, Components => Components, Paths => Paths, Operation => Operation, Responses => ApiResponses,
    Response => ApiResponse, RequestBody => RequestBody, PathItem => PathItem, Parameter => Parameter, Example => Example,
    Callback => Callback, MediaType => MediaType, Encoding => Encoding, Header => Header, SecurityScheme => SecurityScheme,
    Link => Link, Schema => Schema,
);

/// Java `HashMap<Object, Object>` / `HashSet<Object>` semantics: an entry is found by the hash stored when it
/// was inserted and by `equals()`.
#[derive(Default)]
pub struct HashIndex {
    buckets: HashMap<i32, Vec<(Key, Option<Key>)>>,
}

impl HashIndex {
    fn find(&self, k: &Key) -> Option<(i32, usize)> {
        let h = k.java_hash();
        let b = self.buckets.get(&h)?;
        b.iter().position(|(e, _)| e.java_eq(k)).map(|i| (h, i))
    }

    pub fn contains(&self, k: &Key) -> bool {
        self.find(k).is_some()
    }

    /// `get(k)` for a present key (the value may be `null`)
    pub fn get(&self, k: &Key) -> Option<Option<Key>> {
        let (h, i) = self.find(k)?;
        Some(self.buckets[&h][i].1.clone())
    }

    pub fn put(&mut self, k: Key, v: Option<Key>) {
        match self.find(&k) {
            Some((h, i)) => self.buckets.get_mut(&h).unwrap()[i].1 = v,
            None => {
                let h = k.java_hash();
                self.buckets.entry(h).or_default().push((k, v));
            }
        }
    }

    pub fn add(&mut self, k: Key) {
        if self.find(&k).is_none() {
            let h = k.java_hash();
            self.buckets.entry(h).or_default().push((k, None));
        }
    }

    pub fn remove(&mut self, k: &Key) {
        if let Some((h, i)) = self.find(k) {
            self.buckets.get_mut(&h).unwrap().remove(i);
        }
    }
}

/// What `deepcopy` / `deserializeFragment` can produce.
pub trait Fragment: Keyed + Serialize + Sized {
    const KIND: ComponentKind;
    fn from_component(c: Component) -> Option<Self>;
    fn from_tree31(n: &JsonNode) -> Result<Option<Self>, jackson::JacksonError>;
}

macro_rules! fragment {
    ($($t:ident => $kind:ident),*) => {
        $(impl Fragment for $t {
            const KIND: ComponentKind = ComponentKind::$kind;
            fn from_component(c: Component) -> Option<Self> {
                match c { Component::$kind(x) => Some(x), _ => None }
            }
            fn from_tree31(n: &JsonNode) -> Result<Option<Self>, jackson::JacksonError> {
                if matches!(n, JsonNode::Null | JsonNode::Missing) {
                    return Ok(None);
                }
                <$t as FromTree>::from_tree_opt(n, Mapper::Json31)
            }
        })*
    };
}

fragment!(Schema => Schema, ApiResponse => ApiResponse, Parameter => Parameter, RequestBody => RequestBody, Example => Example, Link => Link,
    Callback => Callback, Header => Header, SecurityScheme => SecurityScheme, PathItem => PathItem);

/// `deepcopy(entity, clazz)`: `Json31.mapper().readValue(Json31.mapper().writeValueAsString(entity), clazz)`
fn deepcopy<T: Fragment>(x: &T) -> JResult<Option<T>> {
    let wrap = |e: jackson::JacksonError| JavaException::runtime_wrapping(JavaException::from(e));
    let s = json31_string(x).map_err(wrap)?;
    let tree = jackson::json::read_tree(&s, Default::default()).map_err(wrap)?;
    T::from_tree31(&tree).map_err(wrap)
}

/// The mode-specific parts of the two traversers.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Reference,
    Ids,
}

pub struct Traverser {
    mode: Mode,
    context: Rc<DereferencerContext>,
    visiting: RefCell<HashIndex>,
    visited_map: RefCell<HashIndex>,
    deserializer: RefCell<OpenAPIDeserializer>,
}

/// `OpenAPI31Traverser`
pub type OpenAPI31Traverser = Traverser;

/// `IdsTraverser`
pub struct IdsTraverser(Traverser);

impl IdsTraverser {
    /// `new IdsTraverser(context)`: a traverser that records schema `$id`s in the context's ids cache without resolving references.
    pub fn new(context: Rc<DereferencerContext>) -> IdsTraverser {
        IdsTraverser(Traverser::with_mode(context, Mode::Ids))
    }

    /// `traverse(openAPI, null)`
    pub fn traverse(&self, open_api: Option<OpenAPI>) -> JResult<Option<OpenAPI>> {
        self.0.traverse_open_api_impl(open_api, None)
    }
}

type V<'a> = Option<&'a ReferenceVisitor>;

macro_rules! enter {
    ($self:ident, $x:ident) => {
        let Some($x) = $x else { return Ok(None) };
        let key = $x.key();
        if $self.visiting.borrow().contains(&key) {
            return Ok(Some($x));
        }
        if let Some(v) = $self.visited_map.borrow().get(&key) {
            return Ok(v.and_then(Keyed::from_key));
        }
        $self.visiting.borrow_mut().add(key.clone());
    };
}

impl Traverser {
    pub fn new(context: Rc<DereferencerContext>) -> Traverser {
        Traverser::with_mode(context, Mode::Reference)
    }

    fn with_mode(context: Rc<DereferencerContext>, mode: Mode) -> Traverser {
        Traverser {
            mode,
            context,
            visiting: RefCell::new(HashIndex::default()),
            visited_map: RefCell::new(HashIndex::default()),
            deserializer: RefCell::new(OpenAPIDeserializer::new()),
        }
    }

    pub fn get_context(&self) -> &Rc<DereferencerContext> {
        &self.context
    }

    fn finish<T: Keyed>(&self, original: &T, value: Option<Key>) {
        self.visited_map.borrow_mut().put(original.key(), value);
        self.visiting.borrow_mut().remove(&original.key());
    }

    fn finish_copy<T: Fragment>(&self, original: &T, copy_of: &T) -> JResult<()> {
        let c = deepcopy(copy_of)?;
        self.finish(original, c.map(|c| c.key()));
        Ok(())
    }

    /// `deserializeFragment(node, expectedType, uri, fragment, validationMessages)`
    pub fn deserialize_fragment<T: Fragment>(&self, node: &Option<JsonNode>, uri: &str, fragment: Option<&str>, messages: &Messages) -> JResult<Option<T>> {
        let sanitized = fragment.unwrap_or("").replace('/', ".");
        let mut pr = ParseResult::new().openapi31(true);
        let obj = match node {
            None => None,
            Some(n @ JsonNode::Object(_)) => Some(n),
            Some(n) => {
                let from = format!("com.fasterxml.jackson.databind.node.{}", n.node_class_name());
                return Err(JavaException::class_cast(&from, "com.fasterxml.jackson.databind.node.ObjectNode"));
            }
        };
        let result: Option<Component> = {
            let mut d = self.deserializer.borrow_mut();
            match T::KIND {
                ComponentKind::Schema => d.get_schema(obj, &sanitized, Some(&mut pr))?.map(Component::Schema),
                ComponentKind::RequestBody => d.get_request_body(obj, &sanitized, &mut pr)?.map(Component::RequestBody),
                ComponentKind::ApiResponse => d.get_response(obj, &sanitized, &mut pr)?.map(Component::ApiResponse),
                ComponentKind::Callback => d.get_callback(obj, &sanitized, &mut pr)?.map(Component::Callback),
                ComponentKind::Example => d.get_example(obj, &sanitized, &mut pr)?.map(Component::Example),
                ComponentKind::Header => d.get_header(obj, &sanitized, &mut pr)?.map(Component::Header),
                ComponentKind::Link => d.get_link(obj, &sanitized, &mut pr)?.map(Component::Link),
                ComponentKind::Parameter => d.get_parameter(obj, &sanitized, &mut pr)?.map(Component::Parameter),
                ComponentKind::SecurityScheme => d.get_security_scheme(obj, &sanitized, &mut pr)?.map(Component::SecurityScheme),
                ComponentKind::PathItem => match obj {
                    Some(o) => d.get_path_item(o, &sanitized, &mut pr)?.map(Component::PathItem),
                    None => return Err(JavaException::npe("Cannot invoke \"com.fasterxml.jackson.databind.node.ObjectNode.get(String)\" because \"obj\" is null")),
                },
            }
        };
        for m in pr.get_messages() {
            messages.add(Some(format!("{m} ({uri})")));
        }
        if let Some(r) = result {
            return Ok(T::from_component(r));
        }
        // DeserializationUtils.deserialize(node, uri, expectedType)
        let Some(n) = node else { return Ok(None) };
        let c = convert_value(n, T::KIND, Mapper::Json30).map_err(|e| {
            JavaException::new(
                "java.lang.RuntimeException",
                format!("An exception was thrown while trying to deserialize the contents of {uri} into type class {}", T::KIND.class_name()),
            )
            .with_cause(JavaException::from(e))
        })?;
        Ok(c.and_then(T::from_component))
    }

    /// `traverse(openAPI, visitor)`
    pub fn traverse_open_api(&self, open_api: Option<OpenAPI>, v: &ReferenceVisitor) -> JResult<Option<OpenAPI>> {
        self.traverse_open_api_impl(open_api, Some(v))
    }

    fn traverse_open_api_impl(&self, open_api: Option<OpenAPI>, v: V) -> JResult<Option<OpenAPI>> {
        enter!(self, open_api);
        let resolved = open_api.clone();
        if let Some(p) = self.traverse_paths(resolved.get_paths(), v)? {
            resolved.set_paths(p);
        }
        if let Some(c) = self.traverse_components(resolved.get_components(), v)? {
            resolved.set_components(c);
        }
        self.traverse_map(resolved.get_webhooks(), v, Self::traverse_path_item_v)?;
        self.finish(&open_api, Some(resolved.key()));
        Ok(Some(resolved))
    }

    fn traverse_components(&self, components: Option<Components>, v: V) -> JResult<Option<Components>> {
        enter!(self, components);
        let r = components.clone();
        self.traverse_map(r.get_path_items(), v, Self::traverse_path_item_v)?;
        self.traverse_map(r.get_parameters(), v, Self::traverse_parameter_v)?;
        self.traverse_map(r.get_callbacks(), v, Self::traverse_callback)?;
        self.traverse_map(r.get_request_bodies(), v, Self::traverse_request_body_v)?;
        self.traverse_map(r.get_security_schemes(), v, Self::traverse_security_scheme_v)?;
        self.traverse_schema_map(r.get_schemas(), v, &mut Vec::new())?;
        self.traverse_map(r.get_headers(), v, Self::traverse_header_v)?;
        self.traverse_map(r.get_links(), v, Self::traverse_link_v)?;
        self.traverse_map(r.get_responses(), v, Self::traverse_response_v)?;
        self.traverse_map(r.get_examples(), v, Self::traverse_example_v)?;
        self.finish(&components, Some(r.key()));
        Ok(Some(r))
    }

    fn traverse_paths(&self, paths: Option<Paths>, v: V) -> JResult<Option<Paths>> {
        enter!(self, paths);
        let r = paths.clone();
        for (k, e) in r.entries() {
            if let Some(res) = self.traverse_path_item_v(Some(e), v)? {
                r.put(k, res);
            }
        }
        self.finish(&paths, Some(r.key()));
        Ok(Some(r))
    }

    fn traverse_operation(&self, operation: Option<Operation>, v: V) -> JResult<Option<Operation>> {
        enter!(self, operation);
        let r = operation.clone();
        if let Some(params) = r.get_parameters() {
            let mut i = 0;
            while i < params.len() {
                if let Some(p) = self.traverse_parameter_v(params.get(i), v)? {
                    params.set(i, p);
                }
                i += 1;
            }
        }
        if let Some(b) = r.get_request_body() && let Some(rb) = self.traverse_request_body_v(Some(b), v)? {
            r.set_request_body(rb);
        }
        if let Some(resp) = r.get_responses() && let Some(rr) = self.traverse_responses(Some(resp), v)? {
            r.set_responses(rr);
        }
        self.traverse_map(r.get_callbacks(), v, Self::traverse_callback)?;
        self.finish(&operation, Some(r.key()));
        Ok(Some(r))
    }

    fn traverse_responses(&self, responses: Option<ApiResponses>, v: V) -> JResult<Option<ApiResponses>> {
        enter!(self, responses);
        let r = responses.clone();
        for (k, e) in r.entries() {
            if let Some(res) = self.traverse_response_v(Some(e), v)? {
                r.put(k, res);
            }
        }
        self.finish(&responses, Some(r.key()));
        Ok(Some(r))
    }

    fn rv<'a>(&self, v: V<'a>) -> Option<&'a ReferenceVisitor> {
        if self.mode == Mode::Reference { v } else { None }
    }

    pub fn traverse_response(&self, x: Option<ApiResponse>, v: &ReferenceVisitor) -> JResult<Option<ApiResponse>> {
        self.traverse_response_v(x, Some(v))
    }

    fn traverse_response_v(&self, response: Option<ApiResponse>, v: V) -> JResult<Option<ApiResponse>> {
        enter!(self, response);
        let visited = match self.rv(v) {
            Some(vis) => vis.visit_response(&response)?,
            None => None,
        };
        let resolved_not_null = visited.is_some();
        let resolved = visited.unwrap_or_else(|| response.clone());
        self.traverse_content(resolved.get_content(), v)?;
        self.traverse_map(resolved.get_headers(), v, Self::traverse_header_v)?;
        self.traverse_map(resolved.get_links(), v, Self::traverse_link_v)?;
        if let Some(vis) = self.rv(v) {
            if self.should_handle_root_local_refs(resolved_not_null, response.get_ref().as_deref(), vis)? {
                let comps = self.ensure_components()?;
                if comps.get_responses().is_none() {
                    comps.set_responses(JMap::new());
                }
                self.finish_copy(&response, &response)?;
                return Ok(handle_root_local_refs(response.get_ref().as_deref(), resolved, &comps.get_responses().unwrap()));
            }
            if let Some(d) = response.get_description() {
                resolved.set_description(d);
            }
        }
        self.finish_copy(&response, &resolved)?;
        Ok(Some(resolved))
    }

    pub fn traverse_request_body(&self, x: Option<RequestBody>, v: &ReferenceVisitor) -> JResult<Option<RequestBody>> {
        self.traverse_request_body_v(x, Some(v))
    }

    fn traverse_request_body_v(&self, body: Option<RequestBody>, v: V) -> JResult<Option<RequestBody>> {
        enter!(self, body);
        let visited = match self.rv(v) {
            Some(vis) => vis.visit_request_body(&body)?,
            None => None,
        };
        let resolved_not_null = visited.is_some();
        let resolved = visited.unwrap_or_else(|| body.clone());
        self.traverse_content(resolved.get_content(), v)?;
        if let Some(vis) = self.rv(v) {
            if self.should_handle_root_local_refs(resolved_not_null, body.get_ref().as_deref(), vis)? {
                let comps = self.ensure_components()?;
                if comps.get_request_bodies().is_none() {
                    comps.set_request_bodies(JMap::new());
                }
                self.finish_copy(&body, &body)?;
                return Ok(handle_root_local_refs(body.get_ref().as_deref(), resolved, &comps.get_request_bodies().unwrap()));
            }
            if let Some(d) = body.get_description() {
                resolved.set_description(d);
            }
        }
        self.finish_copy(&body, &resolved)?;
        Ok(Some(resolved))
    }

    pub fn traverse_path_item(&self, x: Option<PathItem>, v: &ReferenceVisitor) -> JResult<Option<PathItem>> {
        self.traverse_path_item_v(x, Some(v))
    }

    fn traverse_path_item_v(&self, path_item: Option<PathItem>, v: V) -> JResult<Option<PathItem>> {
        enter!(self, path_item);
        let visited = match self.rv(v) {
            Some(vis) => vis.visit_path_item(&path_item)?,
            None => None,
        };
        let resolved_not_null = visited.is_some();
        let resolved = visited.unwrap_or_else(|| path_item.clone());
        if let Some(o) = self.traverse_operation(resolved.get_get(), v)? {
            resolved.set_get(o);
        }
        if let Some(o) = self.traverse_operation(resolved.get_put(), v)? {
            resolved.set_put(o);
        }
        if let Some(o) = self.traverse_operation(resolved.get_delete(), v)? {
            resolved.set_delete(o);
        }
        if let Some(o) = self.traverse_operation(resolved.get_patch(), v)? {
            resolved.set_patch(o);
        }
        if let Some(o) = self.traverse_operation(resolved.get_options(), v)? {
            resolved.set_options(o);
        }
        if let Some(o) = self.traverse_operation(resolved.get_head(), v)? {
            resolved.set_head(o);
        }
        if let Some(o) = self.traverse_operation(resolved.get_post(), v)? {
            resolved.set_post(o);
        }
        if let Some(o) = self.traverse_operation(resolved.get_trace(), v)? {
            resolved.set_trace(o);
        }
        if let Some(params) = resolved.get_parameters() {
            let mut i = 0;
            while i < params.len() {
                if let Some(p) = self.traverse_parameter_v(params.get(i), v)? {
                    params.set(i, p);
                }
                i += 1;
            }
        }
        if let Some(vis) = self.rv(v) {
            if self.should_handle_root_local_refs(resolved_not_null, path_item.get_ref().as_deref(), vis)? {
                let comps = self.ensure_components()?;
                if comps.get_path_items().is_none() {
                    comps.set_path_items(JMap::new());
                }
                self.finish_copy(&path_item, &path_item)?;
                return Ok(handle_root_local_refs(path_item.get_ref().as_deref(), resolved, &comps.get_path_items().unwrap()));
            }
            if let Some(p) = path_item.get_parameters() {
                resolved.set_parameters(p);
            }
            if let Some(d) = path_item.get_description() {
                resolved.set_description(d);
            }
            if let Some(s) = path_item.get_summary() {
                resolved.set_summary(s);
            }
        }
        self.finish_copy(&path_item, &resolved)?;
        Ok(Some(resolved))
    }

    pub fn traverse_parameter(&self, x: Option<Parameter>, v: &ReferenceVisitor) -> JResult<Option<Parameter>> {
        self.traverse_parameter_v(x, Some(v))
    }

    fn traverse_parameter_v(&self, parameter: Option<Parameter>, v: V) -> JResult<Option<Parameter>> {
        enter!(self, parameter);
        let visited = match self.rv(v) {
            Some(vis) => vis.visit_parameter(&parameter)?,
            None => None,
        };
        let resolved_not_null = visited.is_some();
        let resolved = visited.unwrap_or_else(|| parameter.clone());
        self.traverse_content(resolved.get_content(), v)?;
        if let Some(s) = resolved.get_schema() && let Some(t) = self.traverse_schema_v(Some(s), v, &mut Vec::new())? {
            resolved.set_schema(t);
        }
        self.traverse_map(resolved.get_examples(), v, Self::traverse_example_v)?;
        if let Some(vis) = self.rv(v) {
            if self.should_handle_root_local_refs(resolved_not_null, parameter.get_ref().as_deref(), vis)? {
                let comps = self.ensure_components()?;
                if comps.get_parameters().is_none() {
                    comps.set_parameters(JMap::new());
                }
                self.finish_copy(&parameter, &parameter)?;
                return Ok(handle_root_local_refs(parameter.get_ref().as_deref(), resolved, &comps.get_parameters().unwrap()));
            }
            if let Some(d) = parameter.get_description() {
                resolved.set_description(d);
            }
        }
        self.finish_copy(&parameter, &resolved)?;
        Ok(Some(resolved))
    }

    pub fn traverse_example(&self, x: Option<Example>, v: &ReferenceVisitor) -> JResult<Option<Example>> {
        self.traverse_example_v(x, Some(v))
    }

    fn traverse_example_v(&self, example: Option<Example>, v: V) -> JResult<Option<Example>> {
        enter!(self, example);
        let visited = match self.rv(v) {
            Some(vis) => vis.visit_example(&example)?,
            None => None,
        };
        let resolved_not_null = visited.is_some();
        let resolved = visited.unwrap_or_else(|| example.clone());
        if let Some(vis) = self.rv(v) {
            if self.should_handle_root_local_refs(resolved_not_null, example.get_ref().as_deref(), vis)? {
                let comps = self.ensure_components()?;
                if comps.get_examples().is_none() {
                    comps.set_examples(JMap::new());
                }
                self.finish_copy(&example, &example)?;
                return Ok(handle_root_local_refs(example.get_ref().as_deref(), resolved, &comps.get_examples().unwrap()));
            }
            if let Some(d) = example.get_description() {
                resolved.set_description(d);
            }
            if let Some(s) = example.get_summary() {
                resolved.set_summary(s);
            }
        }
        self.finish_copy(&example, &resolved)?;
        Ok(Some(resolved))
    }

    fn traverse_callback(&self, callback: Option<Callback>, v: V) -> JResult<Option<Callback>> {
        enter!(self, callback);
        for (k, e) in callback.entries() {
            if let Some(res) = self.traverse_path_item_v(Some(e), v)? {
                callback.put(k, res);
            }
        }
        self.finish(&callback, Some(callback.key()));
        Ok(Some(callback))
    }

    fn traverse_media_type(&self, media_type: Option<MediaType>, v: V) -> JResult<Option<MediaType>> {
        enter!(self, media_type);
        let resolved = media_type.clone();
        if let Some(s) = resolved.get_schema() {
            let t = self.traverse_schema_v(Some(s), v, &mut Vec::new())?;
            if self.mode == Mode::Reference && let Some(t) = t {
                resolved.set_schema(t);
            }
        }
        self.traverse_map(resolved.get_encoding(), v, Self::traverse_encoding)?;
        self.traverse_map(resolved.get_examples(), v, Self::traverse_example_v)?;
        self.finish(&media_type, Some(resolved.key()));
        Ok(Some(resolved))
    }

    fn traverse_encoding(&self, encoding: Option<Encoding>, v: V) -> JResult<Option<Encoding>> {
        enter!(self, encoding);
        let resolved = encoding.clone();
        self.traverse_map(resolved.get_headers(), v, Self::traverse_header_v)?;
        self.finish(&encoding, Some(resolved.key()));
        Ok(Some(resolved))
    }

    pub fn traverse_header(&self, x: Option<Header>, v: &ReferenceVisitor) -> JResult<Option<Header>> {
        self.traverse_header_v(x, Some(v))
    }

    fn traverse_header_v(&self, header: Option<Header>, v: V) -> JResult<Option<Header>> {
        enter!(self, header);
        let visited = match self.rv(v) {
            Some(vis) => vis.visit_header(&header)?,
            None => None,
        };
        let resolved_not_null = visited.is_some();
        let resolved = visited.unwrap_or_else(|| header.clone());
        self.traverse_content(resolved.get_content(), v)?;
        if let Some(s) = resolved.get_schema() {
            let t = self.traverse_schema_v(Some(s), v, &mut Vec::new())?;
            if self.mode == Mode::Reference && let Some(t) = t {
                resolved.set_schema(t);
            }
        }
        if let Some(vis) = self.rv(v) {
            if self.should_handle_root_local_refs(resolved_not_null, header.get_ref().as_deref(), vis)? {
                let comps = self.ensure_components()?;
                if comps.get_headers().is_none() {
                    comps.set_headers(JMap::new());
                }
                self.finish_copy(&header, &header)?;
                return Ok(handle_root_local_refs(header.get_ref().as_deref(), resolved, &comps.get_headers().unwrap()));
            }
            if let Some(d) = header.get_description() {
                resolved.set_description(d);
            }
        }
        self.finish_copy(&header, &resolved)?;
        Ok(Some(resolved))
    }

    pub fn traverse_security_scheme(&self, x: Option<SecurityScheme>, v: &ReferenceVisitor) -> JResult<Option<SecurityScheme>> {
        self.traverse_security_scheme_v(x, Some(v))
    }

    fn traverse_security_scheme_v(&self, ss: Option<SecurityScheme>, v: V) -> JResult<Option<SecurityScheme>> {
        enter!(self, ss);
        let visited = match self.rv(v) {
            Some(vis) => vis.visit_security_scheme(&ss)?,
            None => None,
        };
        let resolved_not_null = visited.is_some();
        let resolved = visited.unwrap_or_else(|| ss.clone());
        if let Some(vis) = self.rv(v) {
            if self.should_handle_root_local_refs(resolved_not_null, ss.get_ref().as_deref(), vis)? {
                let comps = self.ensure_components()?;
                if comps.get_security_schemes().is_none() {
                    comps.set_security_schemes(JMap::new());
                }
                self.finish_copy(&ss, &ss)?;
                return Ok(handle_root_local_refs(ss.get_ref().as_deref(), resolved, &comps.get_security_schemes().unwrap()));
            }
            if let Some(d) = ss.get_description() {
                resolved.set_description(d);
            }
        }
        self.finish_copy(&ss, &resolved)?;
        Ok(Some(resolved))
    }

    pub fn traverse_link(&self, x: Option<Link>, v: &ReferenceVisitor) -> JResult<Option<Link>> {
        self.traverse_link_v(x, Some(v))
    }

    fn traverse_link_v(&self, link: Option<Link>, v: V) -> JResult<Option<Link>> {
        enter!(self, link);
        let visited = match self.rv(v) {
            Some(vis) => vis.visit_link(&link)?,
            None => None,
        };
        let resolved_not_null = visited.is_some();
        let resolved = visited.unwrap_or_else(|| link.clone());
        self.traverse_map(resolved.get_headers(), v, Self::traverse_header_v)?;
        if let Some(vis) = self.rv(v) {
            if self.should_handle_root_local_refs(resolved_not_null, link.get_ref().as_deref(), vis)? {
                let comps = self.ensure_components()?;
                if comps.get_links().is_none() {
                    comps.set_links(JMap::new());
                }
                self.finish_copy(&link, &link)?;
                return Ok(handle_root_local_refs(link.get_ref().as_deref(), resolved, &comps.get_links().unwrap()));
            }
            if let Some(d) = link.get_description() {
                resolved.set_description(d);
            }
        }
        self.finish_copy(&link, &resolved)?;
        Ok(Some(resolved))
    }

    pub fn traverse_schema(&self, x: Option<Schema>, v: &ReferenceVisitor, inherited_ids: &mut Vec<String>) -> JResult<Option<Schema>> {
        self.traverse_schema_v(x, Some(v), inherited_ids)
    }

    fn record_id(&self, schema: &Schema, v: V, inherited_ids: &[String]) {
        let attempt = || -> JResult<()> {
            let mut resolved_uri = match (self.mode, v) {
                (Mode::Reference, Some(vis)) => vis.reference.uri(),
                _ => self.context.get_current_uri(),
            };
            for id in inherited_ids {
                let url_without_hash = to_base_uri(id)?;
                let r = resolve(Some(&url_without_hash), resolved_uri.as_deref())?;
                resolved_uri = Some(to_base_uri(&r)?);
            }
            let key = resolved_uri.clone().unwrap_or_else(|| "null".into());
            let pretty = json31_pretty(schema).unwrap_or_else(|| "null".into());
            self.context.ids_cache.borrow_mut().insert(key.clone(), pretty);
            if let (Mode::Reference, Some(vis)) = (self.mode, v) {
                let set = vis.reference.reference_set();
                if !set.borrow().contains_key(&key) {
                    set.borrow_mut().insert(key, vis.reference.clone());
                }
            }
            Ok(())
        };
        let _ = attempt();
    }

    fn traverse_schema_v(&self, schema: Option<Schema>, v: V, inherited_ids: &mut Vec<String>) -> JResult<Option<Schema>> {
        enter!(self, schema);
        let id = schema.get_id().filter(|s| !s.trim().is_empty());
        if let Some(id) = &id {
            inherited_ids.push(id.clone());
            self.record_id(&schema, v, inherited_ids);
        }
        let visited = match self.rv(v) {
            Some(vis) => vis.visit_schema(&schema, inherited_ids)?,
            None => None,
        };
        let resolved_not_null = visited.is_some();
        let resolved = visited.unwrap_or_else(|| schema.clone());

        self.traverse_schema_map(resolved.get_properties(), v, inherited_ids)?;
        if let Some(s) = resolved.get_additional_items()
            && let Some(t) = self.traverse_schema_v(Some(s), v, inherited_ids)?
        {
            resolved.set_additional_items(t);
        }
        if let Some(AdditionalProperties::Schema(s)) = resolved.get_additional_properties()
            && let Some(t) = self.traverse_schema_v(Some(s), v, inherited_ids)?
        {
            resolved.set_additional_properties(t);
        }
        self.traverse_schema_list(resolved.get_all_of(), v, inherited_ids)?;
        self.traverse_schema_list(resolved.get_any_of(), v, inherited_ids)?;
        if let Some(s) = resolved.get_contains() && let Some(t) = self.traverse_schema_v(Some(s), v, inherited_ids)? {
            resolved.set_contains(t);
        }
        if let Some(s) = resolved.get_content_schema()
            && let Some(t) = self.traverse_schema_v(Some(s), v, inherited_ids)?
        {
            resolved.set_content_schema(t);
        }
        self.traverse_schema_map(resolved.get_dependent_schemas(), v, inherited_ids)?;
        if let Some(s) = resolved.get_else() && let Some(t) = self.traverse_schema_v(Some(s), v, inherited_ids)? {
            resolved.set_else(t);
        }
        if let Some(s) = resolved.get_if() && let Some(t) = self.traverse_schema_v(Some(s), v, inherited_ids)? {
            resolved.set_if(t);
        }
        if let Some(s) = resolved.get_items() && let Some(t) = self.traverse_schema_v(Some(s), v, inherited_ids)? {
            resolved.set_items(t);
        }
        if let Some(s) = resolved.get_not() && let Some(t) = self.traverse_schema_v(Some(s), v, inherited_ids)? {
            resolved.set_not(t);
        }
        self.traverse_schema_list(resolved.get_one_of(), v, inherited_ids)?;
        self.traverse_schema_map(resolved.get_pattern_properties(), v, inherited_ids)?;
        self.traverse_schema_list(resolved.get_prefix_items(), v, inherited_ids)?;
        if let Some(s) = resolved.get_then() && let Some(t) = self.traverse_schema_v(Some(s), v, inherited_ids)? {
            resolved.set_then(t);
        }
        if let Some(s) = resolved.get_unevaluated_items()
            && let Some(t) = self.traverse_schema_v(Some(s), v, inherited_ids)?
        {
            resolved.set_unevaluated_items(t);
        }
        // `getAdditionalProperties() != null && getUnevaluatedProperties() instanceof Schema` (sic)
        if resolved.get_additional_properties().is_some()
            && let Some(s) = resolved.get_unevaluated_properties()
            && let Some(t) = self.traverse_schema_v(Some(s), v, inherited_ids)?
        {
            resolved.set_unevaluated_properties(t);
        }

        if let Some(vis) = self.rv(v) {
            if self.should_handle_root_local_refs(resolved_not_null, schema.get_ref().as_deref(), vis)? {
                let comps = self.ensure_components()?;
                if comps.get_schemas().is_none() {
                    comps.set_schemas(JMap::new());
                }
                self.finish_copy(&schema, &schema)?;
                return Ok(handle_root_local_refs(schema.get_ref().as_deref(), resolved, &comps.get_schemas().unwrap()));
            }
            if resolved_not_null && let Some(p) = schema.get_properties().filter(|p| !p.is_empty()) {
                self.traverse_schema_map(Some(p), v, inherited_ids)?;
            }
            merge_schemas(&schema, &resolved);
            self.finish_copy(&schema, &resolved)?;
            if let Some(id) = &id {
                remove_first(inherited_ids, id);
            }
        } else {
            if let Some(id) = &id {
                remove_first(inherited_ids, id);
            }
            self.finish_copy(&schema, &resolved)?;
        }
        Ok(Some(resolved))
    }

    fn traverse_schema_list(&self, l: Option<JList<Schema>>, v: V, inherited_ids: &mut Vec<String>) -> JResult<()> {
        if let Some(l) = l {
            let mut i = 0;
            while i < l.len() {
                if let Some(t) = self.traverse_schema_v(l.get(i), v, inherited_ids)? {
                    l.set(i, t);
                }
                i += 1;
            }
        }
        Ok(())
    }

    fn traverse_map<T: Clone>(&self, map: Option<JMap<T>>, v: V, f: fn(&Self, Option<T>, V) -> JResult<Option<T>>) -> JResult<()> {
        if let Some(map) = map {
            for (k, e) in map.entries() {
                if let Some(r) = f(self, Some(e), v)? {
                    map.put(k, r);
                }
            }
        }
        Ok(())
    }

    fn traverse_content(&self, content: Option<Content>, v: V) -> JResult<()> {
        if let Some(c) = content {
            for (k, e) in c.entries() {
                if let Some(r) = self.traverse_media_type(Some(e), v)? {
                    c.put(k, r);
                }
            }
        }
        Ok(())
    }

    fn traverse_schema_map(&self, map: Option<JMap<Schema>>, v: V, inherited_ids: &mut Vec<String>) -> JResult<()> {
        if let Some(map) = map {
            for (k, e) in map.entries() {
                if let Some(r) = self.traverse_schema_v(Some(e), v, inherited_ids)? {
                    map.put(k, r);
                }
            }
        }
        Ok(())
    }

    /// `shouldHandleRootLocalRefs(resolvedNotNull, ref, visitor)`
    fn should_handle_root_local_refs(&self, resolved_not_null: bool, r: Option<&str>, v: &ReferenceVisitor) -> JResult<bool> {
        if !(resolved_not_null && is_local_ref(r) && !self.context.parse_options.resolve_fully) {
            return Ok(false);
        }
        let Some(uri) = v.reference.uri() else {
            return Err(JavaException::npe("Cannot invoke \"String.equals(Object)\" because the return value of \"io.swagger.v3.parser.reference.Reference.getUri()\" is null"));
        };
        Ok(Some(uri) == self.context.get_root_uri() && (is_local_ref_to_components(r) || is_anchor_ref(r)))
    }

    fn ensure_components(&self) -> JResult<Components> {
        let Some(api) = &self.context.open_api else {
            return Err(JavaException::npe("Cannot invoke \"io.swagger.v3.oas.models.OpenAPI.getComponents()\" because \"openAPI\" is null"));
        };
        if api.get_components().is_none() {
            api.set_components(Components::new());
        }
        Ok(api.get_components().unwrap())
    }
}

fn remove_first(v: &mut Vec<String>, id: &str) {
    if let Some(i) = v.iter().position(|x| x == id) {
        v.remove(i);
    }
}

/// `handleRootLocalRefs(ref, entity, map)`: always `null`
fn handle_root_local_refs<T: Clone>(r: Option<&str>, entity: T, map: &JMap<T>) -> Option<T> {
    if !is_local_ref_to_components(r) && is_anchor_ref(r) {
        return None;
    }
    let name = get_ref_name(r.unwrap_or(""));
    map.put(name, entity);
    None
}

/// `mergeSchemas(source, target)`
fn merge_schemas(source: &Schema, target: &Schema) {
    if let Some(x) = source.get_description() {
        target.set_description(x);
    }
    if let Some(x) = source.get_one_of().filter(|l| !l.is_empty()) {
        target.set_one_of(x);
    }
    if let Some(x) = source.get_not() {
        target.set_not(x);
    }
    if let Some(x) = source.get_any_of().filter(|l| !l.is_empty()) {
        target.set_any_of(x);
    }
    if let Some(x) = source.get_type() {
        target.set_type(x);
    }
    if let Some(x) = source.get_types().filter(|s| !s.is_empty()) {
        target.set_types(x);
    }
    if let Some(x) = source.get_format() {
        target.set_format(x);
    }
    if let Some(x) = source.get_required() {
        target.required(x);
    }
    if let Some(x) = source.get_then() {
        target.set_then(x);
    }
    if let Some(x) = source.get_if() {
        target.set_if(x);
    }
    if let Some(x) = source.get_else() {
        target.set_else(x);
    }
    if let Some(x) = source.get_content_schema() {
        target.set_content_schema(x);
    }
    if let Some(x) = source.get_contains() {
        target.set_contains(x);
    }
    if let Some(x) = source.get_additional_properties() {
        target.set_additional_properties(x);
    }
    if let Some(x) = source.get_unevaluated_properties() {
        target.set_unevaluated_properties(x);
    }
    if let Some(x) = source.get_unevaluated_items() {
        target.set_unevaluated_items(x);
    }
    if let Some(x) = source.get_prefix_items().filter(|l| !l.is_empty()) {
        target.set_prefix_items(x);
    }
    if let Some(x) = source.get_properties().filter(|m| !m.is_empty()) {
        target.set_properties(x);
    }
    if let Some(x) = source.get_pattern_properties().filter(|m| !m.is_empty()) {
        target.set_pattern_properties(x);
    }
    if let Some(x) = source.get_pattern() {
        target.set_pattern(x);
    }
    if let Some(x) = source.get_dependent_schemas().filter(|m| !m.is_empty()) {
        target.set_dependent_schemas(x);
    }
    if let Some(x) = source.get_const() {
        target.const_(x);
    }
    if let Some(x) = source.get_additional_items() {
        target.set_additional_items(x);
    }
    if let Some(x) = source.get_enum().filter(|l| !l.is_empty()) {
        target.enum_(x);
    }
    if let Some(x) = source.get_read_only() {
        target.set_read_only(x);
    }
    if let Some(x) = source.get_write_only() {
        target.set_write_only(x);
    }
    if let Some(x) = source.get_max_length() {
        target.set_max_length(x);
    }
    if let Some(x) = source.get_anchor() {
        target.set_anchor(x);
    }
    if let Some(x) = source.get_comment() {
        target.set_comment(x);
    }
    if let Some(x) = source.get_id() {
        target.set_id(x);
    }
    if let Some(x) = source.get_schema() {
        target.set_schema(x);
    }
    if let Some(x) = source.get_content_encoding() {
        target.set_content_encoding(x);
    }
    if let Some(x) = source.get_content_media_type() {
        target.set_content_media_type(x);
    }
    if let Some(x) = source.get_default() {
        target.set_default(x);
    }
    if let Some(x) = source.get_dependent_required().filter(|m| !m.is_empty()) {
        target.set_dependent_required(x);
    }
    if let Some(x) = source.get_deprecated() {
        target.set_deprecated(x);
    }
    if let Some(x) = source.get_discriminator() {
        target.set_discriminator(x);
    }
    if let Some(x) = source.get_example() {
        target.set_example(x);
    }
    if let Some(x) = source.get_examples().filter(|l| !l.is_empty()) {
        target.set_examples(x);
    }
    if let Some(x) = source.get_exclusive_maximum() {
        target.set_exclusive_maximum(x);
    }
    if let Some(x) = source.get_exclusive_maximum_value() {
        target.set_exclusive_maximum_value(x);
    }
    if let Some(x) = source.get_exclusive_minimum() {
        target.set_exclusive_minimum(x);
    }
    if let Some(x) = source.get_exclusive_minimum_value() {
        target.set_exclusive_minimum_value(x);
    }
    if let Some(x) = source.get_extensions().filter(|m| !m.is_empty()) {
        target.set_extensions(x);
    }
    if let Some(x) = source.get_external_docs() {
        target.set_external_docs(x);
    }
    if let Some(x) = source.get_max_contains() {
        target.set_max_contains(x);
    }
    if let Some(x) = source.get_maximum() {
        target.set_maximum(x);
    }
    if let Some(x) = source.get_max_items() {
        target.set_max_items(x);
    }
    if let Some(x) = source.get_max_properties() {
        target.set_max_properties(x);
    }
    if let Some(x) = source.get_min_contains() {
        target.set_min_contains(x);
    }
    if let Some(x) = source.get_min_items() {
        target.set_min_items(x);
    }
    if let Some(x) = source.get_min_properties() {
        target.set_min_properties(x);
    }
    if let Some(x) = source.get_multiple_of() {
        target.set_multiple_of(x);
    }
    if let Some(x) = source.get_nullable() {
        target.set_nullable(x);
    }
    if let Some(x) = source.get_property_names() {
        target.set_property_names(x);
    }
    if let Some(x) = source.get_title() {
        target.set_title(x);
    }
    if let Some(x) = source.get_unique_items() {
        target.set_unique_items(x);
    }
}
