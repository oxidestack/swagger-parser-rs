//! `io.swagger.v3.parser.util.ResolverFully`: replaces internal references by their targets and, with
//! `resolveCombinators`, merges `allOf`/`oneOf`/`anyOf` into one schema.

use crate::core::ParseOptions;
use crate::java::{JList, JMap, JResult, JavaException};
use crate::models::*;
use crate::parser::deserializer::create_schema;
use crate::parser::ref_utils::{compute_definition_name, compute_ref_format, is_an_external_ref_format, java_split};
use jackson::object::java_hash_set;
use jackson::JavaObject;
use std::collections::HashMap;

/// Port of `io.swagger.v3.parser.util.ResolverFully`: replaces every local `$ref` with the object it points to, optionally merging `allOf` members.
pub struct ResolverFully {
    aggregate_combinators: bool,
    parse_options: Option<ParseOptions>,
    schemas: Option<JMap<Schema>>,
    resolved_models: HashMap<String, Schema>,
    examples: Option<JMap<Example>>,
    parameters: Option<JMap<Parameter>>,
    responses: Option<JMap<ApiResponse>>,
    request_bodies: Option<JMap<RequestBody>>,
    headers: Option<JMap<Header>>,
    links: Option<JMap<Link>>,
    /// `IdentityHashMap<Schema, Schema>` (keys kept alive alongside)
    resolved_schemas: HashMap<usize, (Schema, Schema)>,
    schemas_in_progress: HashMap<usize, Schema>,
    callbacks: Option<JMap<Callback>>,
}

fn get_or_default<V: Clone>(m: &Option<JMap<V>>, key: &str, default: &V) -> V {
    m.as_ref().and_then(|m| m.get(key)).unwrap_or_else(|| default.clone())
}

/// `ref.substring(ref.lastIndexOf("/") + 1)`
fn after_last_slash(r: &str) -> &str {
    match r.rfind('/') {
        Some(i) => &r[i + 1..],
        None => r,
    }
}

impl Default for ResolverFully {
    fn default() -> Self {
        ResolverFully::with_aggregate(true)
    }
}

impl ResolverFully {
    /// `new ResolverFully(aggregateCombinators)`
    pub fn with_aggregate(aggregate_combinators: bool) -> ResolverFully {
        ResolverFully::build(aggregate_combinators, Some(ParseOptions::default()))
    }

    /// `new ResolverFully(options)`
    pub fn new(options: &ParseOptions) -> ResolverFully {
        ResolverFully::build(options.resolve_combinators, Some(options.clone()))
    }

    /// `new ResolverFully((ParseOptions) null)`
    pub fn without_options() -> ResolverFully {
        ResolverFully::build(true, None)
    }

    fn build(aggregate_combinators: bool, parse_options: Option<ParseOptions>) -> ResolverFully {
        ResolverFully {
            aggregate_combinators,
            parse_options,
            schemas: None,
            resolved_models: HashMap::new(),
            examples: None,
            parameters: None,
            responses: None,
            request_bodies: None,
            headers: None,
            links: None,
            resolved_schemas: HashMap::new(),
            schemas_in_progress: HashMap::new(),
            callbacks: None,
        }
    }

    /// `resolveFully(openAPI)`
    pub fn resolve_fully(&mut self, open_api: Option<&OpenAPI>) -> JResult<()> {
        let Some(open_api) = open_api else {
            return Err(JavaException::npe("Cannot invoke \"io.swagger.v3.oas.models.OpenAPI.getComponents()\" because \"openAPI\" is null"));
        };
        if let Some(c) = open_api.get_components() {
            if let Some(m) = c.get_request_bodies() {
                self.request_bodies = Some(m);
            }
            if let Some(m) = c.get_schemas() {
                self.schemas = Some(m);
            }
            if let Some(m) = c.get_responses() {
                self.responses = Some(m);
            }
            if let Some(m) = c.get_examples() {
                self.examples = Some(m);
            }
            if let Some(m) = c.get_headers() {
                self.headers = Some(m);
            }
            if let Some(m) = c.get_parameters() {
                self.parameters = Some(m);
            }
            if let Some(m) = c.get_links() {
                self.links = Some(m);
            }
            if let Some(m) = c.get_callbacks() {
                self.callbacks = Some(m);
            }
        }
        if let Some(paths) = open_api.get_paths() {
            for name in paths.keys() {
                match paths.get(&name) {
                    Some(p) => self.resolve_path(&p)?,
                    None => return Err(JavaException::npe("Cannot invoke \"io.swagger.v3.oas.models.PathItem.readOperations()\" because \"pathItem\" is null")),
                }
            }
        }
        Ok(())
    }

    /// `resolvePath(pathItem)`
    pub fn resolve_path(&mut self, path_item: &PathItem) -> JResult<()> {
        for op in path_item.read_operations() {
            if let Some(params) = op.get_parameters() {
                for parameter in params.to_vec() {
                    let parameter = if parameter.get_ref().is_some() { self.resolve_parameter(&parameter)? } else { parameter };
                    if let Some(s) = parameter.get_schema() && let Some(resolved) = self.resolve_schema(Some(&s))? {
                        parameter.set_schema(resolved);
                    }
                    if let Some(content) = parameter.get_content() {
                        for key in content.keys() {
                            if let Some(mt) = content.get(&key)
                                && let Some(s) = mt.get_schema()
                                && let Some(resolved) = self.resolve_schema(Some(&s))?
                            {
                                mt.set_schema(resolved);
                            }
                        }
                    }
                }
            }

            if let Some(callbacks) = op.get_callbacks() {
                for name in callbacks.keys() {
                    let Some(callback) = callbacks.get(&name) else {
                        return Err(JavaException::npe("Cannot invoke \"io.swagger.v3.oas.models.callbacks.Callback.get$ref()\" because \"callback\" is null"));
                    };
                    let callback = if callback.get_ref().is_some() { self.resolve_callback(&callback)? } else { callback };
                    for cn in callback.keys() {
                        if let Some(p) = callback.get(&cn) {
                            self.resolve_path(&p)?;
                        }
                    }
                    op.get_callbacks().unwrap().put(name.clone(), callback);
                }
            }

            if let Some(ref_body) = op.get_request_body() {
                let body = if ref_body.get_ref().is_some() { self.resolve_request_body(&ref_body)? } else { ref_body };
                op.set_request_body(body.clone());
                if let Some(content) = body.get_content() {
                    for key in content.keys() {
                        if let Some(mt) = content.get(&key)
                            && let Some(s) = mt.get_schema()
                            && let Some(resolved) = self.resolve_schema(Some(&s))?
                        {
                            mt.set_schema(resolved);
                        }
                    }
                }
            }

            if let Some(responses) = op.get_responses() {
                let resolved_responses = ApiResponses::new();
                for code in responses.keys() {
                    let Some(response) = responses.get(&code) else {
                        return Err(JavaException::npe("Cannot invoke \"io.swagger.v3.oas.models.responses.ApiResponse.get$ref()\" because \"response\" is null"));
                    };
                    let response = if response.get_ref().is_some() { self.resolve_response(&response)? } else { response };
                    if let Some(content) = response.get_content() {
                        for media_type in content.keys() {
                            let mt = content.get(&media_type).unwrap();
                            if let Some(s) = mt.get_schema() {
                                let resolved = self.resolve_schema(Some(&s))?;
                                response.get_content().unwrap().get(&media_type).unwrap().set_schema(resolved);
                            }
                            if let Some(ex) = content.get(&media_type).unwrap().get_examples() {
                                let resolved = self.resolve_example(Some(&ex))?;
                                response.get_content().unwrap().get(&media_type).unwrap().set_examples(resolved);
                            }
                        }
                    }
                    self.resolve_headers(response.get_headers().as_ref())?;
                    if let Some(links) = response.get_links() {
                        for (k, value) in links.entries() {
                            let resolved = if value.get_ref().is_some() { self.resolve_link(&value)? } else { Some(value) };
                            match resolved {
                                Some(v) => links.replace_value(&k, v),
                                // entry.setValue(null)
                                None => {
                                    links.put_null_value(&k);
                                }
                            }
                        }
                    }
                    resolved_responses.add_api_response(&code, response);
                }
                op.set_responses(resolved_responses);
            }
        }
        Ok(())
    }

    fn resolve_headers(&mut self, headers: Option<&JMap<Header>>) -> JResult<()> {
        let Some(headers) = headers.filter(|h| !h.is_empty()) else { return Ok(()) };
        for (k, value) in headers.entries() {
            let resolved = if value.get_ref().is_some() { self.resolve_header(&value)? } else { value };
            if let Some(ex) = resolved.get_examples() {
                let r = self.resolve_example(Some(&ex))?;
                resolved.set_examples(r);
            }
            if let Some(s) = resolved.get_schema() && let Some(rs) = self.resolve_schema(Some(&s))? {
                resolved.set_schema(rs);
            }
            headers.replace_value(&k, resolved);
        }
        Ok(())
    }

    /// `resolveHeader(header)`
    pub fn resolve_header(&mut self, header: &Header) -> JResult<Header> {
        let r = header.get_ref().unwrap_or_default();
        if !is_an_external_ref_format(compute_ref_format(&r)) && self.headers.as_ref().is_some_and(|m| !m.is_empty()) {
            let key = compute_definition_name(&r)?;
            return Ok(get_or_default(&self.headers, &key, header));
        }
        Ok(header.clone())
    }

    /// `resolveLink(link)`
    pub fn resolve_link(&mut self, link: &Link) -> JResult<Option<Link>> {
        let r = link.get_ref().unwrap_or_default();
        if !is_an_external_ref_format(compute_ref_format(&r)) && self.links.as_ref().is_some_and(|m| !m.is_empty()) {
            let key = compute_definition_name(&r)?;
            let l = get_or_default(&self.links, &key, link);
            self.resolve_headers(l.get_headers().as_ref())?;
            return Ok(Some(l));
        }
        Ok(Some(link.clone()))
    }

    /// `resolveRequestBody(requestBody)`
    pub fn resolve_request_body(&mut self, body: &RequestBody) -> JResult<RequestBody> {
        let r = body.get_ref().unwrap_or_default();
        if !is_an_external_ref_format(compute_ref_format(&r)) && self.request_bodies.as_ref().is_some_and(|m| !m.is_empty()) {
            let key = compute_definition_name(&r)?;
            return Ok(get_or_default(&self.request_bodies, &key, body));
        }
        Ok(body.clone())
    }

    /// `resolveResponse(apiResponse)`
    pub fn resolve_response(&mut self, response: &ApiResponse) -> JResult<ApiResponse> {
        let r = response.get_ref().unwrap_or_default();
        if !is_an_external_ref_format(compute_ref_format(&r)) && self.responses.as_ref().is_some_and(|m| !m.is_empty()) {
            let key = compute_definition_name(&r)?;
            return Ok(get_or_default(&self.responses, &key, response));
        }
        Ok(response.clone())
    }

    /// `resolveCallback(callback)`
    pub fn resolve_callback(&mut self, callback: &Callback) -> JResult<Callback> {
        let r = callback.get_ref().unwrap_or_default();
        if !is_an_external_ref_format(compute_ref_format(&r)) && self.callbacks.as_ref().is_some_and(|m| !m.is_empty()) {
            let key = compute_definition_name(&r)?;
            return Ok(get_or_default(&self.callbacks, &key, callback));
        }
        Ok(callback.clone())
    }

    /// `resolveParameter(parameter)`
    pub fn resolve_parameter(&mut self, parameter: &Parameter) -> JResult<Parameter> {
        let r = parameter.get_ref().unwrap_or_default();
        if !is_an_external_ref_format(compute_ref_format(&r)) && self.parameters.as_ref().is_some_and(|m| !m.is_empty()) {
            let key = compute_definition_name(&r)?;
            return Ok(get_or_default(&self.parameters, &key, parameter));
        }
        Ok(parameter.clone())
    }

    /// `resolveSchema(schema)`
    pub fn resolve_schema(&mut self, schema: Option<&Schema>) -> JResult<Option<Schema>> {
        let Some(schema) = schema else { return Ok(None) };
        let id = schema.id();
        if let Some((_, cached)) = self.resolved_schemas.get(&id) {
            return Ok(Some(cached.clone()));
        }
        if self.schemas_in_progress.contains_key(&id) {
            return Ok(Some(schema.clone()));
        }
        self.schemas_in_progress.insert(id, schema.clone());
        let r = self.resolve_schema_impl(schema);
        self.schemas_in_progress.remove(&id);
        let resolved = r?;
        self.resolved_schemas.insert(id, (schema.clone(), resolved.clone()));
        Ok(Some(resolved))
    }

    fn explicit_object_schema(&self) -> JResult<bool> {
        match &self.parse_options {
            Some(o) => Ok(o.explicit_object_schema),
            None => Err(JavaException::npe("Cannot invoke \"io.swagger.v3.parser.core.models.ParseOptions.isExplicitObjectSchema()\" because \"this.parseOptions\" is null")),
        }
    }

    fn resolve_schema_impl(&mut self, schema: &Schema) -> JResult<Schema> {
        if let Some(full_ref) = schema.get_ref() {
            let mut r = full_ref.clone();
            let resolved: Option<Schema> = if r.contains("/properties/") {
                let split = java_split(&r, "/");
                let ref_schema = split.get(3).cloned().ok_or_else(|| {
                    JavaException::new("java.lang.ArrayIndexOutOfBoundsException", format!("Index 3 out of bounds for length {}", split.len()))
                })?;
                let parent = self.schemas.as_ref().and_then(|s| s.get(&ref_schema));
                r = after_last_slash(&r).to_string();
                let Some(parent) = parent else {
                    return Err(JavaException::npe("Cannot invoke \"io.swagger.v3.oas.models.media.Schema.getProperties()\" because \"parentSchema\" is null"));
                };
                let Some(props) = parent.get_properties() else {
                    return Err(JavaException::npe("Cannot invoke \"java.util.Map.get(Object)\" because the return value of \"io.swagger.v3.oas.models.media.Schema.getProperties()\" is null"));
                };
                props.get(&r)
            } else {
                r = after_last_slash(&r).to_string();
                self.schemas.as_ref().and_then(|s| s.get(&r))
            };
            return match resolved {
                Some(resolved) => {
                    if let Some(m) = self.resolved_models.get(&r) {
                        return Ok(m.clone());
                    }
                    self.resolved_models.insert(r.clone(), schema.clone());
                    let model = self.resolve_schema(Some(&resolved))?.unwrap();
                    self.resolved_models.insert(r.clone(), model.clone());
                    self.schemas.as_ref().unwrap().put(r, model.clone());
                    Ok(model)
                }
                None => Ok(schema.clone()),
            };
        }

        if schema.is_array_schema() {
            let items = self.resolve_schema(schema.get_items().as_ref())?;
            schema.set_items(items);
            return Ok(schema.clone());
        }

        if schema.is_map_schema() && let Some(AdditionalProperties::Schema(ap)) = schema.get_additional_properties() {
            let r = self.resolve_schema(Some(&ap))?;
            schema.set_additional_properties(r.map(AdditionalProperties::Schema));
        }

        if schema.is_object_schema() {
            if let Some(props) = schema.get_properties() {
                let updated: JMap<Schema> = JMap::new();
                for name in props.keys() {
                    let inner = props.get(&name).unwrap();
                    if !schema.ptr_eq(&inner) && let Some(r) = self.resolve_schema(Some(&inner))? {
                        updated.put(name, r);
                    }
                }
                schema.set_properties(updated);
            }
            return Ok(schema.clone());
        }

        let mut result = schema.clone();

        if schema.is_composed_schema() {
            let has_all_of = schema.get_all_of().is_some();
            let has_any_of = schema.get_any_of().is_some();
            let has_one_of = schema.get_one_of().is_some();
            let adjacent = (has_all_of && has_any_of) || (has_all_of && has_one_of) || (has_any_of && has_one_of);

            if self.aggregate_combinators && (has_all_of || adjacent) {
                let combined = if schema.get_spec_version() == Some(SpecVersion::V30) {
                    let t = get_schema_type(schema);
                    create_schema(t.as_deref().unwrap_or("\u{0}null"), schema.get_format().as_deref())
                } else {
                    let c = Schema::json_schema();
                    c.set_format(schema.get_format());
                    c.set_types(schema.get_types());
                    c
                };
                let mut examples: Vec<JavaObject> = Vec::new();
                let mut default_values: Vec<JavaObject> = Vec::new();
                if has_all_of {
                    self.aggregate_schema_combinators(schema, &combined, &schema.get_all_of().unwrap(), &mut examples, &mut default_values)?;
                }
                if has_one_of {
                    self.aggregate_schema_combinators(schema, &combined, &schema.get_one_of().unwrap(), &mut examples, &mut default_values)?;
                }
                if has_any_of {
                    self.aggregate_schema_combinators(schema, &combined, &schema.get_any_of().unwrap(), &mut examples, &mut default_values)?;
                }
                let default_set = java_hash_set(default_values);
                if default_set.len() == 1 && !default_set[0].is_null() {
                    combined.set_default(default_set[0].clone());
                }
                if let Some(e) = schema.get_example() {
                    combined.set_example(e);
                } else if !examples.is_empty() {
                    combined.set_example(JavaObject::Set(java_hash_set(examples).into()));
                }
                if let Some(x) = schema.get_xml() {
                    combined.set_xml(x);
                }
                if let Some(d) = schema.get_description() {
                    combined.set_description(d);
                }
                if let Some(ext) = schema.get_extensions() {
                    for (k, v) in ext.entries() {
                        combined.add_extension(&k, v);
                    }
                }
                if let Some(props) = schema.get_properties() {
                    if combined.get_properties().is_none() {
                        combined.set_properties(JMap::new_hash());
                    }
                    combined.get_properties().unwrap().put_all(&props);
                }
                if let Some(req) = schema.get_required() {
                    if combined.get_required().is_none() {
                        combined.required(JList::new());
                    }
                    combined.get_required().unwrap().add_all(&req);
                }
                result = combined;
            } else {
                if has_all_of {
                    let l = self.resolve_list(&schema.get_all_of().unwrap())?;
                    schema.set_all_of(l);
                }
                if has_one_of {
                    let l = self.resolve_list(&schema.get_one_of().unwrap())?;
                    schema.set_one_of(l);
                }
                if has_any_of {
                    let l = self.resolve_list(&schema.get_any_of().unwrap())?;
                    schema.set_any_of(l);
                }
            }
        }

        if let Some(properties) = result.get_properties() {
            let model = result.clone();
            let updated: Vec<(String, Option<Schema>)> = {
                let mut v = Vec::new();
                for name in properties.keys() {
                    let p = model.get_properties().unwrap().get(&name);
                    let r = self.resolve_schema(p.as_ref())?;
                    // LinkedHashMap.put keeps the first position of a key
                    if let Some(pos) = v.iter().position(|(k, _): &(String, Option<Schema>)| *k == name) {
                        v[pos].1 = r;
                    } else {
                        v.push((name, r));
                    }
                }
                v
            };
            for (key, property) in updated {
                let Some(property) = property else {
                    return Err(JavaException::npe("Cannot invoke \"io.swagger.v3.oas.models.media.Schema.getProperties()\" because \"property\" is null"));
                };
                let same_map = match (property.get_properties(), model.get_properties()) {
                    (None, None) => true,
                    (Some(a), Some(b)) => a.ptr_eq(&b),
                    _ => false,
                };
                if !same_map {
                    if !has_schema_type(&property) && self.explicit_object_schema()? {
                        if property.get_spec_version() == Some(SpecVersion::V30) {
                            property.set_type("object");
                        } else {
                            property.add_type("object");
                        }
                    }
                    model.add_properties(&key, property);
                } else {
                    model.add_properties(&key, Schema::object_schema());
                }
            }
            return Ok(model);
        }
        Ok(result)
    }

    fn resolve_list(&mut self, l: &JList<Schema>) -> JResult<JList<Schema>> {
        let out = JList::new();
        for s in l.to_vec() {
            match self.resolve_schema(Some(&s))? {
                Some(r) => out.add(r),
                None => return Err(JavaException::npe("null schema")),
            }
        }
        Ok(out)
    }

    /// `resolveExample(examples)`
    pub fn resolve_example(&mut self, examples: Option<&JMap<Example>>) -> JResult<Option<JMap<Example>>> {
        let Some(examples) = examples else { return Ok(None) };
        for name in examples.keys() {
            let Some(e) = examples.get(&name) else {
                return Err(JavaException::npe("Cannot invoke \"io.swagger.v3.oas.models.examples.Example.get$ref()\" because the return value of \"java.util.Map.get(Object)\" is null"));
            };
            if let Some(r) = e.get_ref() {
                let key = after_last_slash(&r).to_string();
                let Some(component_examples) = &self.examples else {
                    return Err(JavaException::npe("Cannot invoke \"java.util.Map.get(Object)\" because \"this.examples\" is null"));
                };
                match component_examples.get(&key) {
                    Some(sample) => {
                        if examples.contains_key(&name) {
                            examples.put(name.clone(), sample);
                        }
                    }
                    // replace(name, null)
                    None => {
                        examples.put_null_value(&name);
                    }
                }
            }
        }
        Ok(Some(examples.clone()))
    }

    fn aggregate_schema_combinators(&mut self, source: &Schema, target: &Schema, to_aggregate: &JList<Schema>, examples: &mut Vec<JavaObject>, default_values: &mut Vec<JavaObject>) -> JResult<()> {
        let mut required_properties: Vec<String> = Vec::new();
        for inner in to_aggregate.to_vec() {
            let Some(resolved) = self.resolve_schema(Some(&inner))? else {
                return Err(JavaException::npe("Cannot invoke \"io.swagger.v3.oas.models.media.Schema.getProperties()\" because \"resolved\" is null"));
            };
            if let Some(props) = resolved.get_properties() {
                for key in props.keys() {
                    let prop = resolved.get_properties().unwrap().get(&key);
                    let r = self.resolve_schema(prop.as_ref())?;
                    if let Some(r) = r {
                        target.add_properties(&key, r);
                    }
                }
                if let Some(req) = resolved.get_required() {
                    for r in req.to_vec() {
                        if !required_properties.contains(&r) {
                            required_properties.push(r);
                        }
                    }
                }
            }
            if let Some(all_of) = inner.get_all_of() {
                self.aggregate_schema_combinators(source, target, &all_of, examples, default_values)?;
            }
            if let Some(one_of) = inner.get_one_of() {
                self.aggregate_schema_combinators(source, target, &one_of, examples, default_values)?;
            }
            if let Some(any_of) = inner.get_any_of() {
                self.aggregate_schema_combinators(source, target, &any_of, examples, default_values)?;
            }
            if let Some(e) = resolved.get_enum() {
                target.set_enum(e);
            }
            if let Some(e) = resolved.get_example() {
                examples.push(e);
            }
            if let Some(d) = resolved.get_default() {
                default_values.push(d);
            } else if let Some(d) = source.get_default() {
                default_values.push(d);
            }
            if let Some(ext) = resolved.get_extensions() {
                for (k, v) in ext.entries() {
                    target.add_extension(&k, v);
                }
            }
            if let Some(ext) = source.get_extensions() {
                for (k, v) in ext.entries() {
                    target.add_extension(&k, v);
                }
            }
            if let Some(v) = resolved.get_maximum() {
                target.set_maximum(v);
            }
            if let Some(v) = resolved.get_exclusive_maximum() {
                target.set_exclusive_maximum(v);
            }
            if let Some(v) = resolved.get_minimum() {
                target.set_minimum(v);
            }
            if let Some(v) = resolved.get_exclusive_minimum() {
                target.set_exclusive_minimum(v);
            }
            if let Some(v) = resolved.get_max_length() {
                target.set_max_length(v);
            }
            if let Some(v) = resolved.get_min_length() {
                target.set_min_length(v);
            }
            if let Some(v) = resolved.get_pattern() {
                target.set_pattern(v);
            }
            if let Some(v) = resolved.get_max_items() {
                target.set_max_items(v);
            }
            if let Some(v) = resolved.get_min_items() {
                target.set_min_items(v);
            }
            if let Some(v) = resolved.get_unique_items() {
                target.set_unique_items(v);
            }
            if let Some(v) = resolved.get_max_properties() {
                target.set_max_properties(v);
            }
            if let Some(v) = resolved.get_min_properties() {
                target.set_min_properties(v);
            }
            if let Some(v) = resolved.get_type() {
                target.set_type(v);
            }
            if let Some(v) = resolved.get_description() {
                target.set_description(v);
            }
            if let Some(v) = resolved.get_format() {
                target.set_format(v);
            }
            if let Some(v) = resolved.get_nullable() {
                target.set_nullable(v);
            }
            if let Some(v) = resolved.get_read_only() {
                target.set_read_only(v);
            }
            if let Some(v) = resolved.get_write_only() {
                target.set_write_only(v);
            }
            if let Some(v) = resolved.get_exclusive_maximum_value() {
                target.set_exclusive_maximum_value(v);
            }
            if let Some(v) = resolved.get_exclusive_minimum_value() {
                target.set_exclusive_minimum_value(v);
            }
            if let Some(v) = resolved.get_max_contains() {
                target.set_max_contains(v);
            }
            if let Some(v) = resolved.get_min_contains() {
                target.set_min_contains(v);
            }
        }
        if !required_properties.is_empty() {
            let required: Vec<String> = {
                let mut v = target.get_required().map(|r| r.to_vec()).unwrap_or_default();
                // HashSet<String> iteration order
                let ordered = java_hash_set(required_properties.iter().map(|s| JavaObject::string(s.as_str())));
                v.extend(ordered.into_iter().map(|o| o.to_java_string()));
                v
            };
            target.set_required(JList::from_vec(required));
        }
        Ok(())
    }
}

/// `getSchemaType(schema)`
fn get_schema_type(schema: &Schema) -> Option<String> {
    if schema.get_spec_version() == Some(SpecVersion::V30) {
        return schema.get_type();
    }
    match schema.get_types() {
        Some(t) if t.len() == 1 => t.items().into_iter().next(),
        _ => None,
    }
}

/// `hasSchemaType(schema)`
fn has_schema_type(schema: &Schema) -> bool {
    if schema.get_spec_version() == Some(SpecVersion::V30) {
        return schema.get_type().is_some();
    }
    schema.get_types().is_some_and(|t| !t.is_empty())
}
