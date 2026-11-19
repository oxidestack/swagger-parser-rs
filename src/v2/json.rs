//! Jackson bean serialization of the Swagger 2.0 models by swagger-core 2.x's `Json.mapper()`
//! (`NON_NULL`, the v1 classes' own property order, any-getter vendor extensions last). The converter's
//! `Json.mapper().convertValue(v2Object, Schema.class)` is [`property_to_tree`] / [`model_to_tree`] followed
//! by the v3 deserializer.

use super::models::*;
use jackson::convert::{write_object, SerConfig, TreeGenerator};
use jackson::generator::JsonGenerator;
use jackson::{BigDecimal, JacksonError, JavaObject, JsonNode};

type R = Result<(), JacksonError>;

struct W<'a> {
    g: &'a mut dyn JsonGenerator,
    cfg: SerConfig<'static>,
    /// swagger-core 1.x `Json.mapper()` (computed `$ref` only, boolean schemas as booleans) instead of
    /// swagger-core 2.x's
    v1: bool,
}

impl W<'_> {
    fn string(&mut self, name: &str, v: &Option<String>) -> R {
        if let Some(v) = v {
            self.g.write_field_name(name)?;
            self.g.write_string(v)?;
        }
        Ok(())
    }

    fn boolean(&mut self, name: &str, v: Option<bool>) -> R {
        if let Some(v) = v {
            self.g.write_field_name(name)?;
            self.g.write_boolean(v)?;
        }
        Ok(())
    }

    fn int(&mut self, name: &str, v: Option<i32>) -> R {
        if let Some(v) = v {
            self.g.write_field_name(name)?;
            self.g.write_number_i32(v)?;
        }
        Ok(())
    }

    fn decimal(&mut self, name: &str, v: &Option<BigDecimal>) -> R {
        if let Some(v) = v {
            self.g.write_field_name(name)?;
            self.g.write_number_big_decimal(v)?;
        }
        Ok(())
    }

    fn object(&mut self, name: &str, v: &Option<JavaObject>) -> R {
        if let Some(v) = v {
            if v.is_null() {
                return Ok(());
            }
            self.g.write_field_name(name)?;
            write_object(self.g, v, &self.cfg)?;
        }
        Ok(())
    }

    fn strings(&mut self, name: &str, v: &Option<Vec<String>>) -> R {
        if let Some(v) = v {
            self.g.write_field_name(name)?;
            self.g.write_start_array()?;
            for s in v {
                self.g.write_string(s)?;
            }
            self.g.write_end_array()?;
        }
        Ok(())
    }

    fn objects(&mut self, name: &str, v: &Option<Vec<JavaObject>>) -> R {
        if let Some(v) = v {
            self.g.write_field_name(name)?;
            self.g.write_start_array()?;
            for s in v {
                write_object(self.g, s, &self.cfg)?;
            }
            self.g.write_end_array()?;
        }
        Ok(())
    }

    fn xml(&mut self, v: &Option<Xml>) -> R {
        if let Some(x) = v {
            self.g.write_field_name("xml")?;
            self.g.write_start_object()?;
            self.string("name", &x.name)?;
            self.string("namespace", &x.namespace)?;
            self.string("prefix", &x.prefix)?;
            self.boolean("attribute", x.attribute)?;
            self.boolean("wrapped", x.wrapped)?;
            self.g.write_end_object()?;
        }
        Ok(())
    }

    fn external_docs(&mut self, v: &Option<ExternalDocs>) -> R {
        if let Some(d) = v {
            self.g.write_field_name("externalDocs")?;
            self.g.write_start_object()?;
            self.string("description", &d.description)?;
            self.string("url", &d.url)?;
            self.any(&d.vendor_extensions)?;
            self.g.write_end_object()?;
        }
        Ok(())
    }

    /// `@JsonAnyGetter` map (null values skipped)
    fn any(&mut self, m: &VendorExtensions) -> R {
        for (k, v) in m.entries() {
            if v.is_null() {
                continue;
            }
            self.g.write_field_name(&k)?;
            write_object(self.g, &v, &self.cfg)?;
        }
        Ok(())
    }

    fn property_field(&mut self, name: &str, p: &Option<PropertyRef>) -> R {
        if let Some(p) = p {
            self.g.write_field_name(name)?;
            write_property_with(self.g, &p.borrow(), self.v1)?;
        }
        Ok(())
    }

    fn properties(&mut self, name: &str, m: &Option<crate::java::JMap<PropertyRef>>) -> R {
        if let Some(m) = m {
            self.g.write_field_name(name)?;
            self.g.write_start_object()?;
            for (k, v) in m.entries() {
                self.g.write_field_name(&k)?;
                write_property_with(self.g, &v.borrow(), self.v1)?;
            }
            self.g.write_end_object()?;
        }
        Ok(())
    }
}

/// `Json.mapper()` serialization of a v1 `Property` (plain `BeanSerializer`).
pub fn write_property(g: &mut dyn JsonGenerator, p: &Property) -> R {
    write_property_with(g, p, false)
}

/// A v1 `Property` through swagger-core 2.x's (`v1 = false`) or 1.x's (`v1 = true`) `Json.mapper()`.
pub fn write_property_with(g: &mut dyn JsonGenerator, p: &Property, v1: bool) -> R {
    use PropertyKind as K;
    if v1 && let Some(b) = p.boolean_value {
        return g.write_boolean(b);
    }
    let mut w = W { g, cfg: SerConfig::swagger(), v1 };
    w.g.write_start_object()?;
    if p.kind != K::Ref {
        w.string("type", &p.type_)?;
    }
    w.string("format", &p.format)?;
    w.object("example", &p.example)?;
    w.xml(&p.xml)?;
    w.int("position", p.position)?;
    w.string("description", &p.description)?;
    w.string("title", &p.title)?;
    w.boolean("readOnly", p.read_only)?;
    w.boolean("allowEmptyValue", p.allow_empty_value)?;
    match p.kind {
        K::Array => {
            w.boolean("uniqueItems", p.unique_items)?;
            w.property_field("items", &p.items)?;
            w.int("maxItems", p.max_items)?;
            w.int("minItems", p.min_items)?;
        }
        K::BaseInteger | K::Decimal | K::Integer | K::Long | K::Double | K::Float => {
            w.decimal("minimum", &p.minimum)?;
            w.decimal("maximum", &p.maximum)?;
            w.decimal("multipleOf", &p.multiple_of)?;
            w.boolean("exclusiveMinimum", p.exclusive_minimum)?;
            w.boolean("exclusiveMaximum", p.exclusive_maximum)?;
            if matches!(p.kind, K::Integer | K::Long | K::Double | K::Float) {
                w.objects("enum", &p.enum_values)?;
                w.object("default", &p.default_value)?;
            }
        }
        K::Binary | K::Email | K::Password | K::String | K::Uuid => {
            w.int("minLength", p.min_length)?;
            w.int("maxLength", p.max_length)?;
            w.string("pattern", &p.pattern)?;
            w.strings("enum", &p.enum_strings)?;
            w.string("default", &p.default_string)?;
        }
        K::ByteArray => {
            w.int("minLength", p.min_length)?;
            w.int("maxLength", p.max_length)?;
            w.string("pattern", &p.pattern)?;
            w.string("default", &p.default_string)?;
            w.strings("enum", &p.enum_strings)?;
        }
        K::Boolean => {
            w.objects("enum", &p.enum_values)?;
            w.object("default", &p.default_value)?;
        }
        K::Composed => {
            if let Some(all_of) = &p.all_of {
                w.g.write_field_name("allOf")?;
                w.g.write_start_array()?;
                for item in all_of {
                    match item {
                        Some(i) => write_property_with(w.g, &i.borrow(), v1)?,
                        None => w.g.write_null()?,
                    }
                }
                w.g.write_end_array()?;
            }
        }
        K::Date | K::DateTime => w.strings("enum", &p.enum_strings)?,
        K::Map => {
            w.int("minProperties", p.min_properties)?;
            w.int("maxProperties", p.max_properties)?;
            w.property_field("additionalProperties", &p.additional_properties)?;
        }
        K::Object => {
            w.properties("properties", &p.properties)?;
            w.strings("required", &p.required_properties())?;
        }
        K::Ref => {
            w.string("$ref", &p.get_ref())?;
            if !v1 {
                w.string("originalRef", &p.original_ref())?;
            }
        }
        K::BooleanValue | K::File | K::Untyped => {}
    }
    w.any(&p.vendor_extensions)?;
    w.g.write_end_object()
}

/// `Json.mapper()` serialization of a v1 `Model` (`ModelImpl`, `ArrayModel` or `RefModel`).
pub fn write_model(g: &mut dyn JsonGenerator, m: &Model) -> R {
    write_model_with(g, m, false)
}

/// A v1 `Model` through swagger-core 2.x's (`v1 = false`) or 1.x's (`v1 = true`) `Json.mapper()`.
pub fn write_model_with(g: &mut dyn JsonGenerator, m: &Model, v1: bool) -> R {
    if v1 && let Some(b) = m.get_boolean_value() {
        return g.write_boolean(b);
    }
    let mut w = W { g, cfg: SerConfig::swagger(), v1 };
    w.g.write_start_object()?;
    match m.kind {
        ModelKind::Ref => {
            w.string("$ref", &m.get_ref())?;
            if !v1 {
                w.string("originalRef", &m.generic_ref.as_ref().map(|g| g.original_ref.clone()))?;
            }
        }
        ModelKind::Impl => {
            w.string("type", &m.type_)?;
            w.strings("required", &m.get_required())?;
            w.string("discriminator", &m.discriminator)?;
            w.properties("properties", &m.properties)?;
            w.external_docs(&m.external_docs)?;
            w.string("title", &m.title)?;
            w.xml(&m.xml)?;
            w.decimal("minimum", &m.minimum)?;
            w.decimal("maximum", &m.maximum)?;
            w.decimal("multipleOf", &m.multiple_of)?;
            w.boolean("exclusiveMinimum", m.exclusive_minimum)?;
            w.boolean("exclusiveMaximum", m.exclusive_maximum)?;
            w.int("minLength", m.min_length)?;
            w.int("maxLength", m.max_length)?;
            w.string("pattern", &m.pattern)?;
            w.string("format", &m.format)?;
            w.boolean("allowEmptyValue", m.allow_empty_value)?;
            w.boolean("uniqueItems", m.unique_items)?;
            w.string("description", &m.description)?;
            w.object("example", &m.example)?;
            w.property_field("additionalProperties", &m.additional_properties)?;
            w.strings("enum", &m.enum_)?;
            w.object("default", &m.get_default_value())?;
            w.any(&m.vendor_extensions)?;
        }
        ModelKind::Array => {
            w.external_docs(&m.external_docs)?;
            w.string("title", &m.title)?;
            w.xml(&m.xml)?;
            w.decimal("minimum", &m.minimum)?;
            w.decimal("maximum", &m.maximum)?;
            w.decimal("multipleOf", &m.multiple_of)?;
            w.boolean("exclusiveMinimum", m.exclusive_minimum)?;
            w.boolean("exclusiveMaximum", m.exclusive_maximum)?;
            w.int("minLength", m.min_length)?;
            w.int("maxLength", m.max_length)?;
            w.string("pattern", &m.pattern)?;
            w.properties("properties", &m.properties)?;
            w.strings("required", &m.get_required())?;
            w.string("type", &m.type_)?;
            w.string("description", &m.description)?;
            w.property_field("items", &m.items)?;
            w.boolean("uniqueItems", m.unique_items)?;
            w.object("example", &m.example)?;
            w.int("minItems", m.min_items)?;
            w.int("maxItems", m.max_items)?;
            w.any(&m.vendor_extensions)?;
        }
        ModelKind::Composed | ModelKind::BooleanValue => {
            // [externalDocs, title, xml, minimum, …, properties, required, (allOf,) description, example, ext]
            w.external_docs(&m.external_docs)?;
            w.string("title", &m.title)?;
            w.xml(&m.xml)?;
            w.decimal("minimum", &m.minimum)?;
            w.decimal("maximum", &m.maximum)?;
            w.decimal("multipleOf", &m.multiple_of)?;
            w.boolean("exclusiveMinimum", m.exclusive_minimum)?;
            w.boolean("exclusiveMaximum", m.exclusive_maximum)?;
            w.int("minLength", m.min_length)?;
            w.int("maxLength", m.max_length)?;
            w.string("pattern", &m.pattern)?;
            w.properties("properties", &m.properties)?;
            w.strings("required", &m.get_required())?;
            if m.kind == ModelKind::Composed {
                w.g.write_field_name("allOf")?;
                w.g.write_start_array()?;
                for i in &m.all_of {
                    write_model_with(w.g, &i.borrow(), v1)?;
                }
                w.g.write_end_array()?;
                w.string("description", &m.description)?;
                w.object("example", &m.example)?;
            } else {
                w.object("example", &None)?;
                w.string("description", &None)?;
            }
            w.any(&m.vendor_extensions)?;
        }
    }
    w.g.write_end_object()
}

/// The token stream of `Json.mapper().convertValue(property, …)`; `stripped` when the target reads it as
/// a tree (`Schema.class` through `ModelDeserializer`), which strips trailing zeros of `BigDecimal`s.
pub fn property_to_tree_with(p: &Property, stripped: bool) -> Result<JsonNode, JacksonError> {
    let mut g = if stripped { TreeGenerator::with_stripped_decimals() } else { TreeGenerator::new() };
    write_property(&mut g, p)?;
    Ok(g.into_node())
}

/// The token stream of `Json.mapper().convertValue(model, …)` (see [`property_to_tree_with`]).
pub fn model_to_tree_with(m: &Model, stripped: bool) -> Result<JsonNode, JacksonError> {
    let mut g = if stripped { TreeGenerator::with_stripped_decimals() } else { TreeGenerator::new() };
    write_model(&mut g, m)?;
    Ok(g.into_node())
}

/// `Json.mapper().valueToTree(property)`
pub fn property_to_tree(p: &Property) -> Result<JsonNode, JacksonError> {
    let mut g = TreeGenerator::new();
    write_property(&mut g, p)?;
    Ok(g.into_node())
}

/// `Json.mapper().valueToTree(model)`
pub fn model_to_tree(m: &Model) -> Result<JsonNode, JacksonError> {
    let mut g = TreeGenerator::new();
    write_model(&mut g, m)?;
    Ok(g.into_node())
}

// ---------------------------------------------------------------------------------------------------------
// swagger-core 1.x Json.mapper(): the whole document

impl W<'_> {
    fn model_field(&mut self, name: &str, m: &Option<ModelRef>) -> R {
        if let Some(m) = m {
            self.g.write_field_name(name)?;
            write_model_with(self.g, &m.borrow(), true)?;
        }
        Ok(())
    }

    fn operation_field(&mut self, name: &str, o: &Option<OperationRef>) -> R {
        if let Some(o) = o {
            self.g.write_field_name(name)?;
            write_operation_v1(self.g, &o.borrow())?;
        }
        Ok(())
    }

    fn parameters(&mut self, name: &str, l: &Option<Vec<ParameterRef>>) -> R {
        if let Some(l) = l {
            self.g.write_field_name(name)?;
            self.g.write_start_array()?;
            for p in l {
                write_parameter_v1(self.g, &p.borrow())?;
            }
            self.g.write_end_array()?;
        }
        Ok(())
    }

    fn security_list(&mut self, name: &str, l: &Option<Vec<indexmap::IndexMap<String, Vec<String>>>>) -> R {
        if let Some(l) = l {
            self.g.write_field_name(name)?;
            self.g.write_start_array()?;
            for m in l {
                self.g.write_start_object()?;
                for (k, v) in m {
                    self.strings(k, &Some(v.clone()))?;
                }
                self.g.write_end_object()?;
            }
            self.g.write_end_array()?;
        }
        Ok(())
    }

    fn object_map(&mut self, name: &str, m: &Option<indexmap::IndexMap<String, JavaObject>>) -> R {
        if let Some(m) = m {
            self.g.write_field_name(name)?;
            self.g.write_start_object()?;
            for (k, v) in m {
                if v.is_null() {
                    continue;
                }
                self.g.write_field_name(k)?;
                write_object(self.g, v, &self.cfg)?;
            }
            self.g.write_end_object()?;
        }
        Ok(())
    }
}

/// Serializes an `Operation` as swagger-core 1.x's `Json.mapper()` does (`NON_NULL`, vendor extensions last).
pub fn write_operation_v1(g: &mut dyn JsonGenerator, o: &Operation) -> R {
    let mut w = W { g, cfg: SerConfig::swagger(), v1: true };
    w.g.write_start_object()?;
    w.strings("tags", &o.tags)?;
    w.string("summary", &o.summary)?;
    w.string("description", &o.description)?;
    w.string("operationId", &o.operation_id)?;
    if let Some(schemes) = &o.schemes {
        w.g.write_field_name("schemes")?;
        w.g.write_start_array()?;
        for x in schemes {
            match x {
                Some(x) => w.g.write_string(x.to_value())?,
                None => w.g.write_null()?,
            }
        }
        w.g.write_end_array()?;
    }
    w.strings("consumes", &o.consumes)?;
    w.strings("produces", &o.produces)?;
    w.parameters("parameters", &o.parameters)?;
    w.security_list("security", &o.security)?;
    w.external_docs(&o.external_docs)?;
    w.boolean("deprecated", o.deprecated)?;
    if let Some(r) = &o.responses {
        w.g.write_field_name("responses")?;
        w.g.write_start_object()?;
        for (k, v) in r {
            let Some(v) = v else { continue };
            w.g.write_field_name(k)?;
            write_response_v1(w.g, &v.borrow())?;
        }
        w.g.write_end_object()?;
    }
    w.any(&o.vendor_extensions)?;
    w.g.write_end_object()
}

/// Serializes a `Response` as swagger-core 1.x's `Json.mapper()` does (`NON_NULL`, vendor extensions last).
pub fn write_response_v1(g: &mut dyn JsonGenerator, r: &Response) -> R {
    let mut w = W { g, cfg: SerConfig::swagger(), v1: true };
    w.g.write_start_object()?;
    w.string("description", &r.description)?;
    w.object_map("examples", &r.examples)?;
    if let Some(h) = &r.headers {
        w.g.write_field_name("headers")?;
        w.g.write_start_object()?;
        for (k, v) in h {
            let Some(v) = v else { continue };
            w.g.write_field_name(k)?;
            write_property_with(w.g, &v.borrow(), true)?;
        }
        w.g.write_end_object()?;
    }
    if r.is_ref {
        w.string("$ref", &r.get_ref())?;
    }
    w.model_field("schema", &r.schema)?;
    w.any(&r.vendor_extensions)?;
    w.g.write_end_object()
}

/// Serializes a `Parameter` as swagger-core 1.x's `Json.mapper()` does, with the fields of its concrete class.
pub fn write_parameter_v1(g: &mut dyn JsonGenerator, p: &Parameter) -> R {
    let mut w = W { g, cfg: SerConfig::swagger(), v1: true };
    w.g.write_start_object()?;
    match p.kind {
        ParameterKind::Body => {
            w.string("in", &p.in_)?;
            w.string("name", &p.name)?;
            w.string("description", &p.description)?;
            w.boolean("required", Some(p.required))?;
            w.string("pattern", &p.pattern)?;
            w.boolean("readOnly", p.read_only)?;
            w.model_field("schema", &p.schema)?;
            w.object_map("x-examples", &p.examples)?;
        }
        ParameterKind::Ref => {
            w.string("in", &p.in_)?;
            w.string("name", &p.name)?;
            w.string("description", &p.description)?;
            w.string("pattern", &p.pattern)?;
            w.boolean("readOnly", p.read_only)?;
            w.string("$ref", &p.get_ref())?;
        }
        _ => {
            w.string("name", &p.name)?;
            w.string("in", &p.in_)?;
            w.string("description", &p.description)?;
            w.boolean("required", Some(p.required))?;
            w.string("type", &p.type_)?;
            w.property_field("items", &p.items)?;
            w.string("collectionFormat", &p.collection_format)?;
            w.object("default", &p.get_default())?;
            w.decimal("maximum", &p.maximum)?;
            w.boolean("exclusiveMaximum", p.exclusive_maximum)?;
            w.decimal("minimum", &p.minimum)?;
            w.boolean("exclusiveMinimum", p.exclusive_minimum)?;
            w.int("maxLength", p.max_length)?;
            w.int("minLength", p.min_length)?;
            w.string("pattern", &p.pattern)?;
            w.int("maxItems", p.max_items)?;
            w.int("minItems", p.min_items)?;
            w.boolean("uniqueItems", p.unique_items)?;
            w.object("multipleOf", &p.multiple_of)?;
            w.boolean("readOnly", p.read_only)?;
            w.string("format", &p.format)?;
            w.boolean("allowEmptyValue", p.allow_empty_value)?;
            w.object("x-example", &p.get_example())?;
            w.objects("enum", &p.get_enum_value())?;
        }
    }
    w.any(&p.vendor_extensions)?;
    w.g.write_end_object()
}

/// Serializes a `Path` as swagger-core 1.x's `Json.mapper()` does (`NON_NULL`, vendor extensions last).
pub fn write_path_v1(g: &mut dyn JsonGenerator, p: &Path) -> R {
    let mut w = W { g, cfg: SerConfig::swagger(), v1: true };
    w.g.write_start_object()?;
    w.operation_field("get", &p.get)?;
    w.operation_field("head", &p.head)?;
    w.operation_field("post", &p.post)?;
    w.operation_field("put", &p.put)?;
    w.operation_field("delete", &p.delete)?;
    w.operation_field("options", &p.options)?;
    w.operation_field("patch", &p.patch)?;
    w.parameters("parameters", &p.parameters)?;
    if p.is_ref {
        w.string("$ref", &p.get_ref())?;
    }
    w.any(&p.vendor_extensions)?;
    w.g.write_end_object()
}

fn write_security_definition_v1(g: &mut dyn JsonGenerator, d: &SecuritySchemeDefinition) -> R {
    let mut w = W { g, cfg: SerConfig::swagger(), v1: true };
    w.g.write_start_object()?;
    w.string("description", &d.description)?;
    w.string("type", &d.type_)?;
    match d.kind {
        SecuritySchemeKind::ApiKey => {
            w.string("name", &d.name)?;
            w.string("in", &d.in_.map(|i| i.to_value().to_string()))?;
        }
        SecuritySchemeKind::OAuth2 => {
            w.string("authorizationUrl", &d.authorization_url)?;
            w.string("tokenUrl", &d.token_url)?;
            w.string("flow", &d.flow)?;
            if let Some(sc) = &d.scopes {
                w.g.write_field_name("scopes")?;
                w.g.write_start_object()?;
                for (k, v) in sc {
                    if let Some(v) = v {
                        w.g.write_field_name(k)?;
                        w.g.write_string(v)?;
                    }
                }
                w.g.write_end_object()?;
            }
        }
        SecuritySchemeKind::Basic => {}
    }
    w.any(&d.vendor_extensions)?;
    w.g.write_end_object()
}

/// swagger-core 1.x `Json.mapper()` serialization of a whole `Swagger` document.
pub fn write_swagger_v1(g: &mut dyn JsonGenerator, s: &Swagger) -> R {
    let mut w = W { g, cfg: SerConfig::swagger(), v1: true };
    w.g.write_start_object()?;
    w.string("swagger", &s.swagger)?;
    if let Some(i) = &s.info {
        w.g.write_field_name("info")?;
        w.g.write_start_object()?;
        w.string("description", &i.description)?;
        w.string("version", &i.version)?;
        w.string("title", &i.title)?;
        w.string("termsOfService", &i.terms_of_service)?;
        if let Some(c) = &i.contact {
            w.g.write_field_name("contact")?;
            w.g.write_start_object()?;
            w.string("name", &c.name)?;
            w.string("url", &c.url)?;
            w.string("email", &c.email)?;
            w.any(&c.vendor_extensions)?;
            w.g.write_end_object()?;
        }
        if let Some(l) = &i.license {
            w.g.write_field_name("license")?;
            w.g.write_start_object()?;
            w.string("name", &l.name)?;
            w.string("url", &l.url)?;
            w.any(&l.vendor_extensions)?;
            w.g.write_end_object()?;
        }
        w.any(&i.vendor_extensions)?;
        w.g.write_end_object()?;
    }
    w.string("host", &s.host)?;
    w.string("basePath", &s.base_path)?;
    if let Some(tags) = &s.tags {
        w.g.write_field_name("tags")?;
        w.g.write_start_array()?;
        for t in tags {
            w.g.write_start_object()?;
            w.string("name", &t.name)?;
            w.string("description", &t.description)?;
            w.external_docs(&t.external_docs)?;
            w.any(&t.vendor_extensions)?;
            w.g.write_end_object()?;
        }
        w.g.write_end_array()?;
    }
    if let Some(schemes) = &s.schemes {
        w.g.write_field_name("schemes")?;
        w.g.write_start_array()?;
        for x in schemes {
            match x {
                Some(x) => w.g.write_string(x.to_value())?,
                None => w.g.write_null()?,
            }
        }
        w.g.write_end_array()?;
    }
    w.strings("consumes", &s.consumes)?;
    w.strings("produces", &s.produces)?;
    if let Some(sec) = &s.security {
        w.g.write_field_name("security")?;
        w.g.write_start_array()?;
        for r in sec {
            w.g.write_start_object()?;
            for (k, v) in &r.requirements {
                w.strings(k, &Some(v.clone()))?;
            }
            w.g.write_end_object()?;
        }
        w.g.write_end_array()?;
    }
    if let Some(paths) = &s.paths {
        w.g.write_field_name("paths")?;
        w.g.write_start_object()?;
        for (k, p) in paths {
            w.g.write_field_name(k)?;
            write_path_v1(w.g, &p.borrow())?;
        }
        w.g.write_end_object()?;
    }
    if let Some(defs) = &s.security_definitions {
        w.g.write_field_name("securityDefinitions")?;
        w.g.write_start_object()?;
        for (k, d) in defs {
            w.g.write_field_name(k)?;
            write_security_definition_v1(w.g, d)?;
        }
        w.g.write_end_object()?;
    }
    if let Some(defs) = &s.definitions {
        w.g.write_field_name("definitions")?;
        w.g.write_start_object()?;
        for (k, m) in defs {
            w.g.write_field_name(k)?;
            write_model_with(w.g, &m.borrow(), true)?;
        }
        w.g.write_end_object()?;
    }
    if let Some(params) = &s.parameters {
        w.g.write_field_name("parameters")?;
        w.g.write_start_object()?;
        for (k, p) in params {
            w.g.write_field_name(k)?;
            write_parameter_v1(w.g, &p.borrow())?;
        }
        w.g.write_end_object()?;
    }
    if let Some(r) = &s.responses {
        w.g.write_field_name("responses")?;
        w.g.write_start_object()?;
        for (k, v) in r {
            let Some(v) = v else { continue };
            w.g.write_field_name(k)?;
            write_response_v1(w.g, &v.borrow())?;
        }
        w.g.write_end_object()?;
    }
    w.external_docs(&s.external_docs)?;
    if let Some(e) = &s.vendor_extensions {
        w.any(e)?;
    }
    w.g.write_end_object()
}

/// swagger-core 1.x `Json.mapper().convertValue(swagger, JsonNode.class)`
pub fn swagger_to_tree_v1(s: &Swagger) -> Result<JsonNode, JacksonError> {
    let mut g = TreeGenerator::with_stripped_decimals();
    write_swagger_v1(&mut g, s)?;
    Ok(g.into_node())
}
