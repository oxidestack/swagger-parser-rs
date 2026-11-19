//! `io.swagger.v3.parser.util.InlineModelResolver`: moves inline object schemas into `components/schemas`.

use crate::java::{JList, JMap, JResult, JavaException};
use crate::models::*;
use jackson::JavaObject;
use std::cell::RefCell;
use std::collections::HashMap;

const SCHEMAS_REF: &str = "#/components/schemas/";

/// Port of `io.swagger.v3.parser.util.InlineModelResolver`: replaces inline object schemas in paths and components with `$ref`s to new entries of `components/schemas`.
pub struct InlineModelResolver {
    open_api: RefCell<Option<OpenAPI>>,
    added_models: RefCell<JMap<Schema>>,
    /// `generatedSignature`: `Json.pretty(model)` (possibly `null`) → name
    generated_signature: RefCell<HashMap<Option<String>, String>>,
    /// `flattenComposedSchemas`: also move the inline members of `allOf`/`anyOf`/`oneOf` into components.
    pub flatten_composed_schemas: bool,
    /// `camelCaseFlattenNaming`: build the names of generated models in camel case.
    pub camel_case_flatten_naming: bool,
    /// `skipMatches`: always generate a new model, never reuse an identical one generated earlier.
    pub skip_matches: bool,
}

fn sioobe(begin: usize, end: usize, len: usize) -> JavaException {
    JavaException::new("java.lang.StringIndexOutOfBoundsException", format!("begin {begin}, end {end}, length {len}"))
}

fn ref_schema(r: String) -> Schema {
    Schema::new().ref_(r)
}

/// `isObjectSchema(schema)`
fn is_object_schema(schema: Option<&Schema>) -> bool {
    let Some(s) = schema else { return false };
    s.is_object_schema()
        || s.get_type().is_some_and(|t| t.eq_ignore_ascii_case("object"))
        || (s.get_type().is_none() && s.get_properties().is_some_and(|p| !p.is_empty()) || s.is_composed_schema())
}

fn has_properties(s: &Schema) -> bool {
    s.get_properties().is_some_and(|p| !p.is_empty())
}

/// `pathBody(pathname)`
fn path_body(pathname: &str) -> String {
    let parts = crate::parser::ref_utils::java_split(pathname, "/");
    let mut body = String::new();
    if parts.len() > 2 {
        body.push_str(&parts[parts.len() - 2].replace('.', "_"));
        body.push('_');
    }
    if parts.len() > 1 {
        body.push_str(&parts[parts.len() - 1].replace('.', "_"));
        body.push('_');
    }
    body.push_str("body");
    body
}

/// `String.split("[-|\\s|_]")`
fn split_name(key: &str) -> Vec<String> {
    let mut v: Vec<String> = key.split(|c: char| c == '-' || c == '|' || c == '_' || c.is_whitespace() && (c.is_ascii_whitespace() || c == '\u{b}')).map(str::to_string).collect();
    while v.len() > 1 && v.last().is_some_and(|x| x.is_empty()) {
        v.pop();
    }
    if v.len() == 1 && v[0].is_empty() && !key.is_empty() {
        v.clear();
    }
    v
}

/// `replaceAll("[^a-z_\\.A-Z0-9 ]", "")`
fn strip_name(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '.' || *c == ' ').collect()
}

impl InlineModelResolver {
    /// `new InlineModelResolver(flattenComposedSchemas, camelCaseFlattenNaming, skipMatches)`.
    pub fn new(flatten_composed_schemas: bool, camel_case_flatten_naming: bool, skip_matches: bool) -> InlineModelResolver {
        InlineModelResolver {
            open_api: RefCell::new(None),
            added_models: RefCell::new(JMap::new_hash()),
            generated_signature: RefCell::new(HashMap::new()),
            flatten_composed_schemas,
            camel_case_flatten_naming,
            skip_matches,
        }
    }

    fn api(&self) -> OpenAPI {
        self.open_api.borrow().clone().expect("flatten sets openAPI")
    }

    fn components(&self) -> Components {
        self.api().get_components().expect("flatten ensures components")
    }

    /// `flatten(openAPI)`
    pub fn flatten(&self, open_api: &OpenAPI) -> JResult<()> {
        *self.open_api.borrow_mut() = Some(open_api.clone());
        if let Some(c) = open_api.get_components() && c.get_schemas().is_none() {
            c.set_schemas(JMap::new_hash());
        }
        let paths = open_api.get_paths();
        if open_api.get_components().is_none() {
            open_api.set_components(Components::new());
        }
        let models = open_api.get_components().unwrap().get_schemas();
        self.flatten_paths(paths)?;
        self.flatten_definitions(models)
    }

    fn flatten_paths(&self, paths: Option<Paths>) -> JResult<()> {
        let Some(paths) = paths else { return Ok(()) };
        for pathname in paths.keys() {
            let path = paths.get(&pathname).unwrap();
            for operation in path.read_operations() {
                self.flatten_body(&pathname, operation.get_request_body())?;
                self.flatten_params(&pathname, operation.get_parameters())?;
                self.flatten_responses(&pathname, operation.get_responses())?;
            }
        }
        Ok(())
    }

    fn flatten_body(&self, pathname: &str, body: Option<RequestBody>) -> JResult<()> {
        let Some(body) = body else { return Ok(()) };
        let Some(content) = body.get_content() else { return Ok(()) };
        for key in content.keys() {
            let Some(media_type) = content.get(&key) else { continue };
            let Some(model) = media_type.get_schema() else { continue };
            let generic_name = path_body(pathname);
            if has_properties(&model) {
                self.flatten_properties(model.get_properties().as_ref(), pathname)?;
                let model_name = self.resolve_model_name(model.get_title().as_deref(), &generic_name)?;
                media_type.set_schema(ref_schema(format!("{SCHEMAS_REF}{model_name}")));
                self.add_generated(&model_name, &model);
                self.components().add_schemas(&model_name, model);
            } else if model.is_composed_schema() {
                self.flatten_composed_schema(&model, pathname)?;
                if model.get_ref().is_none() {
                    let model_name = self.resolve_model_name(model.get_title().as_deref(), &generic_name)?;
                    media_type.set_schema(self.make_ref_property(&format!("{SCHEMAS_REF}{model_name}"), &model));
                    self.add_generated(&model_name, &model);
                    self.components().add_schemas(&model_name, model);
                }
            } else if model.is_array_schema() {
                let inner = model.get_items();
                if is_object_schema(inner.as_ref()) {
                    let inner = inner.unwrap();
                    if has_properties(&inner) {
                        self.flatten_properties(inner.get_properties().as_ref(), pathname)?;
                        let model_name = self.resolve_model_name(inner.get_title().as_deref(), &generic_name)?;
                        match self.match_generated(&inner) {
                            Some(existing) => model.set_items(ref_schema(format!("{SCHEMAS_REF}{existing}"))),
                            None => {
                                model.set_items(ref_schema(format!("{SCHEMAS_REF}{model_name}")));
                                self.add_generated(&model_name, &inner);
                                self.components().add_schemas(&model_name, inner);
                            }
                        }
                    } else if inner.is_composed_schema() && self.flatten_composed_schemas {
                        self.flatten_composed_schema(&inner, &key)?;
                        if inner.get_ref().is_none() {
                            let model_name = self.resolve_model_name(inner.get_title().as_deref(), &format!("inline_body_items_{key}_{pathname}"))?;
                            model.set_items(self.make_ref_property(&format!("{SCHEMAS_REF}{model_name}"), &inner));
                            self.add_generated(&model_name, &inner);
                            self.components().add_schemas(&model_name, inner);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn flatten_params(&self, pathname: &str, parameters: Option<JList<Parameter>>) -> JResult<()> {
        let Some(parameters) = parameters else { return Ok(()) };
        for parameter in parameters.to_vec() {
            let Some(model) = parameter.get_schema() else { continue };
            if model.get_properties().is_some() {
                if model.get_type().is_none_or(|t| t == "object") && has_properties(&model) {
                    self.flatten_properties(model.get_properties().as_ref(), pathname)?;
                    let model_name = self.resolve_model_name_opt(model.get_title().as_deref(), parameter.get_name().as_deref())?;
                    parameter.set_schema(ref_schema(format!("{SCHEMAS_REF}{model_name}")));
                    self.add_generated(&model_name, &model);
                    self.components().add_schemas(&model_name, model);
                }
            } else if model.is_composed_schema() {
                let model_name = self.resolve_model_name_opt(model.get_title().as_deref(), parameter.get_name().as_deref())?;
                parameter.set_schema(ref_schema(format!("{SCHEMAS_REF}{model_name}")));
                self.add_generated(&model_name, &model);
                self.components().add_schemas(&model_name, model);
            } else if model.is_array_schema() {
                let inner = model.get_items();
                if is_object_schema(inner.as_ref()) {
                    let inner = inner.unwrap();
                    if has_properties(&inner) {
                        self.flatten_properties(inner.get_properties().as_ref(), pathname)?;
                        let model_name = self.resolve_model_name_opt(inner.get_title().as_deref(), parameter.get_name().as_deref())?;
                        match self.match_generated(&inner) {
                            Some(existing) => model.set_items(ref_schema(format!("{SCHEMAS_REF}{existing}"))),
                            None => {
                                model.set_items(ref_schema(format!("{SCHEMAS_REF}{model_name}")));
                                self.add_generated(&model_name, &model);
                                self.components().add_schemas(&model_name, model.clone());
                            }
                        }
                    } else if inner.is_composed_schema() && self.flatten_composed_schemas {
                        let pname = parameter.get_name().unwrap_or_else(|| "null".into());
                        self.flatten_composed_schema(&inner, &pname)?;
                        if inner.get_ref().is_none() {
                            let model_name = self.resolve_model_name(inner.get_title().as_deref(), &format!("inline_parameter_items_{pname}"))?;
                            model.set_items(self.make_ref_property(&format!("{SCHEMAS_REF}{model_name}"), &inner));
                            self.add_generated(&model_name, &inner);
                            self.components().add_schemas(&model_name, inner);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn flatten_responses(&self, pathname: &str, responses: Option<ApiResponses>) -> JResult<()> {
        let Some(responses) = responses else { return Ok(()) };
        for key0 in responses.keys() {
            let mut key = key0.clone();
            let Some(response) = responses.get(&key0) else {
                return Err(JavaException::npe("Cannot invoke \"io.swagger.v3.oas.models.responses.ApiResponse.getContent()\" because \"response\" is null"));
            };
            let Some(content) = response.get_content() else { continue };
            for name in content.keys() {
                let Some(media) = content.get(&name) else { continue };
                let Some(media_schema) = media.get_schema() else { continue };
                if is_object_schema(Some(&media_schema)) {
                    if has_properties(&media_schema) || media_schema.is_composed_schema() {
                        let model_name = self.resolve_model_name(media_schema.get_title().as_deref(), &format!("inline_response_{key}"))?;
                        match self.match_generated(&media_schema) {
                            Some(existing) => media.set_schema(self.make_ref_property(&format!("{SCHEMAS_REF}{existing}"), &media_schema)),
                            None => {
                                media.set_schema(self.make_ref_property(&format!("{SCHEMAS_REF}{model_name}"), &media_schema));
                                self.add_generated(&model_name, &media_schema);
                                self.components().add_schemas(&model_name, media_schema);
                            }
                        }
                    } else if let Some(AdditionalProperties::Schema(inner_property)) = media_schema.get_additional_properties() {
                        if is_object_schema(Some(&inner_property)) {
                            key = format!("inline_response_map{key}");
                            self.flatten_map_schema(&inner_property, &key, pathname, &media_schema)?;
                        } else if inner_property.is_array_schema() {
                            let inner = inner_property.get_items();
                            if is_object_schema(inner.as_ref()) {
                                key = format!("inline_response_map_items{key}");
                                self.flatten_map_schema(&inner.unwrap(), &key, pathname, &media_schema)?;
                            }
                        }
                    }
                } else if media_schema.is_array_schema() {
                    let inner = media_schema.get_items();
                    if is_object_schema(inner.as_ref()) {
                        self.flatten_array_schema(&inner.unwrap(), &key, pathname, &media_schema)?;
                    }
                } else if let Some(AdditionalProperties::Schema(inner_property)) = media_schema.get_additional_properties()
                    && is_object_schema(Some(&inner_property))
                {
                    key = format!("inline_response_map{key}");
                    self.flatten_map_schema(&inner_property, &key, pathname, &media_schema)?;
                }
            }
        }
        Ok(())
    }

    fn flatten_definitions(&self, models: Option<JMap<Schema>>) -> JResult<()> {
        let Some(models) = models else { return Ok(()) };
        for model_name0 in models.keys() {
            let mut model_name = model_name0.clone();
            let Some(model) = models.get(&model_name0) else {
                return Err(JavaException::npe("Cannot invoke \"io.swagger.v3.oas.models.media.Schema.getProperties()\" because \"model\" is null"));
            };
            if let Some(props) = model.get_properties() {
                self.flatten_properties(Some(&props), &model_name)?;
                fix_string_model(&model)?;
            } else if model.is_array_schema() {
                let inner = model.get_items();
                if is_object_schema(inner.as_ref()) {
                    let inner = inner.unwrap();
                    if has_properties(&inner) {
                        let inner_model_name = self.resolve_model_name(inner.get_title().as_deref(), &format!("{model_name}_inner"))?;
                        match self.match_generated(&inner) {
                            None => {
                                self.components().add_schemas(&inner_model_name, inner.clone());
                                self.add_generated(&inner_model_name, &inner);
                                model.set_items(ref_schema(format!("{SCHEMAS_REF}{inner_model_name}")));
                            }
                            Some(existing) => model.set_items(ref_schema(format!("{SCHEMAS_REF}{existing}"))),
                        }
                    } else if inner.is_composed_schema() && self.flatten_composed_schemas {
                        self.flatten_composed_schema(&inner, &model_name)?;
                        if inner.get_ref().is_none() {
                            model_name = self.resolve_model_name(inner.get_title().as_deref(), &format!("inline_array_items_{model_name}"))?;
                            model.set_items(self.make_ref_property(&format!("{SCHEMAS_REF}{model_name}"), &inner));
                            self.add_generated(&model_name, &inner);
                            self.components().add_schemas(&model_name, inner);
                        }
                    }
                }
            } else if model.is_composed_schema() {
                self.flatten_composed_list(&model, &model_name)?;
            }
        }
        Ok(())
    }

    /// The allOf/anyOf/oneOf loop shared by `flattenDefinitions` and `flattenComposedSchema`
    fn flatten_composed_list(&self, composed: &Schema, key: &str) -> JResult<()> {
        let (list, mut inline_model_name) = if let Some(l) = composed.get_all_of() {
            (l, "AllOf".to_string())
        } else if let Some(l) = composed.get_any_of() {
            (l, "AnyOf".to_string())
        } else if let Some(l) = composed.get_one_of() {
            (l, "OneOf".to_string())
        } else {
            return Err(JavaException::npe("Cannot invoke \"java.util.List.size()\" because \"list\" is null"));
        };
        let mut i = 0;
        while i < list.len() {
            let item = list.get(i).unwrap();
            if item.get_ref().is_none() {
                let inline = item;
                if let Some(p) = inline.get_properties() {
                    self.flatten_properties(Some(&p), key)?;
                }
                if self.flatten_composed_schemas {
                    let position = i + 1;
                    inline_model_name = self.resolve_model_name(inline.get_title().as_deref(), &format!("{key}{inline_model_name}_{position}"))?;
                    list.set(i, ref_schema(format!("{SCHEMAS_REF}{inline_model_name}")));
                    self.add_generated(&inline_model_name, &inline);
                    self.components().add_schemas(&inline_model_name, inline);
                }
            }
            i += 1;
        }
        Ok(())
    }

    fn flatten_array_schema(&self, inner: &Schema, key: &str, pathname: &str, ap: &Schema) -> JResult<()> {
        if has_properties(inner) {
            self.flatten_properties(inner.get_properties().as_ref(), pathname)?;
            let key = format!("inline_response_{key}");
            let model_name = self.resolve_model_name(inner.get_title().as_deref(), &key)?;
            match self.match_generated(inner) {
                Some(existing) => ap.set_items(self.make_ref_property(&format!("{SCHEMAS_REF}{existing}"), inner)),
                None => {
                    ap.set_items(self.make_ref_property(&format!("{SCHEMAS_REF}{model_name}"), inner));
                    self.add_generated(&model_name, inner);
                    self.components().add_schemas(&model_name, inner.clone());
                }
            }
        } else if inner.is_composed_schema() && self.flatten_composed_schemas {
            self.flatten_composed_schema(inner, key)?;
            if inner.get_ref().is_none() {
                let key = format!("inline_response_items{key}");
                let model_name = self.resolve_model_name(inner.get_title().as_deref(), &key)?;
                ap.set_items(self.make_ref_property(&format!("{SCHEMAS_REF}{model_name}"), inner));
                self.add_generated(&model_name, inner);
                self.components().add_schemas(&model_name, inner.clone());
            }
        }
        Ok(())
    }

    fn flatten_map_schema(&self, inner_property: &Schema, key: &str, pathname: &str, media_schema: &Schema) -> JResult<()> {
        if has_properties(inner_property) {
            self.flatten_properties(inner_property.get_properties().as_ref(), pathname)?;
            let model_name = self.resolve_model_name(inner_property.get_title().as_deref(), key)?;
            match self.match_generated(inner_property) {
                Some(existing) => media_schema.set_additional_properties(ref_schema(format!("{SCHEMAS_REF}{existing}"))),
                None => {
                    media_schema.set_additional_properties(ref_schema(format!("{SCHEMAS_REF}{model_name}")));
                    self.add_generated(&model_name, inner_property);
                    self.components().add_schemas(&model_name, inner_property.clone());
                }
            }
        } else if inner_property.is_composed_schema() && self.flatten_composed_schemas {
            self.flatten_composed_schema(inner_property, key)?;
            if inner_property.get_ref().is_none() {
                let model_name = self.resolve_model_name(inner_property.get_title().as_deref(), key)?;
                media_schema.set_additional_properties(ref_schema(format!("{SCHEMAS_REF}{model_name}")));
                self.add_generated(&model_name, inner_property);
                self.components().add_schemas(&model_name, inner_property.clone());
            }
        }
        Ok(())
    }

    fn resolve_model_name(&self, title: Option<&str>, key: &str) -> JResult<String> {
        match title {
            None => self.unique_name(key),
            Some(t) => self.unique_name(t),
        }
    }

    fn resolve_model_name_opt(&self, title: Option<&str>, key: Option<&str>) -> JResult<String> {
        match (title, key) {
            (Some(t), _) => self.unique_name(t),
            (None, Some(k)) => self.unique_name(k),
            (None, None) => Err(JavaException::npe("Cannot invoke \"String.split(String)\" because \"key\" is null")),
        }
    }

    /// `matchGenerated(model)`
    pub fn match_generated(&self, model: &Schema) -> Option<String> {
        if self.skip_matches {
            return None;
        }
        self.generated_signature.borrow().get(&json_pretty(model)).cloned()
    }

    /// `addGenerated(name, model)`
    pub fn add_generated(&self, name: &str, model: &Schema) {
        self.generated_signature.borrow_mut().insert(json_pretty(model), name.to_string());
    }

    /// `uniqueName(key)`
    pub fn unique_name(&self, key: &str) -> JResult<String> {
        let key = if self.camel_case_flatten_naming {
            let mut concatenated = String::new();
            for part in split_name(key) {
                let units: Vec<u16> = part.encode_utf16().collect();
                if units.is_empty() {
                    return Err(sioobe(0, 1, 0));
                }
                let first = String::from_utf16_lossy(&units[..1]).to_uppercase();
                concatenated.push_str(&first);
                concatenated.push_str(&String::from_utf16_lossy(&units[1..]));
            }
            strip_name(&concatenated)
        } else {
            strip_name(key)
        };
        let mut count = 0;
        loop {
            let name = if count > 0 { format!("{key}_{count}") } else { key.clone() };
            match self.components().get_schemas() {
                None => return Ok(name),
                Some(s) if !s.contains_key(&name) => return Ok(name),
                _ => {}
            }
            count += 1;
        }
    }

    /// `flattenProperties(properties, path)`
    pub fn flatten_properties(&self, properties: Option<&JMap<Schema>>, path: &str) -> JResult<()> {
        let Some(properties) = properties else { return Ok(()) };
        let props_to_update: JMap<Schema> = JMap::new_hash();
        let models_to_add: JMap<Schema> = JMap::new_hash();
        for key in properties.keys() {
            let Some(property) = properties.get(&key) else {
                return Err(JavaException::npe("Cannot invoke \"io.swagger.v3.oas.models.media.Schema.getAdditionalProperties()\" because \"property\" is null"));
            };
            if is_object_schema(Some(&property)) && has_properties(&property) {
                let model_name = self.resolve_model_name(property.get_title().as_deref(), &format!("{path}_{key}"))?;
                let model = self.create_model_from_property(&property, &model_name)?;
                match self.match_generated(&model) {
                    Some(existing) => {
                        props_to_update.put(key.clone(), ref_schema(format!("{SCHEMAS_REF}{existing}")));
                    }
                    None => {
                        props_to_update.put(key.clone(), ref_schema(format!("{SCHEMAS_REF}{model_name}")));
                        models_to_add.put(model_name.clone(), model.clone());
                        self.add_generated(&model_name, &model);
                        self.components().add_schemas(&model_name, model);
                    }
                }
            } else if property.is_array_schema() {
                let inner = property.get_items();
                if is_object_schema(inner.as_ref()) {
                    let inner = inner.unwrap();
                    if has_properties(&inner) {
                        self.flatten_properties(inner.get_properties().as_ref(), path)?;
                        let model_name = self.resolve_model_name(inner.get_title().as_deref(), &format!("{path}_{key}"))?;
                        let inner_model = self.create_model_from_property(&inner, &model_name)?;
                        match self.match_generated(&inner_model) {
                            Some(existing) => property.set_items(ref_schema(format!("{SCHEMAS_REF}{existing}"))),
                            None => {
                                property.set_items(ref_schema(format!("{SCHEMAS_REF}{model_name}")));
                                self.add_generated(&model_name, &inner_model);
                                self.components().add_schemas(&model_name, inner_model);
                            }
                        }
                    } else if inner.is_composed_schema() && self.flatten_composed_schemas {
                        self.flatten_composed_schema(&inner, &key)?;
                        let model_name = self.resolve_model_name(inner.get_title().as_deref(), &format!("{path}_{key}"))?;
                        let inner_model = self.create_model_from_property(&inner, &model_name)?;
                        match self.match_generated(&inner_model) {
                            Some(existing) => property.set_items(ref_schema(format!("{SCHEMAS_REF}{existing}"))),
                            None => {
                                property.set_items(ref_schema(format!("{SCHEMAS_REF}{model_name}")));
                                self.add_generated(&model_name, &inner_model);
                                self.components().add_schemas(&model_name, inner_model);
                            }
                        }
                    }
                }
            } else if let Some(AdditionalProperties::Schema(inner)) = property.get_additional_properties()
                && is_object_schema(Some(&inner)) && has_properties(&inner)
            {
                self.flatten_properties(inner.get_properties().as_ref(), path)?;
                let model_name = self.resolve_model_name(inner.get_title().as_deref(), &format!("{path}_{key}"))?;
                let inner_model = self.create_model_from_property(&inner, &model_name)?;
                match self.match_generated(&inner_model) {
                    Some(existing) => property.set_additional_properties(ref_schema(format!("{SCHEMAS_REF}{existing}"))),
                    None => {
                        property.set_additional_properties(ref_schema(format!("{SCHEMAS_REF}{model_name}")));
                        self.add_generated(&model_name, &inner_model);
                        self.components().add_schemas(&model_name, inner_model);
                    }
                }
            }
        }
        for (k, v) in props_to_update.entries() {
            properties.put(k, v);
        }
        for (k, v) in models_to_add.entries() {
            self.components().add_schemas(&k, v.clone());
            self.added_models.borrow().put(k, v);
        }
        Ok(())
    }

    fn flatten_composed_schema(&self, inner: &Schema, key: &str) -> JResult<()> {
        if !inner.is_composed_schema() {
            return Err(JavaException::class_cast(&inner.kind().class_name(), "io.swagger.v3.oas.models.media.ComposedSchema"));
        }
        self.flatten_composed_list(inner, key)
    }

    /// `createModelFromProperty(schema, path)`
    pub fn create_model_from_property(&self, schema: &Schema, path: &str) -> JResult<Schema> {
        let description = schema.get_description();
        let example = schema.get_example().map(|o| o.to_java_string());
        let required = schema.get_required();
        let name = schema.get_name();
        let xml = schema.get_xml();
        let properties = schema.get_properties();
        if schema.is_composed_schema() && self.flatten_composed_schemas {
            schema.set_description(description);
            if example.is_some() || schema.get_example_set_flag() {
                schema.set_example(example.map(JavaObject::string));
            }
            schema.set_name(name);
            schema.set_xml(xml);
            schema.set_type(schema.get_type());
            schema.set_required(required);
            return Ok(schema.clone());
        }
        let model = Schema::new();
        model.set_additional_properties(schema.get_additional_properties());
        model.set_description(description);
        model.set_deprecated(schema.get_deprecated());
        model.set_discriminator(schema.get_discriminator());
        model.set_enum(schema.get_enum());
        if example.is_some() || schema.get_example_set_flag() {
            model.set_example(example.map(JavaObject::string));
        }
        model.set_exclusive_maximum(schema.get_exclusive_maximum());
        model.set_exclusive_minimum(schema.get_exclusive_minimum());
        model.set_format(schema.get_format());
        model.set_min_length(schema.get_min_length());
        model.set_maximum(schema.get_maximum());
        model.set_max_items(schema.get_max_items());
        model.set_max_properties(schema.get_max_properties());
        model.set_max_length(schema.get_max_length());
        model.set_minimum(schema.get_minimum());
        model.set_min_items(schema.get_min_items());
        model.set_min_length(schema.get_min_length());
        model.set_min_properties(schema.get_min_properties());
        model.set_multiple_of(schema.get_multiple_of());
        model.set_name(name);
        model.set_nullable(schema.get_nullable());
        model.set_not(schema.get_not());
        model.set_pattern(schema.get_pattern());
        model.set_read_only(schema.get_read_only());
        model.set_required(required);
        model.set_unique_items(schema.get_unique_items());
        model.set_title(schema.get_title());
        model.set_type(schema.get_type());
        model.set_xml(xml);
        model.set_write_only(schema.get_write_only());
        if let Some(p) = properties {
            self.flatten_properties(Some(&p), path)?;
            model.set_properties(p);
        }
        if schema.is_composed_schema() {
            model.set_all_of(schema.get_all_of());
            model.set_any_of(schema.get_any_of());
            model.set_one_of(schema.get_one_of());
        }
        Ok(model)
    }

    /// `makeRefProperty(ref, property)`
    pub fn make_ref_property(&self, r: &str, property: &Schema) -> Schema {
        let n = ref_schema(r.to_string());
        if let Some(ext) = property.get_extensions() {
            for (k, v) in ext.entries() {
                n.add_extension(&k, v);
            }
        }
        n
    }
}

/// `fixStringModel(m)`
fn fix_string_model(m: &Schema) -> JResult<()> {
    if m.get_type().as_deref() == Some("string") && let Some(e) = m.get_example() {
        let example = e.to_java_string();
        let units: Vec<u16> = example.encode_utf16().collect();
        if units.is_empty() {
            return Err(sioobe(0, 1, 0));
        }
        if units[0] == b'"' as u16 && units[units.len() - 1] == b'"' as u16 {
            let inner = if units.len() >= 2 { String::from_utf16_lossy(&units[1..units.len() - 1]) } else { return Err(sioobe(1, 0, 1)) };
            m.set_example(JavaObject::string(inner));
        }
    }
    Ok(())
}
