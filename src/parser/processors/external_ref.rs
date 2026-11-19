//! `ExternalRefProcessor`: loads external references and moves their targets into `components`.

use super::mocks;
use super::{case_insensitive_key, reference_path_utils, ComponentNameAllocator};
use crate::java::path::JavaPath;
use crate::java::{JMap, JResult, JavaException};
use crate::models::*;
use crate::parser::ref_utils::{compute_definition_name, compute_ref_format, get_external_path, is_an_external_ref_format, java_split, RefFormat, RefType};
use crate::parser::resolver_cache::{load_ref, Cache};
use std::rc::Rc;

/// Port of `io.swagger.v3.parser.processors.ExternalRefProcessor`: loads a component from an external file or URL, adds it to `components` under a free name and returns the new local reference.
pub struct ExternalRefProcessor {
    cache: Rc<dyn Cache>,
    open_api: OpenAPI,
}

/// `$ref.split("#/")[0]`
fn file_part(r: &str) -> String {
    java_split(r, "#/").into_iter().next().unwrap_or_default()
}

/// `Paths.get(parent, more).normalize().toString()`
fn join_normalized(parent: &str, more: &str) -> String {
    JavaPath::get(parent, &[more]).normalize().to_string()
}

/// `file.substring(0, file.lastIndexOf('/'))`
fn parent_of(file: &str) -> JResult<String> {
    match file.rfind('/') {
        Some(i) => Ok(file[..i].to_string()),
        None => Err(JavaException::new(
            "java.lang.StringIndexOutOfBoundsException",
            format!("begin 0, end -1, length {}", file.encode_utf16().count()),
        )),
    }
}

/// The `fullRef` rewrite of `processRefToExternalResponse` / `processRefToExternalParameter`
fn relative_full_ref(file: &str, full_ref: &str, format: RefFormat) -> JResult<String> {
    if format == RefFormat::Url {
        return Ok(full_ref.to_string());
    }
    let parent = parent_of(file)?;
    if parent.is_empty() {
        return Ok(full_ref.to_string());
    }
    if full_ref.contains("#/") {
        let parts = java_split(full_ref, "#/");
        let file_part = parts.first().cloned().unwrap_or_default();
        let internal = parts.get(1).cloned().ok_or_else(|| JavaException::new("java.lang.ArrayIndexOutOfBoundsException", "Index 1 out of bounds for length 1"))?;
        Ok(format!("{}#/{internal}", join_normalized(&parent, &file_part)))
    } else {
        Ok(join_normalized(&parent, full_ref))
    }
}

fn is_not_blank(s: Option<&str>) -> bool {
    s.is_some_and(|s| !s.trim().is_empty())
}

impl ExternalRefProcessor {
    /// `new ExternalRefProcessor(cache, openAPI)`.
    pub fn new(cache: Rc<dyn Cache>, open_api: &OpenAPI) -> ExternalRefProcessor {
        let _constructing = mocks::constructed("ExternalRefProcessor");
        ExternalRefProcessor { cache, open_api: open_api.clone() }
    }

    fn allocator(&self) -> ComponentNameAllocator<'_> {
        ComponentNameAllocator { cache: self.cache.as_ref() }
    }

    fn components(&self) -> Components {
        match self.open_api.get_components() {
            Some(c) => c,
            None => {
                let c = Components::new();
                self.open_api.set_components(c.clone());
                c
            }
        }
    }

    fn mocked(&self, kind: &str, r: &str, format: RefFormat) -> Option<JResult<Option<String>>> {
        mocks::current().external_ref.and_then(|f| f(kind, r, format))
    }

    /// `processRefToExternalSchema($ref, refFormat)`
    pub fn process_ref_to_external_schema(&self, r: &str, format: RefFormat) -> JResult<Option<String>> {
        if let Some(m) = self.mocked("Schema", r, format) {
            return m;
        }
        self.schema_impl(r, format).map(Some)
    }

    fn schema_impl(&self, r: &str, format: RefFormat) -> JResult<String> {
        if let Some(renamed) = self.cache.get_renamed_ref(Some(r)) {
            return Ok(renamed);
        }
        let Some(schema) = load_ref::<Schema>(self.cache.as_ref(), r, format)? else {
            return Ok(r.to_string());
        };
        let components = self.components();
        let schemas = components.get_schemas().unwrap_or_default();
        let base = compute_definition_name(r)?;
        let cache = self.cache.clone();
        let new_ref = self.allocator().allocate_with(
            &schemas,
            &base,
            &schema,
            r,
            |s: &Schema| s.get_ref(),
            case_insensitive_key(&schemas),
            move |_existing, inc: &Schema| cache.get_renamed_ref(inc.get_ref().as_deref()).is_some(),
        );
        self.cache.put_renamed_ref(r, &new_ref);
        let mut existing = schemas.get(&new_ref);
        if existing.as_ref().is_some_and(|e| e.get_ref().is_some()) {
            existing = None;
        }
        if existing.is_none() {
            components.add_schemas(&new_ref, schema.clone());
            self.cache.add_referenced_key(&new_ref);
            let file = file_part(r);
            if let Some(schema_ref) = schema.get_ref() {
                let rf = compute_ref_format(&schema_ref);
                if is_an_external_ref_format(rf) {
                    if rf != RefFormat::Url {
                        let mut full = schema_ref.clone();
                        let parent = match file.rfind('/') {
                            Some(i) => file[..i].to_string(),
                            None => String::new(),
                        };
                        if !parent.is_empty() && !full.starts_with('/') {
                            if full.contains("#/") {
                                let parts = java_split(&full, "#/");
                                let fp = parts.first().cloned().unwrap_or_default();
                                let ip = parts.get(1).cloned().ok_or_else(|| JavaException::new("java.lang.ArrayIndexOutOfBoundsException", "Index 1 out of bounds for length 1"))?;
                                full = format!("{}#/{ip}", join_normalized(&parent, &fp));
                            } else {
                                full = join_normalized(&parent, &full);
                            }
                            full = full.replace('\\', "/");
                        }
                        let processed = self.process_ref_to_external_schema(&full, rf)?;
                        schema.set_ref(processed);
                    }
                } else {
                    self.process_ref_to_external_schema(&format!("{file}{schema_ref}"), RefFormat::Relative)?;
                }
            }
            if schema.is_composed_schema() {
                self.process_composed_schema(&schema, &file)?;
            }
            self.process_properties_map(schema.get_properties().as_ref(), &file)?;
            self.process_discriminator(schema.get_discriminator().as_ref(), &file)?;
            if let Some(AdditionalProperties::Schema(ap)) = schema.get_additional_properties() {
                if ap.get_ref().is_some() {
                    self.process_ref_schema(&ap, &file)?;
                } else if ap.is_array_schema() {
                    if let Some(items) = ap.get_items() && is_not_blank(items.get_ref().as_deref()) {
                        self.process_ref_schema(&items, &file)?;
                    }
                } else if let Some(AdditionalProperties::Schema(map_prop)) = ap.get_additional_properties() {
                    if map_prop.get_ref().is_some() {
                        self.process_ref_schema(&map_prop, &file)?;
                    } else if matches!(map_prop.get_additional_properties(), Some(AdditionalProperties::Schema(ref s)) if s.is_array_schema()) {
                        // `((ArraySchema) mapProp)`: a ClassCastException unless mapProp is itself an ArraySchema
                        if !map_prop.is_array_schema() {
                            return Err(JavaException::class_cast(&map_prop.kind().class_name(), "io.swagger.v3.oas.models.media.ArraySchema"));
                        }
                        if let Some(items) = map_prop.get_items() && is_not_blank(items.get_ref().as_deref()) {
                            self.process_ref_schema(&items, &file)?;
                        }
                    }
                }
            }
            if schema.is_array_schema() && let Some(items) = schema.get_items() {
                if is_not_blank(items.get_ref().as_deref()) {
                    self.process_ref_schema(&items, &file)?;
                } else {
                    if items.is_composed_schema() {
                        self.process_composed_schema(&items, &file)?;
                    }
                    self.process_properties_map(items.get_properties().as_ref(), &file)?;
                }
            }
        }
        Ok(new_ref)
    }

    fn process_items(&self, items: Option<crate::java::JList<Schema>>, file: &str) -> JResult<()> {
        if let Some(list) = items {
            for item in list.to_vec() {
                if item.get_ref().is_some() {
                    self.process_ref_schema(&item, file)?;
                } else {
                    self.process_schema(Some(&item), file)?;
                }
            }
        }
        Ok(())
    }

    fn process_composed_schema(&self, composed: &Schema, file: &str) -> JResult<()> {
        self.process_items(composed.get_all_of(), file)?;
        self.process_items(composed.get_one_of(), file)?;
        self.process_items(composed.get_any_of(), file)
    }

    fn process_schema(&self, property: Option<&Schema>, file: &str) -> JResult<()> {
        let Some(property) = property else { return Ok(()) };
        if is_not_blank(property.get_ref().as_deref()) {
            self.process_ref_schema(property, file)?;
        }
        if let Some(props) = property.get_properties() {
            self.process_properties_map(Some(&props), file)?;
        }
        if property.is_array_schema() {
            self.process_schema(property.get_items().as_ref(), file)?;
        }
        if let Some(AdditionalProperties::Schema(ap)) = property.get_additional_properties() {
            self.process_schema(Some(&ap), file)?;
        }
        if property.is_composed_schema() {
            let ref_map: Vec<(String, String)> = property.get_discriminator().and_then(|d| d.get_mapping()).map(|m| m.entries()).unwrap_or_default();
            let mut ref_cache: Vec<(String, Schema)> = Vec::new();
            if !ref_map.is_empty() && (property.get_any_of().is_some() || property.get_one_of().is_some()) {
                for l in [property.get_any_of(), property.get_one_of()].into_iter().flatten() {
                    for s in l.to_vec() {
                        if let Some(r) = s.get_ref() {
                            if ref_cache.iter().any(|(k, _)| *k == r) {
                                return Err(JavaException::new("java.lang.IllegalStateException", format!("Duplicate key {r}")));
                            }
                            ref_cache.push((r, s));
                        }
                    }
                }
            }
            self.process_properties_list(property.get_all_of(), file)?;
            self.process_properties_list(property.get_any_of(), file)?;
            self.process_properties_list(property.get_one_of(), file)?;
            if !ref_map.is_empty() && !ref_cache.is_empty() {
                // Collectors.toMap → HashMap iteration order
                let hm: JMap<Schema> = JMap::new_hash();
                for (k, v) in ref_cache {
                    hm.put(k, v);
                }
                for (key, value) in hm.entries() {
                    if Some(&key) != value.get_ref().as_ref() {
                        let new_ref = value.get_ref();
                        let mapping_key = ref_map.iter().find(|(k, _)| *k == key).map(|(_, v)| v.clone());
                        let mapping = property.get_discriminator().and_then(|d| d.get_mapping()).unwrap();
                        match (mapping_key, new_ref) {
                            (Some(k), Some(v)) => {
                                mapping.put(k, v);
                            }
                            // `refMap.get(key)` is null: `mapping.put(null, newRef)` (a LinkedHashMap accepts a null key)
                            (None, Some(v)) => {
                                mapping.put_null_key(v);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn process_properties_list(&self, props: Option<crate::java::JList<Schema>>, file: &str) -> JResult<()> {
        if let Some(l) = props {
            for p in l.to_vec() {
                self.process_schema(Some(&p), file)?;
            }
        }
        Ok(())
    }

    fn process_properties_map(&self, props: Option<&JMap<Schema>>, file: &str) -> JResult<()> {
        if let Some(m) = props {
            for p in m.values() {
                self.process_schema(Some(&p), file)?;
            }
        }
        Ok(())
    }

    /// `processRefToExternalPathItem($ref, refFormat)`
    pub fn process_ref_to_external_path_item(&self, r: &str, format: RefFormat) -> JResult<Option<PathItem>> {
        let path_item = load_ref::<PathItem>(self.cache.as_ref(), r, format)?;
        let paths = self.open_api.get_paths().unwrap_or_default();
        let name = compute_definition_name(r)?;
        if let Some(existing) = paths.get(&format!("/{name}")) && existing.get_ref().is_none() {
            return Ok(path_item);
        }
        self.cache.put_renamed_ref(r, &name);
        if let Some(p) = &path_item {
            self.process_path_item(p, r)?;
        }
        Ok(path_item)
    }

    fn process_path_item(&self, path_item: &PathItem, r: &str) -> JResult<()> {
        for (_, operation) in path_item.read_operations_map() {
            if let Some(responses) = operation.get_responses() {
                for (_, response) in responses.entries() {
                    if let Some(content) = response.get_content() {
                        for (_, media_type) in content.entries() {
                            if let Some(schema) = media_type.get_schema() {
                                self.process_ref_schema_object(&schema, r)?;
                                if let Some(ex) = media_type.get_examples() {
                                    self.process_ref_examples(&ex, r)?;
                                }
                            }
                        }
                    }
                    if let Some(links) = response.get_links() {
                        self.process_ref_links(&links, r)?;
                    }
                    if let Some(headers) = response.get_headers() {
                        self.process_ref_headers(&headers, r)?;
                    }
                }
            }
            if let Some(body) = operation.get_request_body() && let Some(content) = body.get_content() {
                for (_, media_type) in content.entries() {
                    if let Some(schema) = media_type.get_schema() {
                        self.process_ref_schema_object(&schema, r)?;
                    }
                }
            }
            if let Some(params) = operation.get_parameters() {
                for p in params.to_vec() {
                    if let Some(s) = p.get_schema() {
                        self.process_ref_schema_object(&s, r)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn process_discriminator(&self, d: Option<&Discriminator>, file: &str) -> JResult<()> {
        if let Some(mapping) = d.and_then(|d| d.get_mapping()) {
            for key in mapping.keys() {
                let r = mapping.get(&key);
                let subtype = Schema::new().ref_(r);
                self.process_schema(Some(&subtype), file)?;
                match subtype.get_ref() {
                    Some(v) => {
                        mapping.put(key, v);
                    }
                    None => {
                        mapping.put_null_value(&key);
                    }
                }
            }
        }
        Ok(())
    }

    /// `processRefToExternalResponse($ref, refFormat)`
    pub fn process_ref_to_external_response(&self, r: &str, format: RefFormat) -> JResult<Option<String>> {
        if let Some(m) = self.mocked("ApiResponse", r, format) {
            return m;
        }
        if let Some(renamed) = self.cache.get_renamed_ref(Some(r)) {
            return Ok(Some(renamed));
        }
        let Some(response) = load_ref::<ApiResponse>(self.cache.as_ref(), r, format)? else { return Ok(Some(r.to_string())) };
        let components = self.components();
        let responses = components.get_responses().unwrap_or_default();
        let new_ref = self.allocator().allocate(&responses, &compute_definition_name(r)?, &response, r, |x: &ApiResponse| x.get_ref());
        self.cache.put_renamed_ref(r, &new_ref);
        let existing = responses.get(&new_ref);
        if existing.as_ref().is_none_or(|e| e.get_ref().is_some()) {
            components.add_responses(&new_ref, response.clone());
            self.cache.add_referenced_key(&new_ref);
            let file = file_part(r);
            if let Some(rr) = response.get_ref() {
                let f = compute_ref_format(&rr);
                if is_an_external_ref_format(f) {
                    let full = relative_full_ref(&file, &rr, f)?;
                    let v = self.process_ref_to_external_response(&full, f)?;
                    response.set_ref(v);
                } else {
                    self.process_ref_to_external_response(&format!("{file}{rr}"), RefFormat::Relative)?;
                }
            }
        }
        if let Some(c) = response.get_content() {
            self.process_ref_content(&c, r)?;
        }
        if let Some(h) = response.get_headers() {
            self.process_ref_headers(&h, r)?;
        }
        if let Some(l) = response.get_links() {
            self.process_ref_links(&l, r)?;
        }
        Ok(Some(new_ref))
    }

    /// `processRefToExternalRequestBody($ref, refFormat)`
    pub fn process_ref_to_external_request_body(&self, r: &str, format: RefFormat) -> JResult<Option<String>> {
        if let Some(m) = self.mocked("RequestBody", r, format) {
            return m;
        }
        if let Some(renamed) = self.cache.get_renamed_ref(Some(r)) {
            return Ok(Some(renamed));
        }
        let Some(body) = load_ref::<RequestBody>(self.cache.as_ref(), r, format)? else { return Ok(Some(r.to_string())) };
        let components = self.components();
        let bodies = components.get_request_bodies().unwrap_or_default();
        let new_ref = self.allocator().allocate(&bodies, &compute_definition_name(r)?, &body, r, |x: &RequestBody| x.get_ref());
        self.cache.put_renamed_ref(r, &new_ref);
        let existing = bodies.get(&new_ref);
        if existing.as_ref().is_none_or(|e| e.get_ref().is_some()) {
            components.add_request_bodies(&new_ref, body.clone());
            self.cache.add_referenced_key(&new_ref);
            let file = file_part(r);
            if let Some(br) = body.get_ref() {
                let f = compute_ref_format(&br);
                if is_an_external_ref_format(f) {
                    let v = self.process_ref_to_external_request_body(&br, f)?;
                    body.set_ref(v);
                } else {
                    self.process_ref_to_external_request_body(&format!("{file}{br}"), RefFormat::Relative)?;
                }
            } else if let Some(c) = body.get_content() {
                self.process_ref_content(&c, r)?;
            }
        }
        Ok(Some(new_ref))
    }

    /// `processRefToExternalHeader($ref, refFormat)`
    pub fn process_ref_to_external_header(&self, r: &str, format: RefFormat) -> JResult<Option<String>> {
        if let Some(m) = self.mocked("Header", r, format) {
            return m;
        }
        if let Some(renamed) = self.cache.get_renamed_ref(Some(r)) {
            return Ok(Some(renamed));
        }
        let Some(header) = load_ref::<Header>(self.cache.as_ref(), r, format)? else { return Ok(Some(r.to_string())) };
        let components = self.components();
        let headers = components.get_headers().unwrap_or_default();
        let new_ref = self.allocator().allocate(&headers, &compute_definition_name(r)?, &header, r, |x: &Header| x.get_ref());
        self.cache.put_renamed_ref(r, &new_ref);
        let existing = headers.get(&new_ref);
        if existing.as_ref().is_none_or(|e| e.get_ref().is_some()) {
            components.add_headers(&new_ref, header.clone());
            self.cache.add_referenced_key(&new_ref);
            let file = file_part(r);
            if let Some(hr) = header.get_ref() {
                let f = compute_ref_format(&hr);
                if is_an_external_ref_format(f) {
                    let v = self.process_ref_to_external_header(&hr, f)?;
                    header.set_ref(v);
                } else {
                    self.process_ref_to_external_header(&format!("{file}{hr}"), RefFormat::Relative)?;
                }
            }
        }
        if let Some(c) = header.get_content() {
            self.process_ref_content(&c, r)?;
        }
        if let Some(s) = header.get_schema() {
            self.process_ref_schema_object(&s, r)?;
        }
        Ok(Some(new_ref))
    }

    /// `processRefToExternalSecurityScheme($ref, refFormat)`
    pub fn process_ref_to_external_security_scheme(&self, r: &str, format: RefFormat) -> JResult<Option<String>> {
        if let Some(m) = self.mocked("SecurityScheme", r, format) {
            return m;
        }
        if let Some(renamed) = self.cache.get_renamed_ref(Some(r)) {
            return Ok(Some(renamed));
        }
        let Some(ss) = load_ref::<SecurityScheme>(self.cache.as_ref(), r, format)? else { return Ok(Some(r.to_string())) };
        let components = self.components();
        let map = components.get_security_schemes().unwrap_or_default();
        let new_ref = self.allocator().allocate(&map, &compute_definition_name(r)?, &ss, r, |x: &SecurityScheme| x.get_ref());
        self.cache.put_renamed_ref(r, &new_ref);
        let existing = map.get(&new_ref);
        if existing.as_ref().is_none_or(|e| e.get_ref().is_some()) {
            components.add_security_schemes(&new_ref, ss.clone());
            self.cache.add_referenced_key(&new_ref);
            let file = file_part(r);
            if let Some(sr) = ss.get_ref() {
                let f = compute_ref_format(&sr);
                if is_an_external_ref_format(f) {
                    let v = self.process_ref_to_external_security_scheme(&sr, f)?;
                    ss.set_ref(v);
                } else {
                    self.process_ref_to_external_security_scheme(&format!("{file}{sr}"), RefFormat::Relative)?;
                }
            }
        }
        Ok(Some(new_ref))
    }

    /// `processRefToExternalLink($ref, refFormat)`
    pub fn process_ref_to_external_link(&self, r: &str, format: RefFormat) -> JResult<Option<String>> {
        if let Some(m) = self.mocked("Link", r, format) {
            return m;
        }
        if let Some(renamed) = self.cache.get_renamed_ref(Some(r)) {
            return Ok(Some(renamed));
        }
        let Some(link) = load_ref::<Link>(self.cache.as_ref(), r, format)? else { return Ok(Some(r.to_string())) };
        let components = self.components();
        let links = components.get_links().unwrap_or_default();
        let new_ref = self.allocator().allocate(&links, &compute_definition_name(r)?, &link, r, |x: &Link| x.get_ref());
        self.cache.put_renamed_ref(r, &new_ref);
        let existing = links.get(&new_ref);
        if existing.as_ref().is_none_or(|e| e.get_ref().is_some()) {
            components.add_links(&new_ref, link.clone());
            self.cache.add_referenced_key(&new_ref);
            let file = file_part(r);
            if let Some(lr) = link.get_ref() {
                let f = compute_ref_format(&lr);
                if is_an_external_ref_format(f) {
                    let v = self.process_ref_to_external_link(&lr, f)?;
                    link.set_ref(v);
                } else {
                    self.process_ref_to_external_link(&format!("{file}{lr}"), RefFormat::Relative)?;
                }
            }
        }
        Ok(Some(new_ref))
    }

    /// `processRefToExternalExample($ref, refFormat)`
    pub fn process_ref_to_external_example(&self, r: &str, format: RefFormat) -> JResult<Option<String>> {
        if let Some(m) = self.mocked("Example", r, format) {
            return m;
        }
        if let Some(renamed) = self.cache.get_renamed_ref(Some(r)) {
            return Ok(Some(renamed));
        }
        let Some(example) = load_ref::<Example>(self.cache.as_ref(), r, format)? else { return Ok(Some(r.to_string())) };
        let components = self.components();
        let examples = components.get_examples().unwrap_or_default();
        let new_ref = self.allocator().allocate(&examples, &compute_definition_name(r)?, &example, r, |x: &Example| x.get_ref());
        self.cache.put_renamed_ref(r, &new_ref);
        let existing = examples.get(&new_ref);
        if existing.as_ref().is_none_or(|e| e.get_ref().is_some()) {
            components.add_examples(&new_ref, example.clone());
            self.cache.add_referenced_key(&new_ref);
            let file = file_part(r);
            if let Some(er) = example.get_ref() {
                let f = compute_ref_format(&er);
                if is_an_external_ref_format(f) {
                    let v = self.process_ref_to_external_example(&er, f)?;
                    example.set_ref(v);
                } else {
                    self.process_ref_to_external_example(&format!("{file}{er}"), RefFormat::Relative)?;
                }
            }
        }
        Ok(Some(new_ref))
    }

    /// `processRefToExternalParameter($ref, refFormat)`
    pub fn process_ref_to_external_parameter(&self, r: &str, format: RefFormat) -> JResult<Option<String>> {
        if let Some(m) = self.mocked("Parameter", r, format) {
            return m;
        }
        if let Some(renamed) = self.cache.get_renamed_ref(Some(r)) {
            return Ok(Some(renamed));
        }
        let Some(parameter) = load_ref::<Parameter>(self.cache.as_ref(), r, format)? else { return Ok(Some(r.to_string())) };
        let components = self.components();
        let parameters = components.get_parameters().unwrap_or_default();
        let new_ref = self.allocator().allocate(&parameters, &compute_definition_name(r)?, &parameter, r, |x: &Parameter| x.get_ref());
        self.cache.put_renamed_ref(r, &new_ref);
        let existing = parameters.get(&new_ref);
        if existing.as_ref().is_none_or(|e| e.get_ref().is_some()) {
            components.add_parameters(&new_ref, parameter.clone());
            self.cache.add_referenced_key(&new_ref);
            let file = file_part(r);
            if let Some(pr) = parameter.get_ref() {
                let f = compute_ref_format(&pr);
                if is_an_external_ref_format(f) {
                    let full = relative_full_ref(&file, &pr, f)?;
                    let v = self.process_ref_to_external_parameter(&full, f)?;
                    parameter.set_ref(v);
                } else {
                    self.process_ref_to_external_parameter(&format!("{file}{pr}"), RefFormat::Relative)?;
                }
            }
        }
        if let Some(c) = parameter.get_content() {
            self.process_ref_content(&c, r)?;
        }
        if let Some(s) = parameter.get_schema() {
            self.process_ref_schema_object(&s, r)?;
        }
        if let Some(e) = parameter.get_examples() {
            self.process_ref_examples(&e, r)?;
        }
        Ok(Some(new_ref))
    }

    /// `processRefToExternalCallback($ref, refFormat)`
    pub fn process_ref_to_external_callback(&self, r: &str, format: RefFormat) -> JResult<Option<String>> {
        if let Some(m) = self.mocked("Callback", r, format) {
            return m;
        }
        if let Some(renamed) = self.cache.get_renamed_ref(Some(r)) {
            return Ok(Some(renamed));
        }
        let Some(callback) = load_ref::<Callback>(self.cache.as_ref(), r, format)? else { return Ok(Some(r.to_string())) };
        let components = self.components();
        let callbacks = components.get_callbacks().unwrap_or_default();
        let new_ref = self.allocator().allocate(&callbacks, &compute_definition_name(r)?, &callback, r, |x: &Callback| x.get_ref());
        self.cache.put_renamed_ref(r, &new_ref);
        let existing = callbacks.get(&new_ref);
        if existing.as_ref().is_none_or(|e| e.get_ref().is_some()) {
            components.add_callbacks(&new_ref, callback.clone());
            self.cache.add_referenced_key(&new_ref);
            let file = file_part(r);
            if let Some(cr) = callback.get_ref() {
                let f = compute_ref_format(&cr);
                if is_an_external_ref_format(f) {
                    let v = self.process_ref_to_external_callback(&cr, f)?;
                    callback.set_ref(v);
                } else {
                    self.process_ref_to_external_callback(&format!("{file}{cr}"), RefFormat::Relative)?;
                }
            } else {
                for path in callback.keys() {
                    let Some(path_item) = callback.get(&path) else { continue };
                    if let Some(pr) = path_item.get_ref() {
                        let prf = compute_ref_format(&pr);
                        // note: tests the callback's own format, as upstream does
                        let loaded = if is_an_external_ref_format(format) {
                            self.process_ref_to_external_path_item(&pr, prf)?
                        } else {
                            load_ref::<PathItem>(self.cache.as_ref(), &pr, format)?
                        };
                        match loaded {
                            Some(p) => {
                                callback.put(path.clone(), p);
                            }
                            // `callback.put(path, null)`: a null value is skipped when serialized
                            None => {
                                callback.remove(&path);
                            }
                        }
                    } else {
                        self.process_path_item(&path_item, r)?;
                    }
                }
            }
        }
        Ok(Some(new_ref))
    }

    fn process_ref_content(&self, content: &Content, r: &str) -> JResult<()> {
        for media_type in content.values() {
            if let Some(s) = media_type.get_schema() {
                self.process_ref_schema_object(&s, r)?;
            }
            if let Some(e) = media_type.get_examples() {
                self.process_ref_examples(&e, r)?;
            }
        }
        Ok(())
    }

    fn process_ref_examples(&self, examples: &JMap<Example>, r: &str) -> JResult<()> {
        let file = file_part(r);
        for example in examples.values() {
            if let Some(er) = example.get_ref() {
                let f = compute_ref_format(&er);
                if is_an_external_ref_format(f) {
                    let ex = example.clone();
                    self.process_ref(&er, &file, &move |v| ex.set_ref(v), &|a, b| self.process_ref_to_external_example(a, b).map(|_| ()), &|x| join(Some(&file), Some(x)))?;
                } else {
                    self.process_ref_to_external_example(&format!("{file}{er}"), RefFormat::Relative)?;
                }
            }
        }
        Ok(())
    }

    fn process_ref_schema_object(&self, schema: &Schema, r: &str) -> JResult<()> {
        let file = file_part(r);
        if let Some(sr) = schema.get_ref() {
            let f = compute_ref_format(&sr);
            if is_an_external_ref_format(f) {
                self.process_ref_schema(schema, &file)?;
            } else {
                self.process_ref_to_external_schema(&format!("{file}{sr}"), RefFormat::Relative)?;
            }
        } else {
            self.process_schema(Some(schema), &file)?;
        }
        Ok(())
    }

    fn process_ref_headers(&self, headers: &JMap<Header>, r: &str) -> JResult<()> {
        let file = file_part(r);
        for header in headers.values() {
            if let Some(hr) = header.get_ref() {
                let f = compute_ref_format(&hr);
                if is_an_external_ref_format(f) {
                    let h = header.clone();
                    self.process_ref(&hr, &file, &move |v| h.set_ref(v), &|a, b| self.process_ref_to_external_header(a, b).map(|_| ()), &|x| join(Some(&file), Some(x)))?;
                } else {
                    self.process_ref_to_external_header(&format!("{file}{hr}"), RefFormat::Relative)?;
                }
            }
        }
        Ok(())
    }

    fn process_ref_links(&self, links: &JMap<Link>, r: &str) -> JResult<()> {
        let file = file_part(r);
        for link in links.values() {
            if let Some(lr) = link.get_ref() {
                let f = compute_ref_format(&lr);
                if is_an_external_ref_format(f) {
                    let l = link.clone();
                    self.process_ref(&lr, &file, &move |v| l.set_ref(v), &|a, b| self.process_ref_to_external_link(a, b).map(|_| ()), &|x| join(Some(&file), Some(x)))?;
                } else {
                    self.process_ref_to_external_link(&format!("{file}{lr}"), RefFormat::Relative)?;
                }
            }
        }
        Ok(())
    }

    fn process_ref_schema(&self, sub_ref: &Schema, external_file: &str) -> JResult<()> {
        let r = sub_ref.get_ref();
        let s1 = sub_ref.clone();
        let s2 = sub_ref.clone();
        self.process_ref(
            r.as_deref().unwrap_or(""),
            external_file,
            &move |v| s1.set_ref(v),
            &|a, b| self.process_ref_to_external_schema(a, b).map(|_| ()),
            &move |_| self.construct_ref(&s2, external_file),
        )
    }

    /// `constructRef(refProperty, rootLocation)`
    pub fn construct_ref(&self, ref_property: &Schema, root_location: &str) -> Option<String> {
        join(Some(root_location), ref_property.get_ref().as_deref())
    }

    fn process_ref(
        &self,
        r: &str,
        external_file: &str,
        ref_setter: &dyn Fn(Option<String>),
        external_ref_processor: &dyn Fn(&str, RefFormat) -> JResult<()>,
        relative_ref_resolver: &dyn Fn(&str) -> Option<String>,
    ) -> JResult<()> {
        let format = compute_ref_format(r);
        if !is_an_external_ref_format(format) {
            let processed = self.process_ref_to_external_schema(&format!("{external_file}{r}"), RefFormat::Relative)?;
            ref_setter(Some(format!("{}{}", RefType::Schemas.internal_prefix(), processed.unwrap_or_else(|| "null".into()))));
            return Ok(());
        }
        let sub_ref_external_path = get_external_path(Some(r));
        if format == RefFormat::Relative && sub_ref_external_path.as_deref() != Some(external_file) {
            ref_setter(relative_ref_resolver(r));
        } else {
            external_ref_processor(r, format)?;
        }
        Ok(())
    }
}

/// `ExternalRefProcessor.join(source, fragment)`
pub fn join(source: Option<&str>, fragment: Option<&str>) -> Option<String> {
    reference_path_utils::resolve(source, fragment)
}
