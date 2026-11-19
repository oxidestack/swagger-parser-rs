//! Hand-written model behaviour: constructors with defaults, `set$ref` prefixing, typed example setters,
//! `addExtension` rules and the `add…Item` helpers.

use super::*;
use crate::java::{IntoNullable, JList, JMap};
use jackson::JavaObject;

macro_rules! extension_methods {
    ($($name:ident),* $(,)?) => {
        $(impl $name {
            /// `addExtension(name, value)`: only names starting with `x-`
            pub fn add_extension(&self, name: &str, value: JavaObject) {
                if extension_allowed(name) {
                    put_extension(&mut self.write().extensions, name, value);
                }
            }

            /// `addExtension31(name, value)`: additionally drops `x-oas-` / `x-oai-` names
            pub fn add_extension31(&self, name: &str, value: JavaObject) {
                if extension31_allowed(name) {
                    self.add_extension(name, value);
                }
            }

            /// `setExtensions(map)`: replaces the `x-` extensions map; `None` clears it.
            pub fn set_extensions(&self, v: impl IntoNullable<JMap<JavaObject>>) {
                self.write().extensions = v.into_nullable();
            }

            /// `extensions(map)`: `set_extensions`, returning this object for chaining.
            pub fn extensions(&self, v: impl IntoNullable<JMap<JavaObject>>) -> Self {
                self.set_extensions(v);
                self.clone()
            }
        })*
    };
}

extension_methods!(
    OpenAPI, Components, ExternalDocumentation, Operation, PathItem, Example, Header, Contact, Info, License, Link, LinkParameter, Encoding,
    EncodingProperty, MediaType, XML, Parameter, RequestBody, ApiResponse, OAuthFlow, OAuthFlows, SecurityScheme, Server, ServerVariable, Tag,
);

impl Discriminator {
    /// `addExtension`: `x-` names, excluding `x-oas-` / `x-oai-`
    pub fn add_extension(&self, name: &str, value: JavaObject) {
        if extension_allowed(name) && extension31_allowed(name) {
            put_extension(&mut self.write().extensions, name, value);
        }
    }

    /// `setExtensions(map)`: replaces the `x-` extensions map; `None` clears it.
    pub fn set_extensions(&self, v: impl IntoNullable<JMap<JavaObject>>) {
        self.write().extensions = v.into_nullable();
    }

    /// `mapping(name, value)`
    pub fn mapping_entry(&self, name: &str, value: &str) -> Discriminator {
        let m = { self.write().mapping.get_or_insert_with(JMap::new).clone() };
        m.put(name, value.to_string());
        self.clone()
    }
}

macro_rules! map_extension_methods {
    ($($name:ident),* $(,)?) => {
        $(impl $name {
            /// `addExtension(name, value)`: only names starting with `x-`.
            pub fn add_extension(&self, name: &str, value: JavaObject) {
                if extension_allowed(name) {
                    put_extension(&mut self.0.write().extensions, name, value);
                }
            }

            /// `addExtension31(name, value)`: additionally drops `x-oas-` / `x-oai-` names.
            pub fn add_extension31(&self, name: &str, value: JavaObject) {
                if extension31_allowed(name) {
                    self.add_extension(name, value);
                }
            }

            /// `setExtensions(map)`: replaces the `x-` extensions map; `None` clears it.
            pub fn set_extensions(&self, v: impl IntoNullable<JMap<JavaObject>>) {
                self.0.write().extensions = v.into_nullable();
            }

            /// `extensions(map)`: `set_extensions`, returning this object for chaining.
            pub fn extensions(&self, v: impl IntoNullable<JMap<JavaObject>>) -> Self {
                self.set_extensions(v);
                self.clone()
            }
        })*
    };
}

map_extension_methods!(Paths, Callback, ApiResponses, Scopes, ServerVariables);

impl Default for OpenAPI {
    fn default() -> Self {
        OpenAPI::new()
    }
}

impl OpenAPI {
    /// `new OpenAPI()`: `openapi = "3.0.1"`, spec version 3.0
    pub fn new() -> OpenAPI {
        OpenAPI::from_data(OpenAPIData { openapi: Some("3.0.1".into()), spec_version: Some(SpecVersion::V30), ..Default::default() })
    }

    /// `new OpenAPI(SpecVersion)`
    pub fn new_with_spec_version(v: SpecVersion) -> OpenAPI {
        let o = OpenAPI::new();
        o.set_spec_version(v);
        o
    }

    /// `addServersItem(server)`: appends to `servers`, creating the list.
    pub fn add_servers_item(&self, s: Server) -> OpenAPI {
        self.write().servers.get_or_insert_with(JList::new).clone().add(s);
        self.clone()
    }

    /// `addSecurityItem(requirement)`: appends to `security`, creating the list.
    pub fn add_security_item(&self, s: SecurityRequirement) -> OpenAPI {
        self.write().security.get_or_insert_with(JList::new).clone().add(s);
        self.clone()
    }

    /// `addTagsItem(tag)`: appends to `tags`, creating the list.
    pub fn add_tags_item(&self, t: Tag) -> OpenAPI {
        self.write().tags.get_or_insert_with(JList::new).clone().add(t);
        self.clone()
    }

    /// `path(name, pathItem)`
    pub fn path(&self, name: &str, item: PathItem) -> OpenAPI {
        let p = { self.write().paths.get_or_insert_with(Paths::new).clone() };
        p.put(name, item);
        self.clone()
    }

    /// `schema(name, schema)`
    pub fn schema(&self, name: &str, s: Schema) -> OpenAPI {
        let c = { self.write().components.get_or_insert_with(Components::new).clone() };
        c.add_schemas(name, s);
        self.clone()
    }

    /// `schemaRequirement(name, securityScheme)`
    pub fn schema_requirement(&self, name: &str, s: SecurityScheme) -> OpenAPI {
        let c = { self.write().components.get_or_insert_with(Components::new).clone() };
        c.add_security_schemes(name, s);
        self.clone()
    }

    /// `addWebhooks(key, pathItem)`: puts into the 3.1 `webhooks` map, creating it.
    pub fn add_webhooks(&self, key: &str, p: PathItem) -> OpenAPI {
        self.write().webhooks.get_or_insert_with(JMap::new).clone().put(key, p);
        self.clone()
    }
}

macro_rules! components_adders {
    ($($method:ident => $field:ident : $ty:ident),* $(,)?) => {
        impl Components {
            $(#[doc = concat!("Puts `v` under `key` in the `", stringify!($field), "` map of `", stringify!($ty), "` values, creating the map; returns the components for chaining.")]
            pub fn $method(&self, key: &str, v: $ty) -> Components {
                let m = { self.write().$field.get_or_insert_with(JMap::new).clone() };
                m.put(key, v);
                self.clone()
            })*
        }
    };
}

components_adders!(
    add_schemas => schemas: Schema,
    add_responses => responses: ApiResponse,
    add_parameters => parameters: Parameter,
    add_examples => examples: Example,
    add_request_bodies => request_bodies: RequestBody,
    add_headers => headers: Header,
    add_security_schemes => security_schemes: SecurityScheme,
    add_links => links: Link,
    add_callbacks => callbacks: Callback,
    add_path_item => path_items: PathItem,
);

impl Operation {
    /// `addTagsItem(tag)`: appends to `tags`, creating the list.
    pub fn add_tags_item(&self, t: &str) -> Operation {
        self.write().tags.get_or_insert_with(JList::new).clone().add(t.to_string());
        self.clone()
    }

    /// `addParametersItem(parameter)`: appends to `parameters`, creating the list.
    pub fn add_parameters_item(&self, p: Parameter) -> Operation {
        self.write().parameters.get_or_insert_with(JList::new).clone().add(p);
        self.clone()
    }

    /// `addCallback(key, callback)`: puts into `callbacks`, creating the map.
    pub fn add_callback(&self, key: &str, c: Callback) -> Operation {
        self.write().callbacks.get_or_insert_with(JMap::new).clone().put(key, c);
        self.clone()
    }

    /// `addSecurityItem(requirement)`: appends to `security`, creating the list.
    pub fn add_security_item(&self, s: SecurityRequirement) -> Operation {
        self.write().security.get_or_insert_with(JList::new).clone().add(s);
        self.clone()
    }

    /// `addServersItem(server)`: appends to `servers`, creating the list.
    pub fn add_servers_item(&self, s: Server) -> Operation {
        self.write().servers.get_or_insert_with(JList::new).clone().add(s);
        self.clone()
    }
}

impl PathItem {
    /// `readOperations()` — note the Java order: get, put, head, post, delete, patch, options, trace.
    pub fn read_operations(&self) -> Vec<Operation> {
        let d = self.read();
        [&d.get, &d.put, &d.head, &d.post, &d.delete, &d.patch, &d.options, &d.trace].into_iter().flatten().cloned().collect()
    }

    /// `readOperationsMap()` — get, put, post, delete, patch, head, options, trace.
    pub fn read_operations_map(&self) -> Vec<(HttpMethod, Operation)> {
        let d = self.read();
        let mut v = Vec::new();
        for (m, o) in [
            (HttpMethod::Get, &d.get),
            (HttpMethod::Put, &d.put),
            (HttpMethod::Post, &d.post),
            (HttpMethod::Delete, &d.delete),
            (HttpMethod::Patch, &d.patch),
            (HttpMethod::Head, &d.head),
            (HttpMethod::Options, &d.options),
            (HttpMethod::Trace, &d.trace),
        ] {
            if let Some(o) = o {
                v.push((m, o.clone()));
            }
        }
        v
    }

    /// `operation(HttpMethod, Operation)`
    pub fn operation(&self, method: HttpMethod, op: impl IntoNullable<Operation>) {
        let op = op.into_nullable();
        let mut d = self.write();
        match method {
            HttpMethod::Patch => d.patch = op,
            HttpMethod::Post => d.post = op,
            HttpMethod::Put => d.put = op,
            HttpMethod::Get => d.get = op,
            HttpMethod::Options => d.options = op,
            HttpMethod::Trace => d.trace = op,
            HttpMethod::Head => d.head = op,
            HttpMethod::Delete => d.delete = op,
        }
    }

    /// `addServersItem(server)`: appends to `servers`, creating the list.
    pub fn add_servers_item(&self, s: Server) -> PathItem {
        self.write().servers.get_or_insert_with(JList::new).clone().add(s);
        self.clone()
    }

    /// `addParametersItem(parameter)`: appends to `parameters`, creating the list.
    pub fn add_parameters_item(&self, p: Parameter) -> PathItem {
        self.write().parameters.get_or_insert_with(JList::new).clone().add(p);
        self.clone()
    }
}

impl Example {
    /// `getValue()`: the example value, `None` when unset or explicitly null.
    pub fn get_value(&self) -> Option<JavaObject> {
        self.read().value.clone()
    }

    /// `setValue`: always marks the value as set (so an explicit null is serialized)
    pub fn set_value(&self, v: impl IntoNullable<JavaObject>) {
        let mut d = self.write();
        d.value = v.into_nullable().filter(|o| !o.is_null());
        d.value_set_flag = true;
    }

    /// `value(value)`: [`Self::set_value`], returning this example for chaining.
    pub fn value(&self, v: impl IntoNullable<JavaObject>) -> Example {
        self.set_value(v);
        self.clone()
    }

    /// `get$ref()`: the reference as stored, `None` when this example is not a reference.
    pub fn get_ref(&self) -> Option<String> {
        self.read().ref_.clone()
    }

    /// `set$ref`: a bare name becomes `#/components/examples/<name>`.
    pub fn set_ref(&self, r: impl IntoNullable<String>) {
        self.write().ref_ = component_ref(r.into_nullable(), "#/components/examples/");
    }

    /// `$ref(ref)`: [`Self::set_ref`], returning this example for chaining.
    pub fn ref_(&self, r: impl IntoNullable<String>) -> Example {
        self.set_ref(r);
        self.clone()
    }
}

macro_rules! component_ref_methods {
    ($($name:ident => $prefix:expr),* $(,)?) => {
        $(impl $name {
            /// `get$ref()`: the reference as stored, `None` when this object is not a reference.
            pub fn get_ref(&self) -> Option<String> {
                self.read().ref_.clone()
            }

            /// `set$ref`: a bare name is prefixed with the component path
            pub fn set_ref(&self, r: impl IntoNullable<String>) {
                self.write().ref_ = component_ref(r.into_nullable(), $prefix);
            }

            /// `$ref(ref)`: `set_ref`, returning this object for chaining.
            pub fn ref_(&self, r: impl IntoNullable<String>) -> Self {
                self.set_ref(r);
                self.clone()
            }
        })*
    };
}

component_ref_methods!(
    Header => "#/components/headers/",
    Link => "#/components/links/",
    RequestBody => "#/components/requestBodies/",
    ApiResponse => "#/components/responses/",
    SecurityScheme => "#/components/securitySchemes/",
);

impl Header {
    /// `addExample(key, example)`: puts into `examples`, creating the map.
    pub fn add_example(&self, key: &str, e: Example) -> Header {
        self.write().examples.get_or_insert_with(JMap::new).clone().put(key, e);
        self.clone()
    }
}

impl Link {
    /// `parameters(name, value)` / `addParameter`
    pub fn add_parameter(&self, name: &str, value: &str) -> Link {
        self.write().parameters.get_or_insert_with(JMap::new).clone().put(name, value.to_string());
        self.clone()
    }

    /// `addHeaderObject(name, header)`: puts into the link's `headers` map, creating it.
    pub fn add_header_object(&self, name: &str, h: Header) -> Link {
        self.write().headers.get_or_insert_with(JMap::new).clone().put(name, h);
        self.clone()
    }
}

impl MediaType {
    /// `getExample()`: the example after the schema's cast, `None` when unset or explicitly null.
    pub fn get_example(&self) -> Option<JavaObject> {
        self.read().example.clone()
    }

    /// `setExample`: cast through the schema when there is one
    pub fn set_example(&self, v: impl IntoNullable<JavaObject>) {
        let v = v.into_nullable().filter(|o| !o.is_null());
        let schema = self.get_schema();
        let mut d = self.write();
        match schema {
            None => {
                d.example = v;
                d.example_set_flag = true;
            }
            Some(s) => {
                let cast = v.as_ref().and_then(|o| s.cast(o));
                let flag = v.is_none() || cast.is_some();
                d.example = cast;
                if flag {
                    d.example_set_flag = true;
                }
            }
        }
    }

    /// `example(value)`: [`Self::set_example`], returning this media type for chaining.
    pub fn example(&self, v: impl IntoNullable<JavaObject>) -> MediaType {
        self.set_example(v);
        self.clone()
    }

    /// `addExamples(key, example)`: puts into `examples`, creating the map.
    pub fn add_examples(&self, key: &str, e: Example) -> MediaType {
        self.write().examples.get_or_insert_with(JMap::new).clone().put(key, e);
        self.clone()
    }

    /// `addEncoding(key, encoding)`: puts into `encoding`, creating the map.
    pub fn add_encoding(&self, key: &str, e: Encoding) -> MediaType {
        self.write().encoding.get_or_insert_with(JMap::new).clone().put(key, e);
        self.clone()
    }
}

impl Encoding {
    /// `addHeader(name, header)`: puts into `headers`, creating the map.
    pub fn add_header(&self, name: &str, h: Header) -> Encoding {
        self.write().headers.get_or_insert_with(JMap::new).clone().put(name, h);
        self.clone()
    }
}

impl Default for Parameter {
    fn default() -> Self {
        Parameter::new()
    }
}

impl Parameter {
    /// `new Parameter()`
    pub fn new() -> Parameter {
        Parameter::of(ParameterKind::Parameter)
    }

    /// `new QueryParameter()` etc.
    pub fn of(kind: ParameterKind) -> Parameter {
        let (in_, required) = match kind {
            ParameterKind::Parameter => (None, None),
            ParameterKind::Query => (Some("query"), None),
            ParameterKind::Path => (Some("path"), Some(true)),
            ParameterKind::Header => (Some("header"), None),
            ParameterKind::Cookie => (Some("cookie"), None),
        };
        Parameter::from_data(ParameterData { kind, in_: in_.map(str::to_string), required, ..Default::default() })
    }

    /// Which Java class this parameter is (`Parameter`, `QueryParameter`, `PathParameter`, ...).
    pub fn kind(&self) -> ParameterKind {
        self.read().kind
    }

    /// `getIn()`: where the parameter goes (`query`, `path`, `header`, `cookie`), `None` when unset.
    pub fn get_in(&self) -> Option<String> {
        self.read().in_.clone()
    }

    /// `setIn`: `"path"` also forces `required = true` (the subclasses override with a plain setter)
    pub fn set_in(&self, v: impl IntoNullable<String>) {
        let v = v.into_nullable();
        let mut d = self.write();
        if d.kind == ParameterKind::Parameter && v.as_deref() == Some("path") {
            d.required = Some(true);
        }
        d.in_ = v;
    }

    /// `in(value)`: [`Self::set_in`], returning this parameter for chaining.
    pub fn in_(&self, v: impl IntoNullable<String>) -> Parameter {
        self.set_in(v);
        self.clone()
    }

    /// `getRequired()`: `None` when unset; a `PathParameter` starts with `true`.
    pub fn get_required(&self) -> Option<bool> {
        self.read().required
    }

    /// `setRequired(value)`.
    pub fn set_required(&self, v: impl IntoNullable<bool>) {
        self.write().required = v.into_nullable();
    }

    /// `required(value)`: [`Self::set_required`], returning this parameter for chaining.
    pub fn required(&self, v: impl IntoNullable<bool>) -> Parameter {
        self.set_required(v);
        self.clone()
    }

    /// `get$ref()`: the reference as stored, `None` when this parameter is not a reference.
    pub fn get_ref(&self) -> Option<String> {
        self.read().ref_.clone()
    }

    /// `set$ref`: a bare name becomes `#/components/parameters/<name>`
    pub fn set_ref(&self, r: impl IntoNullable<String>) {
        let r = r.into_nullable().map(|r| {
            if !r.starts_with("#/components/parameters/") && !r.contains('.') && !r.contains('/') {
                format!("#/components/parameters/{r}")
            } else {
                r
            }
        });
        self.write().ref_ = r;
    }

    /// `$ref(ref)`: [`Self::set_ref`], returning this parameter for chaining.
    pub fn ref_(&self, r: impl IntoNullable<String>) -> Parameter {
        self.set_ref(r);
        self.clone()
    }

    /// `addExample(key, example)`: puts into `examples`, creating the map.
    pub fn add_example(&self, key: &str, e: Example) -> Parameter {
        self.write().examples.get_or_insert_with(JMap::new).clone().put(key, e);
        self.clone()
    }
}

impl ApiResponse {
    /// `addHeaderObject(name, header)`: puts into `headers`, creating the map.
    pub fn add_header_object(&self, name: &str, h: Header) -> ApiResponse {
        self.write().headers.get_or_insert_with(JMap::new).clone().put(name, h);
        self.clone()
    }

    /// `addLink(name, link)`: puts into `links`, creating the map.
    pub fn add_link(&self, name: &str, l: Link) -> ApiResponse {
        self.write().links.get_or_insert_with(JMap::new).clone().put(name, l);
        self.clone()
    }
}

impl ServerVariable {
    /// `addEnumItem(value)`: appends to `enum`, creating the list.
    pub fn add_enum_item(&self, s: &str) -> ServerVariable {
        self.write().enum_.get_or_insert_with(JList::new).clone().add(s.to_string());
        self.clone()
    }
}

impl Paths {
    /// `addPathItem(name, pathItem)`: puts the path item under `name`.
    pub fn add_path_item(&self, name: &str, item: PathItem) -> Paths {
        self.put(name, item);
        self.clone()
    }
}

impl Callback {
    /// `set$ref`: a bare name becomes `#/components/callbacks/<name>`
    pub fn set_ref(&self, r: impl IntoNullable<String>) {
        self.0.write().ref_ = component_ref(r.into_nullable(), "#/components/callbacks/");
    }

    /// `$ref(ref)`: [`Self::set_ref`], returning this callback for chaining.
    pub fn ref_(&self, r: impl IntoNullable<String>) -> Callback {
        self.set_ref(r);
        self.clone()
    }

    /// `addPathItem(expression, pathItem)`: puts the path item under the callback expression `name`.
    pub fn add_path_item(&self, name: &str, item: PathItem) -> Callback {
        self.put(name, item);
        self.clone()
    }
}

impl Content {
    /// `addMediaType(name, mediaType)`: puts the media type under `name`.
    pub fn add_media_type(&self, name: &str, m: MediaType) -> Content {
        self.put(name, m);
        self.clone()
    }
}

impl ApiResponses {
    /// `addApiResponse(name, response)`: puts the response under the status code or `default` key `name`.
    pub fn add_api_response(&self, name: &str, r: ApiResponse) -> ApiResponses {
        self.put(name, r);
        self.clone()
    }

    /// `getDefault()`
    pub fn get_default(&self) -> Option<ApiResponse> {
        self.get("default")
    }

    /// `setDefault(ApiResponse)`: `null` leaves the map unchanged
    pub fn set_default(&self, r: impl IntoNullable<ApiResponse>) {
        if let Some(r) = r.into_nullable() {
            self.put("default", r);
        }
    }
}

impl Scopes {
    /// `addString(name, item)`: puts the scope `name` with its description `item`.
    pub fn add_string(&self, name: &str, item: &str) -> Scopes {
        self.put(name, item.to_string());
        self.clone()
    }
}

impl SecurityRequirement {
    /// `addList(name, item)`
    pub fn add_list(&self, name: &str, item: &str) -> SecurityRequirement {
        self.put(name, JList::from_vec(vec![item.to_string()]));
        self.clone()
    }

    /// `addList(name, items)`
    pub fn add_list_items(&self, name: &str, items: JList<String>) -> SecurityRequirement {
        self.put(name, items);
        self.clone()
    }

    /// `addList(name)` (empty list)
    pub fn add_list_empty(&self, name: &str) -> SecurityRequirement {
        self.put(name, JList::new());
        self.clone()
    }
}

impl ServerVariables {
    /// `addServerVariable(name, variable)`: puts the variable under `name`.
    pub fn add_server_variable(&self, name: &str, v: ServerVariable) -> ServerVariables {
        self.put(name, v);
        self.clone()
    }
}
