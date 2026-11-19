//! Schema parsing of `OpenAPIDeserializer`: `getSchema` (3.0), `getJsonSchema` (3.1) and helpers.

use super::*;
use crate::models::schema::parse_offset_date_time;

impl OpenAPIDeserializer {
    /// `getSchemas(obj, location, result, underComponents)`: a map of named schemas; under `components` a name outside `^[a-zA-Z0-9\.\-_]+$` adds a warning, a non-object value is reported as an invalid type.
    pub fn get_schemas(&mut self, obj: Option<&JsonNode>, location: &str, result: &mut ParseResult, under_components: bool) -> JResult<JMap<Schema>> {
        let schemas = JMap::new();
        let Some(obj) = obj else { return Ok(schemas) };
        for name in get_keys(Some(obj)) {
            if under_components && !matches_component_name(&name) {
                result.warning(location, &format!("Schema name {name} doesn't adhere to regular expression ^[a-zA-Z0-9\\.\\-_]+$"));
            }
            let v = obj.get(&name).unwrap();
            if !v.is_object() {
                result.invalid_type(location, &name, "object", Some(v));
            } else if let Some(s) = self.get_schema(Some(v), &format!("{location}.{name}"), Some(result))? {
                schemas.put(name, s);
            }
        }
        Ok(schemas)
    }

    /// `getDiscriminator(node, location, result)`: a schema's `discriminator`; `propertyName` is required.
    pub fn get_discriminator(&mut self, node: &JsonNode, location: &str, result: &mut ParseResult) -> JResult<Discriminator> {
        let d = Discriminator::new();
        let v = self.get_string("propertyName", node, true, location, result);
        if result.allow(&v) {
            d.set_property_name(v);
        }
        if let Some(m) = self.get_object("mapping", node, false, location, result) {
            let mapping: JMap<String> = JMap::new();
            for key in get_keys(Some(&m)) {
                mapping.put(key.clone(), m.get(&key).unwrap().as_text());
            }
            d.set_mapping(mapping);
        }
        if result.openapi31 {
            for key in get_keys(Some(node)) {
                if key.starts_with("x-") {
                    let ext = self.get_extensions(node)?;
                    if !ext.is_empty() {
                        d.set_extensions(ext);
                    }
                }
            }
        }
        Ok(d)
    }

    /// `getSchema(jsonNode, location, result)` (`result` may be null: a lenient fresh one is used)
    pub fn get_schema(&mut self, json_node: Option<&JsonNode>, location: &str, result: Option<&mut ParseResult>) -> JResult<Option<Schema>> {
        let Some(json_node) = json_node else { return Ok(None) };
        let mut fresh;
        let result = match result {
            Some(r) => r,
            None => {
                fresh = ParseResult::new();
                fresh.allow_empty_strings = true;
                &mut fresh
            }
        };
        if result.openapi31 {
            return self.get_json_schema(Some(json_node), location, result);
        }
        if !json_node.is_object() {
            result.invalid_type(location, "", "object", Some(json_node));
            return Ok(None);
        }
        let node = json_node;
        let one_of = self.get_array("oneOf", node, false, location, result);
        let all_of = self.get_array("allOf", node, false, location, result);
        let any_of = self.get_array("anyOf", node, false, location, result);
        let items_node = self.get_object("items", node, false, location, result);
        let mut schema: Option<Schema> = None;
        if all_of.is_some() || any_of.is_some() || one_of.is_some() {
            let composed = Schema::composed_schema();
            if let Some(a) = &all_of {
                for n in a.as_array().unwrap().iter() {
                    if n.is_object() {
                        let s = self.get_schema(Some(n), location, Some(result))?;
                        composed.add_all_of_item_nullable(s);
                    }
                }
                schema = Some(composed.clone());
            }
            if let Some(a) = &any_of {
                for n in a.as_array().unwrap().iter() {
                    if n.is_object() {
                        let s = self.get_schema(Some(n), location, Some(result))?;
                        composed.add_any_of_item_nullable(s);
                    }
                }
                schema = Some(composed.clone());
            }
            if let Some(a) = &one_of {
                for n in a.as_array().unwrap().iter() {
                    if n.is_object() {
                        let s = self.get_schema(Some(n), location, Some(result))?;
                        composed.add_one_of_item_nullable(s);
                    }
                }
                schema = Some(composed.clone());
            }
        }
        if let Some(items) = &items_node {
            let s = if result.infer_schema_type { Schema::array_schema() } else { Schema::new() };
            // itemsNode is always an object here (getObject)
            s.set_items(self.get_schema(Some(items), location, Some(result))?);
            schema = Some(s);
        }
        let ap_bool = get_boolean("additionalProperties", node, false, location, result);
        let ap_obj = if ap_bool.is_none() { self.get_object("additionalProperties", node, false, location, result) } else { None };
        let additional: Option<AdditionalProperties> = match &ap_obj {
            Some(o) => self.get_schema(Some(o), location, Some(result))?.map(AdditionalProperties::Schema),
            None => ap_bool.map(AdditionalProperties::Bool),
        };
        if let Some(ap) = &additional {
            if result.infer_schema_type {
                if schema.is_none() {
                    schema = Some(if *ap == AdditionalProperties::Bool(false) { Schema::object_schema() } else { Schema::map_schema() });
                }
            } else if schema.is_none() {
                schema = Some(Schema::new());
            }
            schema.as_ref().unwrap().set_additional_properties(ap.clone());
        }
        let schema = match schema {
            Some(s) => s,
            None => create_schema_by_type(Some(node)),
        };

        if let Some(r) = node.get("$ref") {
            if let Some(text) = r.text_value() {
                if location.starts_with("paths") {
                    self.validate_component_ref(&r.as_text(), location, result);
                }
                match self.munged_ref(text) {
                    Some(m) => schema.set_ref(m),
                    None => schema.set_ref(r.as_text()),
                }
                let sref = schema.get_ref().unwrap_or_default();
                if sref.starts_with("#/components/schemas") && sref.matches('/').count() == 3 {
                    let ref_name = sref[sref.rfind('/').unwrap() + 1..].to_string();
                    self.local_schema_refs.put(ref_name, location.to_string());
                }
                if text.starts_with("#/components") && !text.starts_with("#/components/schemas") {
                    result.warning(location, &format!("$ref target {text} is not of expected type Schema"));
                }
                return Ok(Some(schema));
            }
            result.invalid_type(location, "$ref", "string", Some(node));
            return Ok(None);
        }

        self.get_common_schema_fields(node, location, result, &schema)?;
        if let Some(b) = get_boolean("exclusiveMaximum", node, false, location, result) {
            schema.set_exclusive_maximum(b);
        }
        if let Some(b) = get_boolean("exclusiveMinimum", node, false, location, result) {
            schema.set_exclusive_minimum(b);
        }
        if let Some(arr) = self.get_array("enum", node, false, location, result) {
            for n in arr.as_array().unwrap().iter() {
                if n.is_number() {
                    schema.add_enum_item_object(n.number_value().unwrap());
                } else if let JsonNode::Boolean(b) = n {
                    schema.add_enum_item_object(JavaObject::Boolean(*b));
                } else if n.is_value_node() {
                    let text = if n.is_null() { None } else { Some(n.as_text()) };
                    match get_decoded_object(&schema, text.as_deref()) {
                        Ok(o) => schema.add_enum_item_object(o.unwrap_or(JavaObject::Null)),
                        Err(msg) => result.invalid_type(location, &format!("enum=`{msg}`"), schema.get_format().as_deref().unwrap_or("null"), Some(n)),
                    }
                } else if n.is_container_node() {
                    schema.add_enum_item_object(JavaObject::Node(n.clone()));
                } else {
                    result.invalid_type(location, "enum", "value", Some(n));
                }
            }
        }
        let value = self.get_string("type", node, false, location, result);
        if is_blank(schema.get_type().as_deref()) {
            if result.allow(&value) {
                schema.set_type(value);
            } else if result.infer_schema_type && let Some(e) = node.get("enum") && e.is_array() {
                schema.set_type(infer_type_from_array(e));
            }
            if schema.get_type().as_deref() == Some("array") && schema.get_items().is_none() {
                result.missing(location, "items");
            }
        }
        if let Some(not_obj) = self.get_object("not", node, false, location, result)
            && let Some(not) = self.get_schema(Some(&not_obj), location, Some(result))?
        {
            schema.set_not(not);
        }
        let properties: JMap<Schema> = JMap::new();
        let props_obj = self.get_object("properties", node, false, location, result);
        for name in get_keys(props_obj.as_ref()) {
            let pv = props_obj.as_ref().unwrap().get(&name).unwrap();
            if !pv.is_object() {
                result.invalid_type(location, "properties", "object", Some(pv));
            } else if let Some(p) = self.get_schema(Some(pv), location, Some(result))? {
                properties.put(name, p);
            }
        }
        if props_obj.is_some() {
            schema.set_properties(properties);
        }
        if let Some(b) = get_boolean("nullable", node, false, location, result) {
            schema.set_nullable(b);
        }
        if let Some(def) = node.get("default") {
            if result.infer_schema_type {
                let nullable = schema.get_nullable().unwrap_or(true);
                let is_null = def.is_null();
                if !is_blank(schema.get_type().as_deref()) {
                    let ty = schema.get_type().unwrap();
                    if is_null {
                        if nullable {
                            schema.set_default(None::<JavaObject>);
                        } else {
                            result.invalid_type(location, "default", &format!("non-null {ty}"), Some(node));
                        }
                    } else if ty == "array" {
                        if let Some(a) = self.get_array("default", node, false, location, result) {
                            schema.set_default(JavaObject::Node(a));
                        }
                    } else if ty == "string" {
                        let v = self.get_string("default", node, false, location, result);
                        if result.allow(&v) {
                            match get_decoded_object(&schema, v.as_deref()) {
                                Ok(o) => schema.set_default(o),
                                Err(msg) => result.invalid_type(location, &format!("default=`{msg}`"), schema.get_format().as_deref().unwrap_or("null"), Some(node)),
                            }
                        }
                    } else if ty == "boolean" {
                        if let Some(b) = get_boolean("default", node, false, location, result) {
                            schema.set_default(JavaObject::Boolean(b));
                        }
                    } else if ty == "object" {
                        if let Some(o) = self.get_object("default", node, false, location, result) {
                            schema.set_default(JavaObject::Node(o));
                        }
                    } else if ty == "integer" {
                        if let Some(i) = get_integer("default", node, false, location, result) {
                            schema.set_default(JavaObject::Integer(i));
                        }
                    } else if ty == "number"
                        && let Some(b) = get_big_decimal("default", node, false, location, result)
                    {
                        schema.set_default(JavaObject::BigDecimal(b.into()));
                    }
                } else if let Some(d) = self.get_any_type("default", node, location, result) {
                    schema.set_default(d);
                }
            } else if let Some(d) = self.get_any_type("default", node, location, result) {
                schema.set_default(d);
            }
        }
        let ext = self.get_extensions(node)?;
        if !ext.is_empty() {
            schema.set_extensions(ext);
        }
        for key in get_keys(Some(node)) {
            if !SCHEMA_KEYS.contains(&key.as_str()) && !key.starts_with("x-") {
                result.extra(location, &key, node.get(&key));
            }
        }
        Ok(Some(schema))
    }

    /// The `location.startsWith("paths")` component-existence check (`ReferenceValidator`).
    fn validate_component_ref(&mut self, r: &str, location: &str, result: &mut ParseResult) {
        let components: Vec<&str> = java_split(r, "#/components");
        if r.starts_with("#/components") && components.len() > 1 {
            let child: Vec<&str> = java_split(components[1], "/");
            let new_child = if child.len() > 1 { &child[1..] } else { &[][..] };
            let ok = (|| -> Option<bool> {
                let kind = new_child.first()?;
                let name = new_child.get(1)?;
                let c = self.components.as_ref()?;
                let has = match *kind {
                    "schemas" => c.get_schemas()?.contains_key(name),
                    "responses" => c.get_responses()?.contains_key(name),
                    "parameters" => c.get_parameters()?.contains_key(name),
                    "examples" => c.get_examples()?.contains_key(name),
                    "requestBodies" => c.get_request_bodies()?.contains_key(name),
                    "headers" => c.get_headers()?.contains_key(name),
                    "securitySchemes" => c.get_security_schemes()?.contains_key(name),
                    "links" => c.get_links()?.contains_key(name),
                    "callbacks" => c.get_callbacks()?.contains_key(name),
                    _ => return None,
                };
                Some(has)
            })();
            if ok != Some(true) {
                result.missing(location, r);
            }
        }
    }

    pub(crate) fn get_common_schema_fields(&mut self, node: &JsonNode, location: &str, result: &mut ParseResult, schema: &Schema) -> JResult<()> {
        let v = self.get_string("title", node, false, location, result);
        if result.allow(&v) {
            schema.set_title(v);
        }
        if let Some(d) = self.get_object("discriminator", node, false, location, result) {
            schema.set_discriminator(self.get_discriminator(&d, location, result)?);
        }
        if let Some(b) = get_big_decimal("multipleOf", node, false, location, result) {
            if b.signum() > 0 {
                schema.set_multiple_of(b);
            } else {
                result.warning(location, "multipleOf value must be > 0");
            }
        }
        if let Some(b) = get_big_decimal("maximum", node, false, location, result) {
            schema.set_maximum(b);
        }
        if let Some(b) = get_big_decimal("minimum", node, false, location, result) {
            schema.set_minimum(b);
        }
        if let Some(i) = get_integer("minLength", node, false, location, result) {
            schema.set_min_length(i);
        }
        if let Some(i) = get_integer("maxLength", node, false, location, result) {
            schema.set_max_length(i);
        }
        let pattern = self.get_string("pattern", node, false, location, result);
        if result.allow_empty_strings && pattern.is_some() {
            schema.set_pattern(pattern);
        }
        if let Some(i) = get_integer("maxItems", node, false, location, result) {
            schema.set_max_items(i);
        }
        if let Some(i) = get_integer("minItems", node, false, location, result) {
            schema.set_min_items(i);
        }
        if let Some(b) = get_boolean("uniqueItems", node, false, location, result) {
            schema.set_unique_items(b);
        }
        if let Some(i) = get_integer("maxProperties", node, false, location, result) {
            schema.set_max_properties(i);
        }
        if let Some(i) = get_integer("minProperties", node, false, location, result) {
            schema.set_min_properties(i);
        }
        if let Some(req) = self.get_array("required", node, false, location, result) {
            let mut list = Vec::new();
            for n in req.as_array().unwrap().iter() {
                match n.text_value() {
                    Some(t) => list.push(t.to_string()),
                    None => result.invalid_type(location, "required", "string", Some(n)),
                }
            }
            if !list.is_empty() {
                schema.set_required(JList::from_vec(list));
            }
        }
        let v = self.get_string("description", node, false, location, result);
        if result.allow(&v) {
            schema.set_description(v);
        }
        let v = self.get_string("format", node, false, location, result);
        if result.allow(&v) {
            schema.set_format(v);
        }
        if let Some(b) = get_boolean("readOnly", node, false, location, result) {
            schema.set_read_only(b);
        }
        if let Some(b) = get_boolean("writeOnly", node, false, location, result) {
            schema.set_write_only(b);
        }
        let both = get_boolean("writeOnly", node, false, location, result).unwrap_or(false) && get_boolean("readOnly", node, false, location, result).unwrap_or(false);
        if both {
            result.warning(location, " writeOnly and readOnly are both present");
        }
        if let Some(x) = self.get_object("xml", node, false, location, result)
            && let Some(xml) = self.get_xml(Some(&x), location, result)?
        {
            schema.set_xml(xml);
        }
        if let Some(e) = self.get_object("externalDocs", node, false, location, result)
            && let Some(docs) = self.get_external_docs(Some(&e), location, result)
        {
            schema.set_external_docs(docs);
        }
        if let Some(example) = self.get_any_type("example", node, location, result) {
            schema.set_example(if is_null_node(&example) { None } else { Some(example) });
        }
        if let Some(b) = get_boolean("deprecated", node, false, location, result) {
            schema.set_deprecated(b);
        }
        Ok(())
    }

    /// `getJsonSchema` (OpenAPI 3.1)
    pub fn get_json_schema(&mut self, json_node: Option<&JsonNode>, location: &str, result: &mut ParseResult) -> JResult<Option<Schema>> {
        let Some(json_node) = json_node else { return Ok(None) };
        let mut bool_value = None;
        if let JsonNode::Boolean(b) = json_node {
            bool_value = Some(*b);
        } else if let Some(s) = json_node.text_value() {
            if s.eq_ignore_ascii_case("true") || s.eq_ignore_ascii_case("false") {
                bool_value = Some(s.eq_ignore_ascii_case("true"));
            } else {
                result.invalid_type(location, "", "object", Some(json_node));
                return Ok(None);
            }
        }
        if let Some(b) = bool_value {
            return Ok(Some(Schema::json_schema().boolean_schema_value(b)));
        }
        if !json_node.is_object() {
            result.invalid_type(location, "", "object", Some(json_node));
            return Ok(None);
        }
        let node = json_node;
        let one_of = self.get_array("oneOf", node, false, location, result);
        let all_of = self.get_array("allOf", node, false, location, result);
        let any_of = self.get_array("anyOf", node, false, location, result);
        let items_node = self.get_object_or_boolean("items", node, false, location, result);
        let mut schema: Option<Schema> = None;
        if all_of.is_some() || any_of.is_some() || one_of.is_some() {
            let composed = Schema::json_schema();
            if let Some(a) = &all_of {
                for n in a.as_array().unwrap().iter() {
                    let s = self.get_json_schema(Some(n), location, result)?;
                    composed.add_all_of_item_nullable(s);
                }
                schema = Some(composed.clone());
            }
            if let Some(a) = &any_of {
                for n in a.as_array().unwrap().iter() {
                    let s = self.get_json_schema(Some(n), location, result)?;
                    composed.add_any_of_item_nullable(s);
                }
                schema = Some(composed.clone());
            }
            if let Some(a) = &one_of {
                for n in a.as_array().unwrap().iter() {
                    let s = self.get_json_schema(Some(n), location, result)?;
                    composed.add_one_of_item_nullable(s);
                }
                schema = Some(composed.clone());
            }
        }
        if let Some(items) = &items_node {
            let s = Schema::json_schema();
            s.set_items(self.get_json_schema(Some(items), location, result)?);
            schema = Some(s);
        }
        if let Some(ap) = self.get_object_or_boolean("additionalProperties", node, false, location, result) {
            let aps = self.get_json_schema(Some(&ap), location, result)?;
            let s = schema.get_or_insert_with(Schema::json_schema);
            s.set_additional_properties(aps.map(AdditionalProperties::Schema));
        }
        if let Some(up) = self.get_object_or_boolean("unevaluatedProperties", node, false, location, result) {
            let ups = self.get_json_schema(Some(&up), location, result)?;
            let s = schema.get_or_insert_with(Schema::json_schema);
            s.set_unevaluated_properties(ups);
        }
        let schema = schema.unwrap_or_else(Schema::json_schema);
        if let Some(r) = node.get("$ref") {
            if let Some(text) = r.text_value() {
                if location.starts_with("paths") {
                    self.validate_component_ref(&r.as_text(), location, result);
                }
                match self.munged_ref(text) {
                    Some(m) => schema.set_ref(m),
                    None => schema.set_ref(r.as_text()),
                }
                if text.starts_with("#/components") && !text.starts_with("#/components/schemas") {
                    result.warning(location, &format!("$ref target {text} is not of expected type Schema"));
                }
            } else {
                result.invalid_type(location, "$ref", "string", Some(node));
            }
        }
        self.get_common_schema_fields(node, location, result, &schema)?;
        if node.get("default").is_some() {
            schema.set_default(self.get_any_type("default", node, location, result));
        }
        if let Some(b) = get_big_decimal("exclusiveMaximum", node, false, location, result) {
            schema.set_exclusive_maximum_value(b);
        }
        if let Some(b) = get_big_decimal("exclusiveMinimum", node, false, location, result) {
            schema.set_exclusive_minimum_value(b);
        }
        if let Some(i) = get_integer("minContains", node, false, location, result) {
            schema.set_min_contains(i);
        }
        if let Some(i) = get_integer("maxContains", node, false, location, result) {
            schema.set_max_contains(i);
        }
        let type_string = self.get_string_full("type", node, false, location, result, None, true, false);
        let type_array = self.get_array_full("type", node, false, location, result, true);
        if result.allow(&type_string) {
            schema.add_type(type_string.as_deref().unwrap());
        } else if let Some(ta) = &type_array {
            for n in ta.as_array().unwrap().iter() {
                if n.is_value_node() {
                    let t = n.as_text();
                    if !["null", "boolean", "object", "array", "number", "string", "integer"].contains(&t.as_str()) {
                        result.warning(location, &format!(" invalid type {t}"));
                    }
                    if !schema.add_type(&t) {
                        result.warning(location, &format!(" duplicated type {t}"));
                    }
                } else {
                    result.invalid_type(location, "type", "value", Some(n));
                }
            }
        } else if let Some(e) = node.get("enum") && e.is_array() {
            schema.add_type(infer_type_from_array(e));
        }
        if let Some(arr) = self.get_array("enum", node, false, location, result) {
            for n in arr.as_array().unwrap().iter() {
                if n.is_number() {
                    schema.add_enum_item_object(n.number_value().unwrap());
                } else if let JsonNode::Boolean(b) = n {
                    schema.add_enum_item_object(JavaObject::Boolean(*b));
                } else if n.is_value_node() {
                    let text = if n.is_null() { None } else { Some(n.as_text()) };
                    match get_decoded_object31(&schema, text.as_deref()) {
                        Ok(o) => schema.add_enum_item_object(o.unwrap_or(JavaObject::Null)),
                        Err(msg) => result.invalid_type(location, &format!("enum=`{msg}`"), schema.get_format().as_deref().unwrap_or("null"), Some(n)),
                    }
                } else if n.is_container_node() {
                    schema.add_enum_item_object(JavaObject::Node(n.clone()));
                } else {
                    result.invalid_type(location, "enum", "value", Some(n));
                }
            }
        }
        for (key, setter) in [
            ("not", Schema::set_not as fn(&Schema, Option<Schema>)),
            ("contentSchema", Schema::set_content_schema_),
            ("propertyNames", Schema::set_property_names_),
            ("if", Schema::set_if_),
            ("then", Schema::set_then_),
            ("else", Schema::set_else_),
            ("unevaluatedItems", Schema::set_unevaluated_items_),
        ] {
            if let Some(o) = self.get_object_or_boolean(key, node, false, location, result)
                && let Some(s) = self.get_json_schema(Some(&o), location, result)?
            {
                setter(&schema, Some(s));
            }
        }
        // dependentRequired (note: one shared list for all entries, as in Java)
        let dependent_required_list: JMap<JList<String>> = JMap::new();
        let dr_obj = self.get_object("dependentRequired", node, false, location, result);
        let dependent_required: JList<String> = JList::new();
        for name in get_keys(dr_obj.as_ref()) {
            let v = dr_obj.as_ref().unwrap().get(&name).unwrap();
            if !v.is_array() {
                result.invalid_type(location, "dependentRequired", "object", Some(v));
            } else {
                for n in v.as_array().unwrap().iter() {
                    if let Some(t) = n.text_value() {
                        dependent_required.add(t.to_string());
                    }
                }
                dependent_required_list.put(name, dependent_required.clone());
            }
        }
        if dr_obj.is_some() {
            schema.set_dependent_required(dependent_required_list);
        }
        if let Some(ds) = self.get_object("dependentSchemas", node, false, location, result) {
            let list: JMap<Schema> = JMap::new();
            for name in get_keys(Some(&ds)) {
                if let Some(s) = self.get_json_schema(ds.get(&name), location, result)? {
                    list.put(name, s);
                }
            }
            schema.set_dependent_schemas(list);
        }
        if let Some(pa) = self.get_array("prefixItems", node, false, location, result) {
            let mut list = Vec::new();
            for n in pa.as_array().unwrap().iter() {
                if let Some(s) = self.get_json_schema(Some(n), location, result)? {
                    list.push(s);
                }
            }
            if !list.is_empty() {
                schema.set_prefix_items(JList::from_vec(list));
            }
        }
        if let Some(c) = self.get_object_or_boolean("contains", node, false, location, result)
            && let Some(s) = self.get_json_schema(Some(&c), location, result)?
        {
            schema.set_contains(s);
        }
        let properties: JMap<Schema> = JMap::new();
        let po = self.get_object("properties", node, false, location, result);
        for name in get_keys(po.as_ref()) {
            if let Some(p) = self.get_json_schema(po.as_ref().unwrap().get(&name), location, result)? {
                properties.put(name, p);
            }
        }
        if po.is_some() {
            schema.set_properties(properties);
        }
        let pattern_properties: JMap<Schema> = JMap::new();
        let ppo = self.get_object("patternProperties", node, false, location, result);
        for name in get_keys(ppo.as_ref()) {
            if let Some(p) = self.get_json_schema(ppo.as_ref().unwrap().get(&name), location, result)? {
                pattern_properties.put(name, p);
            }
        }
        if ppo.is_some() {
            schema.set_pattern_properties(pattern_properties);
        }
        if let Some(c) = self.get_any_type("const", node, location, result) {
            schema.set_const(c);
        }
        if let Some(v) = self.get_string("contentEncoding", node, false, location, result) {
            schema.set_content_encoding(v);
        }
        if let Some(v) = self.get_string("contentMediaType", node, false, location, result) {
            schema.set_content_media_type(v);
        }
        if let Some(ex) = self.get_array("examples", node, false, location, result) {
            let list: Vec<JavaObject> = ex.as_array().unwrap().iter().map(|n| JavaObject::Node(n.clone())).collect();
            if !list.is_empty() {
                schema.set_examples(JList::from_vec(list));
            }
        }
        for (key, setter) in [
            ("$anchor", Schema::set_anchor as fn(&Schema, Option<String>)),
            ("$vocabulary", Schema::set_vocabulary_),
            ("$dynamicAnchor", Schema::set_dynamic_anchor_),
            ("$dynamicRef", Schema::set_dynamic_ref_),
            ("$id", Schema::set_id_),
            ("$schema", Schema::set_schema_),
            ("$comment", Schema::set_comment_),
        ] {
            if let Some(v) = self.get_string(key, node, false, location, result) {
                setter(&schema, Some(v));
            }
        }
        let extensions = self.get_extensions(node)?;
        if !extensions.is_empty() {
            schema.set_extensions(extensions.clone());
        }
        for key in get_keys(Some(node)) {
            validate_reserved_keywords(&key, location, result);
            if !SCHEMA_KEYS_31.contains(&key.as_str()) && !key.starts_with("x-") {
                extensions.put(key.clone(), node_to_object(node.get(&key).unwrap()));
                schema.set_extensions(extensions.clone());
            }
        }
        Ok(Some(schema))
    }
}

/// `SchemaTypeUtil.createSchemaByType`
pub fn create_schema_by_type(node: Option<&JsonNode>) -> Schema {
    let Some(node) = node else { return Schema::new() };
    let ty = node.get("type").and_then(|n| n.text_value()).map(str::to_string);
    if is_blank(ty.as_deref()) {
        return Schema::new();
    }
    let format = node.get("format").and_then(|n| n.text_value()).map(str::to_string);
    create_schema(ty.as_deref().unwrap(), format.as_deref())
}

/// `SchemaTypeUtil.BINARY_AS_STRING`
pub const BINARY_AS_STRING: &str = "swaggerParserBinaryAsString";

/// `System.getProperty(BINARY_AS_STRING) != null || System.getenv(BINARY_AS_STRING) != null`
fn binary_as_string() -> bool {
    crate::java::system::get_property(BINARY_AS_STRING).is_some() || std::env::var_os(BINARY_AS_STRING).is_some()
}

/// `SchemaTypeUtil.createSchema(type, format)`
pub fn create_schema(ty: &str, format: Option<&str>) -> Schema {
    let blank = is_blank(format);
    match ty {
        "integer" => {
            let s = Schema::integer_schema();
            s.set_format(if blank { None } else { format.map(str::to_string) });
            s
        }
        "number" => {
            let s = Schema::number_schema();
            if !blank {
                s.set_format(format);
            }
            s
        }
        "boolean" => {
            let s = Schema::boolean_schema();
            if !blank {
                s.set_format(format);
            }
            s
        }
        "string" => match format {
            Some("byte") if binary_as_string() => Schema::string_schema().format("byte"),
            Some("byte") => Schema::of(SchemaKind::ByteArray),
            Some("binary") if binary_as_string() => Schema::string_schema().format("binary"),
            Some("binary") => Schema::of(SchemaKind::Binary),
            Some("date") => Schema::of(SchemaKind::Date),
            Some("date-time") => Schema::of(SchemaKind::DateTime),
            Some("password") => Schema::of(SchemaKind::Password),
            Some("email") => Schema::of(SchemaKind::Email),
            Some("uuid") => Schema::of(SchemaKind::Uuid),
            _ => {
                let s = Schema::string_schema();
                s.set_format(if blank { None } else { format.map(str::to_string) });
                s
            }
        },
        "object" => Schema::object_schema(),
        _ => Schema::new(),
    }
}

/// `inferTypeFromArray` (note: always inspects element 0, as the Java code does)
pub fn infer_type_from_array(an: &JsonNode) -> &'static str {
    let arr = an.as_array().unwrap();
    if arr.is_empty() {
        return "string";
    }
    let mut ty: Option<&'static str> = None;
    for _ in 0..arr.len() {
        let e = &arr[0];
        if e.is_boolean() {
            ty = Some(if ty.is_none() || ty == Some("boolean") { "boolean" } else { "string" });
        } else if e.is_number() {
            ty = Some(if ty.is_none() || ty == Some("number") { "number" } else { "string" });
        } else {
            ty = Some("string");
        }
    }
    ty.unwrap()
}

/// `getDecodedObject` (3.0): by schema class. Err carries the `ParseException` message.
fn get_decoded_object(schema: &Schema, s: Option<&str>) -> Result<Option<JavaObject>, String> {
    let Some(s) = s else { return Ok(None) };
    let obj = match schema.kind() {
        SchemaKind::Date => to_date(s),
        SchemaKind::DateTime => parse_offset_date_time(s).map(JavaObject::OffsetDateTime),
        SchemaKind::ByteArray => to_bytes(s),
        _ => Some(JavaObject::string(s)),
    };
    obj.map(Some).ok_or_else(|| s.to_string())
}

/// `getDecodedObject31`: by type/format strings.
fn get_decoded_object31(schema: &Schema, s: Option<&str>) -> Result<Option<JavaObject>, String> {
    let Some(s) = s else { return Ok(None) };
    let is_string = schema.get_type().as_deref() == Some("string");
    let obj = match (is_string, schema.get_format().as_deref()) {
        (true, Some("date")) => to_date(s),
        (true, Some("date-time")) => parse_offset_date_time(s).map(JavaObject::OffsetDateTime),
        (true, Some("byte")) => to_bytes(s),
        _ => Some(JavaObject::string(s)),
    };
    obj.map(Some).ok_or_else(|| s.to_string())
}

/// `toDate`: RFC 3339 full-date, built with `new Calendar.Builder().setDate(y, m-1, d)` in the JVM default
/// time zone (UTC is assumed here) and lenient field handling.
fn to_date(s: &str) -> Option<JavaObject> {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' || !s[0..4].bytes().chain(s[5..7].bytes()).chain(s[8..10].bytes()).all(|c| c.is_ascii_digit()) {
        return None;
    }
    crate::models::schema::parse_date_utc(s).map(JavaObject::Date)
}

/// `toBytes`: `Base64.getDecoder().decode` (basic, strict alphabet, padding optional)
fn to_bytes(s: &str) -> Option<JavaObject> {
    use base64::Engine;
    let engine = base64::engine::GeneralPurpose::new(
        &base64::alphabet::STANDARD,
        base64::engine::GeneralPurposeConfig::new().with_decode_padding_mode(base64::engine::DecodePaddingMode::Indifferent),
    );
    engine.decode(s).ok().map(|b| JavaObject::Bytes(b.into()))
}

/// `String.split(regex)` for a literal separator (trailing empty strings removed).
pub(crate) fn java_split<'a>(s: &'a str, sep: &str) -> Vec<&'a str> {
    let mut parts: Vec<&str> = s.split(sep).collect();
    while parts.len() > 1 && parts.last() == Some(&"") {
        parts.pop();
    }
    if parts.len() == 1 && parts[0].is_empty() && !s.is_empty() {
        return Vec::new();
    }
    parts
}

impl Schema {
    pub(crate) fn add_all_of_item_nullable(&self, s: Option<Schema>) {
        match s {
            Some(s) => {
                self.add_all_of_item(s);
            }
            None => {
                // a null list element: represented by leaving the list created but not adding
                self.write().all_of.get_or_insert_with(JList::new);
            }
        }
    }
    pub(crate) fn add_any_of_item_nullable(&self, s: Option<Schema>) {
        match s {
            Some(s) => {
                self.add_any_of_item(s);
            }
            None => {
                self.write().any_of.get_or_insert_with(JList::new);
            }
        }
    }
    pub(crate) fn add_one_of_item_nullable(&self, s: Option<Schema>) {
        match s {
            Some(s) => {
                self.add_one_of_item(s);
            }
            None => {
                self.write().one_of.get_or_insert_with(JList::new);
            }
        }
    }
    pub(crate) fn set_content_schema_(&self, s: Option<Schema>) {
        self.set_content_schema(s)
    }
    pub(crate) fn set_property_names_(&self, s: Option<Schema>) {
        self.set_property_names(s)
    }
    pub(crate) fn set_if_(&self, s: Option<Schema>) {
        self.set_if(s)
    }
    pub(crate) fn set_then_(&self, s: Option<Schema>) {
        self.set_then(s)
    }
    pub(crate) fn set_else_(&self, s: Option<Schema>) {
        self.set_else(s)
    }
    pub(crate) fn set_unevaluated_items_(&self, s: Option<Schema>) {
        self.set_unevaluated_items(s)
    }
    pub(crate) fn set_vocabulary_(&self, s: Option<String>) {
        self.set_vocabulary(s)
    }
    pub(crate) fn set_dynamic_anchor_(&self, s: Option<String>) {
        self.set_dynamic_anchor(s)
    }
    pub(crate) fn set_dynamic_ref_(&self, s: Option<String>) {
        self.set_dynamic_ref(s)
    }
    pub(crate) fn set_id_(&self, s: Option<String>) {
        self.set_id(s)
    }
    pub(crate) fn set_schema_(&self, s: Option<String>) {
        self.set_schema(s)
    }
    pub(crate) fn set_comment_(&self, s: Option<String>) {
        self.set_comment(s)
    }
}
