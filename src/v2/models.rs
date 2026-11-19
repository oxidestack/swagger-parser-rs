//! The Swagger 2.0 model of swagger-models 1.6.16 (`io.swagger.models`), as far as the v2 → v3 converter
//! and the Swagger 2.0 parser use it.
//!
//! Java's class hierarchies are flattened: a [`Property`], [`Model`] or [`Parameter`] is one struct whose
//! `kind` names the concrete Java class; only the fields that class declares are meaningful. Objects the
//! Java code shares and mutates in place (models, properties, parameters, responses, paths, operations)
//! are [`Shared`] handles, so aliasing created by the resolver behaves as in Java.

use crate::java::{JMap, MapKind};
use indexmap::IndexMap;
use jackson::{BigDecimal, JavaObject};
use std::cell::RefCell;
use std::rc::Rc;

/// A model object shared and mutated in place, as Java shares object references.
pub type Shared<T> = Rc<RefCell<T>>;

/// Wraps a value in a new [`Shared`] handle.
pub fn shared<T>(v: T) -> Shared<T> {
    Rc::new(RefCell::new(v))
}

/// `Map<String, Object> vendorExtensions` (a `LinkedHashMap` unless noted)
pub type VendorExtensions = JMap<JavaObject>;

/// An empty vendor-extension map (a `LinkedHashMap`, keeping insertion order).
pub fn new_vendor_extensions() -> VendorExtensions {
    JMap::with_kind(MapKind::Linked)
}

// ---------------------------------------------------------------------------------------------------------
// refs

/// `io.swagger.models.refs.RefFormat`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefFormat {
    /// `RefFormat` constant `URL`: an absolute `http:` / `https:` reference.
    Url,
    /// `RefFormat` constant `RELATIVE`: a relative file reference such as `./other.yaml#/Pet`.
    Relative,
    /// `RefFormat` constant `INTERNAL`: a reference into the same document (`#/...`) or a bare simple name.
    Internal,
}

/// `io.swagger.models.refs.RefType`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefType {
    /// `RefType.DEFINITION`: `#/definitions/`.
    Definition,
    /// `RefType.PARAMETER`: `#/parameters/`.
    Parameter,
    /// `RefType.PATH`: `#/paths/`.
    Path,
    /// `RefType.RESPONSE`: `#/responses/`.
    Response,
}

impl RefType {
    /// `RefType.getInternalPrefix()`: the `#/...s/` prefix a simple name of this type is expanded with.
    pub fn internal_prefix(&self) -> &'static str {
        match self {
            RefType::Definition => "#/definitions/",
            RefType::Parameter => "#/parameters/",
            RefType::Path => "#/paths/",
            RefType::Response => "#/responses/",
        }
    }
}

/// `io.swagger.models.refs.GenericRef`
#[derive(Clone, Debug, PartialEq)]
pub struct GenericRef {
    /// `GenericRef.format`: how the reference locates its target.
    pub format: RefFormat,
    /// `GenericRef.type`: the kind of object the reference points at.
    pub type_: RefType,
    /// `GenericRef.ref`: the reference, with a bare internal name expanded to `#/<type>/<name>`.
    pub ref_: String,
    /// `GenericRef.originalRef`: the reference as given, before expansion.
    pub original_ref: String,
    /// `GenericRef.simpleRef`: the last path segment of an internal reference, otherwise the whole reference.
    pub simple_ref: String,
}

impl GenericRef {
    /// `new GenericRef(type, ref)` / `new GenericRef(type, ref, format)`
    pub fn new(type_: RefType, r: &str, format: Option<RefFormat>) -> GenericRef {
        let format = format.unwrap_or_else(|| compute_ref_format(r));
        let ref_ = if format == RefFormat::Internal && !r.starts_with("#/") { format!("{}{}", type_.internal_prefix(), r) } else { r.to_string() };
        let simple_ref = if format == RefFormat::Internal {
            match ref_.rfind('/') {
                Some(i) => ref_[i + 1..].to_string(),
                None => ref_.clone(),
            }
        } else {
            ref_.clone()
        };
        GenericRef { format, type_, ref_, original_ref: r.to_string(), simple_ref }
    }
}

/// `GenericRef.computeRefFormat` (with the default `relativeRefWithAnyDot = true`)
pub fn compute_ref_format(r: &str) -> RefFormat {
    if r.starts_with("http:") || r.starts_with("https:") {
        return RefFormat::Url;
    }
    if r.starts_with("#/") {
        return RefFormat::Internal;
    }
    if r.starts_with('.') || r.starts_with('/') {
        return RefFormat::Relative;
    }
    if !r.contains(':') && !r.starts_with('#') && r.contains('.') {
        return RefFormat::Relative;
    }
    RefFormat::Internal
}

// ---------------------------------------------------------------------------------------------------------
// small value classes

/// `io.swagger.models.Xml`
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Xml {
    /// `Xml.name`: the XML element or attribute name.
    pub name: Option<String>,
    /// `Xml.namespace`: the XML namespace URI.
    pub namespace: Option<String>,
    /// `Xml.prefix`: the XML namespace prefix.
    pub prefix: Option<String>,
    /// `Xml.attribute`: whether the property is serialized as an attribute instead of an element.
    pub attribute: Option<bool>,
    /// `Xml.wrapped`: whether an array is wrapped in an outer element.
    pub wrapped: Option<bool>,
}

/// `io.swagger.models.ExternalDocs`
#[derive(Clone, Debug)]
pub struct ExternalDocs {
    /// `ExternalDocs.description`: a short description of the target documentation.
    pub description: Option<String>,
    /// `ExternalDocs.url`: the URL of the target documentation.
    pub url: Option<String>,
    /// `ExternalDocs.vendorExtensions`: the `x-` fields of the object.
    pub vendor_extensions: VendorExtensions,
}

impl Default for ExternalDocs {
    fn default() -> Self {
        ExternalDocs { description: None, url: None, vendor_extensions: new_vendor_extensions() }
    }
}

/// `io.swagger.models.Contact`
#[derive(Clone, Debug)]
pub struct Contact {
    /// `Contact.name`: the identifying name of the contact person or organization.
    pub name: Option<String>,
    /// `Contact.url`: the URL pointing to the contact information.
    pub url: Option<String>,
    /// `Contact.email`: the email address of the contact person or organization.
    pub email: Option<String>,
    /// `Contact.vendorExtensions`: the `x-` fields of the object.
    pub vendor_extensions: VendorExtensions,
}

impl Default for Contact {
    fn default() -> Self {
        Contact { name: None, url: None, email: None, vendor_extensions: new_vendor_extensions() }
    }
}

/// `io.swagger.models.License`
#[derive(Clone, Debug)]
pub struct License {
    /// `License.name`: the license name used for the API.
    pub name: Option<String>,
    /// `License.url`: the URL of the license text.
    pub url: Option<String>,
    /// `License.vendorExtensions`: the `x-` fields of the object.
    pub vendor_extensions: VendorExtensions,
}

impl Default for License {
    fn default() -> Self {
        License { name: None, url: None, vendor_extensions: new_vendor_extensions() }
    }
}

/// `io.swagger.models.Info`
#[derive(Clone, Debug)]
pub struct Info {
    /// `Info.description`: a short description of the application.
    pub description: Option<String>,
    /// `Info.version`: the version of the API (not of the specification).
    pub version: Option<String>,
    /// `Info.title`: the title of the application.
    pub title: Option<String>,
    /// `Info.termsOfService`: the terms of service for the API.
    pub terms_of_service: Option<String>,
    /// `Info.contact`: the contact information for the exposed API.
    pub contact: Option<Contact>,
    /// `Info.license`: the license information for the exposed API.
    pub license: Option<License>,
    /// `Info.vendorExtensions`: the `x-` fields of the object.
    pub vendor_extensions: VendorExtensions,
}

impl Default for Info {
    fn default() -> Self {
        Info { description: None, version: None, title: None, terms_of_service: None, contact: None, license: None, vendor_extensions: new_vendor_extensions() }
    }
}

/// `io.swagger.models.Tag`
#[derive(Clone, Debug)]
pub struct Tag {
    /// `Tag.name`: the name of the tag.
    pub name: Option<String>,
    /// `Tag.description`: a short description of the tag.
    pub description: Option<String>,
    /// `Tag.externalDocs`: additional external documentation for the tag.
    pub external_docs: Option<ExternalDocs>,
    /// `Tag.vendorExtensions`: the `x-` fields of the object.
    pub vendor_extensions: VendorExtensions,
}

impl Default for Tag {
    fn default() -> Self {
        Tag { name: None, description: None, external_docs: None, vendor_extensions: new_vendor_extensions() }
    }
}

/// `io.swagger.models.Scheme`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scheme {
    /// `Scheme.HTTP`: `http`.
    Http,
    /// `Scheme.HTTPS`: `https`.
    Https,
    /// `Scheme.WS`: `ws`.
    Ws,
    /// `Scheme.WSS`: `wss`.
    Wss,
}

impl Scheme {
    /// `Scheme.toValue()`: the lower-case scheme name as written in a document.
    pub fn to_value(&self) -> &'static str {
        match self {
            Scheme::Http => "http",
            Scheme::Https => "https",
            Scheme::Ws => "ws",
            Scheme::Wss => "wss",
        }
    }

    /// `Scheme.forValue` (case-insensitive)
    pub fn for_value(v: &str) -> Option<Scheme> {
        [Scheme::Http, Scheme::Https, Scheme::Ws, Scheme::Wss].into_iter().find(|s| s.to_value().eq_ignore_ascii_case(v))
    }
}

/// `io.swagger.models.SecurityRequirement`
#[derive(Clone, Debug, Default)]
pub struct SecurityRequirement {
    /// `SecurityRequirement.requirements`: security scheme name to the scopes it requires (empty for non-OAuth2 schemes).
    pub requirements: IndexMap<String, Vec<String>>,
}

/// `io.swagger.models.auth.In`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum In {
    /// `In.HEADER`: the API key travels in a request header.
    Header,
    /// `In.QUERY`: the API key travels in a query parameter.
    Query,
}

impl In {
    /// `In.forValue(value)` (`names.get(value.toLowerCase())`)
    pub fn for_value(v: &str) -> Option<In> {
        match v.to_lowercase().as_str() {
            "header" => Some(In::Header),
            "query" => Some(In::Query),
            _ => None,
        }
    }

    /// `In.toValue()`: the lower-case location name as written in a document.
    pub fn to_value(&self) -> &'static str {
        match self {
            In::Header => "header",
            In::Query => "query",
        }
    }

    /// `toString()` (the enum constant name)
    pub fn name(&self) -> &'static str {
        match self {
            In::Header => "HEADER",
            In::Query => "QUERY",
        }
    }
}

/// The concrete `SecuritySchemeDefinition` class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecuritySchemeKind {
    /// `BasicAuthDefinition`: HTTP basic authentication.
    Basic,
    /// `ApiKeyAuthDefinition`: an API key in a header or query parameter.
    ApiKey,
    /// `OAuth2Definition`: OAuth 2.0.
    OAuth2,
}

/// `io.swagger.models.auth.SecuritySchemeDefinition` (`BasicAuthDefinition`, `ApiKeyAuthDefinition`,
/// `OAuth2Definition`)
#[derive(Clone, Debug)]
pub struct SecuritySchemeDefinition {
    /// The concrete Java class this definition stands for; decides which fields are meaningful.
    pub kind: SecuritySchemeKind,
    /// `type` (initialised to "basic" / "apiKey" / "oauth2"; a bean `type` field overwrites it)
    pub type_: Option<String>,
    /// `description`: a short description of the security scheme.
    pub description: Option<String>,
    /// `vendorExtensions`: the `x-` fields of the object.
    pub vendor_extensions: VendorExtensions,
    // ApiKeyAuthDefinition
    /// `ApiKeyAuthDefinition.name`: the name of the header or query parameter carrying the key.
    pub name: Option<String>,
    /// `ApiKeyAuthDefinition.in`: where the key is sent; `None` for an unknown location.
    pub in_: Option<In>,
    // OAuth2Definition
    /// `OAuth2Definition.authorizationUrl`: the authorization URL of the `implicit` and `accessCode` flows.
    pub authorization_url: Option<String>,
    /// `OAuth2Definition.tokenUrl`: the token URL of the `password`, `application` and `accessCode` flows.
    pub token_url: Option<String>,
    /// `OAuth2Definition.flow`: the OAuth2 flow (`implicit`, `password`, `application` or `accessCode`).
    pub flow: Option<String>,
    /// `OAuth2Definition.scopes`: scope name to its description.
    pub scopes: Option<IndexMap<String, Option<String>>>,
}

impl SecuritySchemeDefinition {
    /// The no-argument constructor of the class `kind` names, with `type` preset to its scheme type.
    pub fn new(kind: SecuritySchemeKind) -> SecuritySchemeDefinition {
        let t = match kind {
            SecuritySchemeKind::Basic => "basic",
            SecuritySchemeKind::ApiKey => "apiKey",
            SecuritySchemeKind::OAuth2 => "oauth2",
        };
        SecuritySchemeDefinition {
            kind,
            type_: Some(t.into()),
            description: None,
            vendor_extensions: new_vendor_extensions(),
            name: None,
            in_: None,
            authorization_url: None,
            token_url: None,
            flow: None,
            scopes: None,
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// properties

/// The concrete `Property` class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyKind {
    /// `ArrayProperty` (`type: array`).
    Array,
    /// `BaseIntegerProperty` (`type: integer`, no format).
    BaseInteger,
    /// `BinaryProperty` (`string` / `binary`).
    Binary,
    /// `BooleanProperty` (`type: boolean`).
    Boolean,
    /// `BooleanValueProperty`: a bare `true` / `false` schema.
    BooleanValue,
    /// `ByteArrayProperty` (`string` / `byte`).
    ByteArray,
    /// `ComposedProperty` (`allOf`).
    Composed,
    /// `DateProperty` (`string` / `date`).
    Date,
    /// `DateTimeProperty` (`string` / `date-time`).
    DateTime,
    /// `DecimalProperty` (`type: number`, no format).
    Decimal,
    /// `DoubleProperty` (`number` / `double`).
    Double,
    /// `EmailProperty` (`string` / `email`).
    Email,
    /// `FileProperty` (`type: file`).
    File,
    /// `FloatProperty` (`number` / `float`).
    Float,
    /// `IntegerProperty` (`integer` / `int32`).
    Integer,
    /// `LongProperty` (`integer` / `int64`).
    Long,
    /// `MapProperty`: an object with `additionalProperties`.
    Map,
    /// `ObjectProperty`: an object with `properties`.
    Object,
    /// `PasswordProperty` (`string` / `password`).
    Password,
    /// `RefProperty`: a `$ref` to a definition.
    Ref,
    /// `StringProperty` (`type: string`).
    String,
    /// `UntypedProperty`: a schema without a type.
    Untyped,
    /// `UUIDProperty` (`string` / `uuid`).
    Uuid,
}

impl PropertyKind {
    /// `instanceof StringProperty` (ByteArrayProperty and EmailProperty extend it)
    pub fn is_string_property(&self) -> bool {
        matches!(self, PropertyKind::String | PropertyKind::ByteArray | PropertyKind::Email)
    }

    /// `instanceof AbstractNumericProperty`
    pub fn is_numeric(&self) -> bool {
        matches!(self, PropertyKind::BaseInteger | PropertyKind::Integer | PropertyKind::Long | PropertyKind::Decimal | PropertyKind::Double | PropertyKind::Float)
    }

    /// `instanceof BaseIntegerProperty`
    pub fn is_base_integer(&self) -> bool {
        matches!(self, PropertyKind::BaseInteger | PropertyKind::Integer | PropertyKind::Long)
    }

    /// `instanceof DecimalProperty`
    pub fn is_decimal(&self) -> bool {
        matches!(self, PropertyKind::Decimal | PropertyKind::Double | PropertyKind::Float)
    }

    /// Declares `minLength` / `maxLength` / `pattern` / `_default` (String) / `_enum` (`List<String>`)
    pub fn is_string_like(&self) -> bool {
        matches!(self, PropertyKind::String | PropertyKind::ByteArray | PropertyKind::Email | PropertyKind::Binary | PropertyKind::Password | PropertyKind::Uuid)
    }

    /// `getClass().getName()`
    pub fn class_name(&self) -> &'static str {
        match self {
            PropertyKind::Array => "io.swagger.models.properties.ArrayProperty",
            PropertyKind::BaseInteger => "io.swagger.models.properties.BaseIntegerProperty",
            PropertyKind::Binary => "io.swagger.models.properties.BinaryProperty",
            PropertyKind::Boolean => "io.swagger.models.properties.BooleanProperty",
            PropertyKind::BooleanValue => "io.swagger.models.properties.BooleanValueProperty",
            PropertyKind::ByteArray => "io.swagger.models.properties.ByteArrayProperty",
            PropertyKind::Composed => "io.swagger.models.properties.ComposedProperty",
            PropertyKind::Date => "io.swagger.models.properties.DateProperty",
            PropertyKind::DateTime => "io.swagger.models.properties.DateTimeProperty",
            PropertyKind::Decimal => "io.swagger.models.properties.DecimalProperty",
            PropertyKind::Double => "io.swagger.models.properties.DoubleProperty",
            PropertyKind::Email => "io.swagger.models.properties.EmailProperty",
            PropertyKind::File => "io.swagger.models.properties.FileProperty",
            PropertyKind::Float => "io.swagger.models.properties.FloatProperty",
            PropertyKind::Integer => "io.swagger.models.properties.IntegerProperty",
            PropertyKind::Long => "io.swagger.models.properties.LongProperty",
            PropertyKind::Map => "io.swagger.models.properties.MapProperty",
            PropertyKind::Object => "io.swagger.models.properties.ObjectProperty",
            PropertyKind::Password => "io.swagger.models.properties.PasswordProperty",
            PropertyKind::Ref => "io.swagger.models.properties.RefProperty",
            PropertyKind::String => "io.swagger.models.properties.StringProperty",
            PropertyKind::Untyped => "io.swagger.models.properties.UntypedProperty",
            PropertyKind::Uuid => "io.swagger.models.properties.UUIDProperty",
        }
    }
}

/// A shared, mutable [`Property`].
pub type PropertyRef = Shared<Property>;

/// `io.swagger.models.properties.Property` and its implementations.
#[derive(Clone, Debug)]
pub struct Property {
    /// The concrete Java class this property stands for; decides which fields are meaningful.
    pub kind: PropertyKind,
    // AbstractProperty
    /// `name`: the property name.
    pub name: Option<String>,
    /// `type`: the Swagger `type`, preset by the class.
    pub type_: Option<String>,
    /// `format`: the Swagger `format`, preset by some classes.
    pub format: Option<String>,
    /// `example`: an example value, typed by [`Property::set_example`].
    pub example: Option<JavaObject>,
    /// `xml`: the XML representation metadata.
    pub xml: Option<Xml>,
    /// `required`: whether the enclosing object lists this property as required.
    pub required: bool,
    /// `position`: the ordering hint of the property.
    pub position: Option<i32>,
    /// `description`: a description of the property.
    pub description: Option<String>,
    /// `title`: the title of the property.
    pub title: Option<String>,
    /// `readOnly`: `Some(true)` or `None` (a `false` is stored as `None`, see [`Property::set_read_only`]).
    pub read_only: Option<bool>,
    /// `allowEmptyValue`: whether an empty value is allowed.
    pub allow_empty_value: Option<bool>,
    /// `access`: the access hint of the property.
    pub access: Option<String>,
    /// `vendorExtensions`: the `x-` fields of the schema.
    pub vendor_extensions: VendorExtensions,
    /// `BooleanValueProperty.value`: the bare boolean the schema is.
    pub boolean_value: Option<bool>,
    // AbstractNumericProperty
    /// `AbstractNumericProperty.minimum`.
    pub minimum: Option<BigDecimal>,
    /// `AbstractNumericProperty.maximum`.
    pub maximum: Option<BigDecimal>,
    /// `AbstractNumericProperty.multipleOf`.
    pub multiple_of: Option<BigDecimal>,
    /// `AbstractNumericProperty.exclusiveMinimum`.
    pub exclusive_minimum: Option<bool>,
    /// `AbstractNumericProperty.exclusiveMaximum`.
    pub exclusive_maximum: Option<bool>,
    // String / Binary / Password / UUID / Email / ByteArray
    /// `minLength` of the string-like properties.
    pub min_length: Option<i32>,
    /// `maxLength` of the string-like properties.
    pub max_length: Option<i32>,
    /// `pattern`: the regular expression of the string-like properties.
    pub pattern: Option<String>,
    /// `_default` of the string-like properties.
    pub default_string: Option<String>,
    /// `_enum` of the String-like, Date and DateTime properties
    pub enum_strings: Option<Vec<String>>,
    /// `_default` of Boolean / Integer / Long / Float / Double properties (Boolean, Integer, Long, Float, Double)
    pub default_value: Option<JavaObject>,
    /// `_enum` of Boolean / Integer / Long / Float / Double properties
    pub enum_values: Option<Vec<JavaObject>>,
    // ArrayProperty
    /// `ArrayProperty.items`: the schema of the array elements.
    pub items: Option<PropertyRef>,
    /// `ArrayProperty.uniqueItems`: `Some(true)` or `None` (see [`Property::set_unique_items`]).
    pub unique_items: Option<bool>,
    /// `ArrayProperty.maxItems`.
    pub max_items: Option<i32>,
    /// `ArrayProperty.minItems`.
    pub min_items: Option<i32>,
    // MapProperty
    /// `MapProperty.additionalProperties`: the schema of the map values.
    pub additional_properties: Option<PropertyRef>,
    /// `MapProperty.minProperties`.
    pub min_properties: Option<i32>,
    /// `MapProperty.maxProperties`.
    pub max_properties: Option<i32>,
    // ObjectProperty
    /// `ObjectProperty.properties`: the named properties of the object.
    pub properties: Option<JMap<PropertyRef>>,
    // ComposedProperty
    /// `ComposedProperty.allOf`: the composed schemas (an empty list for a new `ComposedProperty`).
    pub all_of: Option<Vec<Option<PropertyRef>>>,
    // RefProperty
    /// `RefProperty`'s reference to a definition.
    pub generic_ref: Option<GenericRef>,
}

impl Property {
    /// The no-argument constructor of the given class.
    pub fn new(kind: PropertyKind) -> Property {
        let (t, f): (Option<&str>, Option<&str>) = match kind {
            PropertyKind::Array => (Some("array"), None),
            PropertyKind::BaseInteger => (Some("integer"), None),
            PropertyKind::Binary => (Some("string"), Some("binary")),
            PropertyKind::Boolean => (Some("boolean"), None),
            PropertyKind::BooleanValue => (None, None),
            PropertyKind::ByteArray => (Some("string"), Some("byte")),
            PropertyKind::Composed => (Some("object"), None),
            PropertyKind::Date => (Some("string"), Some("date")),
            PropertyKind::DateTime => (Some("string"), Some("date-time")),
            PropertyKind::Decimal => (Some("number"), None),
            PropertyKind::Double => (Some("number"), Some("double")),
            PropertyKind::Email => (Some("string"), Some("email")),
            PropertyKind::File => (Some("file"), None),
            PropertyKind::Float => (Some("number"), Some("float")),
            PropertyKind::Integer => (Some("integer"), Some("int32")),
            PropertyKind::Long => (Some("integer"), Some("int64")),
            PropertyKind::Map => (Some("object"), None),
            PropertyKind::Object => (Some("object"), None),
            PropertyKind::Password => (Some("string"), Some("password")),
            PropertyKind::Ref => (Some("ref"), None),
            PropertyKind::String => (Some("string"), None),
            PropertyKind::Untyped => (None, None),
            PropertyKind::Uuid => (Some("string"), Some("uuid")),
        };
        Property {
            kind,
            name: None,
            type_: t.map(str::to_string),
            format: f.map(str::to_string),
            example: None,
            xml: None,
            required: false,
            position: None,
            description: None,
            title: None,
            read_only: None,
            allow_empty_value: None,
            access: None,
            vendor_extensions: new_vendor_extensions(),
            boolean_value: None,
            minimum: None,
            maximum: None,
            multiple_of: None,
            exclusive_minimum: None,
            exclusive_maximum: None,
            min_length: None,
            max_length: None,
            pattern: None,
            default_string: None,
            enum_strings: None,
            default_value: None,
            enum_values: None,
            items: None,
            unique_items: None,
            max_items: None,
            min_items: None,
            additional_properties: None,
            min_properties: None,
            max_properties: None,
            properties: None,
            all_of: if kind == PropertyKind::Composed { Some(Vec::new()) } else { None },
            generic_ref: None,
        }
    }

    /// `new RefProperty(ref)` / `new RefProperty(ref, format)`: a reference to a definition.
    pub fn new_ref(r: &str, format: Option<RefFormat>) -> Property {
        let mut p = Property::new(PropertyKind::Ref);
        p.generic_ref = Some(GenericRef::new(RefType::Definition, r, format));
        p
    }

    /// `new BooleanValueProperty(b)`.
    pub fn new_boolean_value(b: bool) -> Property {
        let mut p = Property::new(PropertyKind::BooleanValue);
        p.boolean_value = Some(b);
        p
    }

    /// Wraps the property in a new shared handle.
    pub fn into_ref(self) -> PropertyRef {
        shared(self)
    }

    /// `RefProperty.get$ref()`
    pub fn get_ref(&self) -> Option<String> {
        self.generic_ref.as_ref().map(|g| g.ref_.clone())
    }

    /// `RefProperty.set$ref(ref)`
    pub fn set_ref(&mut self, r: &str) {
        self.generic_ref = Some(GenericRef::new(RefType::Definition, r, None));
    }

    /// `RefProperty.getRefFormat()`: `None` unless this is a reference.
    pub fn ref_format(&self) -> Option<RefFormat> {
        self.generic_ref.as_ref().map(|g| g.format)
    }

    /// `RefProperty.getSimpleRef()`: the definition name of an internal reference; `None` unless this is a reference.
    pub fn simple_ref(&self) -> Option<String> {
        self.generic_ref.as_ref().map(|g| g.simple_ref.clone())
    }

    /// `RefProperty.getOriginalRef()`: the reference as given; `None` unless this is a reference.
    pub fn original_ref(&self) -> Option<String> {
        self.generic_ref.as_ref().map(|g| g.original_ref.clone())
    }

    /// `setReadOnly(Boolean)`: `FALSE` is stored as null.
    pub fn set_read_only(&mut self, v: Option<bool>) {
        self.read_only = if v == Some(false) { None } else { v };
    }

    /// `setAllowEmptyValue(Boolean)`: null is ignored.
    pub fn set_allow_empty_value(&mut self, v: Option<bool>) {
        if v.is_some() {
            self.allow_empty_value = v;
        }
    }

    /// `ArrayProperty.setUniqueItems`: only `TRUE` is kept.
    pub fn set_unique_items(&mut self, v: Option<bool>) {
        self.unique_items = if v == Some(true) { Some(true) } else { None };
    }

    /// `setVendorExtension(name, value)` (`x-` names only)
    pub fn set_vendor_extension(&mut self, name: &str, value: JavaObject) {
        if name.starts_with("x-") {
            self.vendor_extensions.put(name, value);
        }
    }

    /// `setVendorExtensionMap(map)`: `putAll`
    pub fn set_vendor_extension_map(&mut self, m: &JMap<JavaObject>) {
        for (k, v) in m.entries() {
            self.vendor_extensions.put(k, v);
        }
    }

    /// `setExample(Object)` as overridden per class (numeric and boolean properties parse strings).
    pub fn set_example(&mut self, example: Option<JavaObject>) {
        let example = example.filter(|e| !e.is_null());
        let parsed = match (&example, self.kind) {
            (Some(JavaObject::String(s)), PropertyKind::BaseInteger | PropertyKind::Long) => Some(java_parse_long(s).map(JavaObject::Long).unwrap_or_else(|| JavaObject::String(s.clone()))),
            (Some(JavaObject::String(s)), PropertyKind::Integer) => Some(java_parse_int(s).map(JavaObject::Integer).unwrap_or_else(|| JavaObject::String(s.clone()))),
            (Some(JavaObject::String(s)), PropertyKind::Decimal | PropertyKind::Double) => {
                Some(jackson::javanum::parse_double(s).map(JavaObject::Double).unwrap_or_else(|| JavaObject::String(s.clone())))
            }
            (Some(JavaObject::String(s)), PropertyKind::Float) => Some(parse_float(s).map(JavaObject::Float).unwrap_or_else(|| JavaObject::String(s.clone()))),
            (Some(JavaObject::String(s)), PropertyKind::Boolean) => {
                if s.is_empty() {
                    // an empty string leaves the example unchanged
                    return;
                }
                Some(JavaObject::Boolean(s.eq_ignore_ascii_case("true")))
            }
            _ => example,
        };
        self.example = parsed;
    }

    /// `ObjectProperty.getRequiredProperties()` / the sorted names of required properties, null if none.
    pub fn required_properties(&self) -> Option<Vec<String>> {
        required_of(self.properties.as_ref())
    }

    /// `ObjectProperty.setRequiredProperties(list)`
    pub fn set_required_properties(&mut self, required: &[String]) {
        if let Some(props) = &self.properties {
            for s in required {
                if let Some(p) = props.get(s) {
                    p.borrow_mut().required = true;
                }
            }
        }
    }
}

/// The sorted names of the properties whose `required` flag is set; `None` when there are none.
pub fn required_of(properties: Option<&JMap<PropertyRef>>) -> Option<Vec<String>> {
    let mut out = Vec::new();
    if let Some(props) = properties {
        for (k, p) in props.entries() {
            if p.borrow().required {
                out.push(k);
            }
        }
    }
    out.sort();
    if out.is_empty() { None } else { Some(out) }
}

// ---------------------------------------------------------------------------------------------------------
// models

/// The concrete `Model` class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelKind {
    /// `ModelImpl`: an object or primitive schema.
    Impl,
    /// `ArrayModel`: an array schema.
    Array,
    /// `ComposedModel`: an `allOf` schema.
    Composed,
    /// `RefModel`: a `$ref` to a definition.
    Ref,
    /// `BooleanValueModel`: a bare `true` / `false` schema.
    BooleanValue,
}

/// A shared, mutable [`Model`].
pub type ModelRef = Shared<Model>;

/// `io.swagger.models.Model` and its implementations (`ModelImpl`, `ArrayModel`, `ComposedModel`,
/// `RefModel`, `BooleanValueModel`).
#[derive(Clone, Debug)]
pub struct Model {
    /// The concrete Java class this model stands for; decides which fields are meaningful.
    pub kind: ModelKind,
    // AbstractModel (and RefModel's own fields of the same names)
    /// `externalDocs`: additional external documentation for the schema.
    pub external_docs: Option<ExternalDocs>,
    /// `reference`: the `$ref`-like reference string `AbstractModel` carries.
    pub reference: Option<String>,
    /// `title`: the title of the schema.
    pub title: Option<String>,
    /// `vendorExtensions`: the `x-` fields of the schema (see [`Model::get_vendor_extensions`]).
    pub vendor_extensions: VendorExtensions,
    /// `xml`: the XML representation metadata.
    pub xml: Option<Xml>,
    /// ModelImpl declares its own `minimum`/`maximum` shadowing AbstractModel's; one field serves both.
    pub minimum: Option<BigDecimal>,
    /// `maximum` (see `minimum` for the shadowing).
    pub maximum: Option<BigDecimal>,
    /// `multipleOf`.
    pub multiple_of: Option<BigDecimal>,
    /// `exclusiveMinimum`.
    pub exclusive_minimum: Option<bool>,
    /// `exclusiveMaximum`.
    pub exclusive_maximum: Option<bool>,
    /// `minLength`.
    pub min_length: Option<i32>,
    /// `maxLength`.
    pub max_length: Option<i32>,
    /// `pattern`: the regular expression a string value must match.
    pub pattern: Option<String>,
    /// `properties`: the named properties of the object schema.
    pub properties: Option<JMap<PropertyRef>>,
    /// `AbstractModel.required` (the list given to `setRequired`, see [`Model::get_required`])
    pub required: Option<Vec<String>>,
    /// `BooleanValueModel.value`: the bare boolean the schema is.
    pub boolean_value: Option<bool>,
    // ModelImpl / ArrayModel
    /// `type`: the Swagger `type` (`array` for a new `ArrayModel`).
    pub type_: Option<String>,
    /// `ModelImpl.format`.
    pub format: Option<String>,
    /// `ModelImpl.name`.
    pub name: Option<String>,
    /// `ModelImpl.allowEmptyValue`.
    pub allow_empty_value: Option<bool>,
    /// `uniqueItems`.
    pub unique_items: Option<bool>,
    /// `ModelImpl.isSimple`.
    pub is_simple: bool,
    /// `description`: a description of the schema (see [`Model::get_description`]).
    pub description: Option<String>,
    /// `example`: an example value (see [`Model::get_example`]).
    pub example: Option<JavaObject>,
    /// `ModelImpl.additionalProperties`: the schema of the map values.
    pub additional_properties: Option<PropertyRef>,
    /// `ModelImpl.discriminator`: the property that tells subtypes apart.
    pub discriminator: Option<String>,
    /// `ModelImpl.defaultValue`: the `default` as a string (typed by [`Model::get_default_value`]).
    pub default_value: Option<String>,
    /// `ModelImpl._enum`: the allowed values as strings.
    pub enum_: Option<Vec<String>>,
    // ArrayModel
    /// `ArrayModel.items`: the schema of the array elements.
    pub items: Option<PropertyRef>,
    /// `ArrayModel.minItems`.
    pub min_items: Option<i32>,
    /// `ArrayModel.maxItems`.
    pub max_items: Option<i32>,
    // ComposedModel
    /// `ComposedModel.allOf`: the composed schemas.
    pub all_of: Vec<ModelRef>,
    /// `ComposedModel.parent`.
    pub parent: Option<ModelRef>,
    /// `ComposedModel.child`.
    pub child: Option<ModelRef>,
    /// `ComposedModel.interfaces`.
    pub interfaces: Option<Vec<ModelRef>>,
    // RefModel
    /// `RefModel`'s reference to a definition.
    pub generic_ref: Option<GenericRef>,
}

impl Model {
    /// The no-argument constructor of the class `kind` names.
    pub fn new(kind: ModelKind) -> Model {
        Model {
            kind,
            external_docs: None,
            reference: None,
            title: None,
            vendor_extensions: new_vendor_extensions(),
            xml: None,
            minimum: None,
            maximum: None,
            multiple_of: None,
            exclusive_minimum: None,
            exclusive_maximum: None,
            min_length: None,
            max_length: None,
            pattern: None,
            properties: None,
            required: None,
            boolean_value: None,
            type_: if kind == ModelKind::Array { Some("array".into()) } else { None },
            format: None,
            name: None,
            allow_empty_value: None,
            unique_items: None,
            is_simple: false,
            description: None,
            example: None,
            additional_properties: None,
            discriminator: None,
            default_value: None,
            enum_: None,
            items: None,
            min_items: None,
            max_items: None,
            all_of: Vec::new(),
            parent: None,
            child: None,
            interfaces: None,
            generic_ref: None,
        }
    }

    /// `new RefModel(ref)` / `new RefModel(ref, format)`: a reference to a definition.
    pub fn new_ref(r: &str, format: Option<RefFormat>) -> Model {
        let mut m = Model::new(ModelKind::Ref);
        m.generic_ref = Some(GenericRef::new(RefType::Definition, r, format));
        m
    }

    /// `new BooleanValueModel(b)`.
    pub fn new_boolean_value(b: bool) -> Model {
        let mut m = Model::new(ModelKind::BooleanValue);
        m.boolean_value = Some(b);
        m
    }

    /// Wraps the model in a new shared handle.
    pub fn into_ref(self) -> ModelRef {
        shared(self)
    }

    /// `RefModel.get$ref()`: the expanded reference; `None` unless this is a reference.
    pub fn get_ref(&self) -> Option<String> {
        self.generic_ref.as_ref().map(|g| g.ref_.clone())
    }

    /// `RefModel.set$ref(ref)`: replaces the reference to a definition.
    pub fn set_ref(&mut self, r: &str) {
        self.generic_ref = Some(GenericRef::new(RefType::Definition, r, None));
    }

    /// `RefModel.getRefFormat()`: `None` unless this is a reference.
    pub fn ref_format(&self) -> Option<RefFormat> {
        self.generic_ref.as_ref().map(|g| g.format)
    }

    /// `RefModel.getSimpleRef()`: the definition name of an internal reference; `None` unless this is a reference.
    pub fn simple_ref(&self) -> Option<String> {
        self.generic_ref.as_ref().map(|g| g.simple_ref.clone())
    }

    /// `getVendorExtensions()` (`RefModel` returns null)
    pub fn get_vendor_extensions(&self) -> Option<VendorExtensions> {
        if self.kind == ModelKind::Ref { None } else { Some(self.vendor_extensions.clone()) }
    }

    /// `getBooleanValue()` (`RefModel` returns null)
    pub fn get_boolean_value(&self) -> Option<bool> {
        if self.kind == ModelKind::Ref { None } else { self.boolean_value }
    }

    /// `getDescription()` (`BooleanValueModel` returns null)
    pub fn get_description(&self) -> Option<String> {
        if self.kind == ModelKind::BooleanValue { None } else { self.description.clone() }
    }

    /// `setDescription` (a no-op on `BooleanValueModel`)
    pub fn set_description(&mut self, d: Option<String>) {
        if self.kind != ModelKind::BooleanValue {
            self.description = d;
        }
    }

    /// `getExample()` (`BooleanValueModel` returns null)
    pub fn get_example(&self) -> Option<JavaObject> {
        if self.kind == ModelKind::BooleanValue { None } else { self.example.clone() }
    }

    /// `setExample(Object)` (a no-op on `BooleanValueModel`).
    pub fn set_example(&mut self, e: Option<JavaObject>) {
        if self.kind != ModelKind::BooleanValue {
            self.example = e;
        }
    }

    /// `setVendorExtension(name, value)` (`x-` names only)
    pub fn set_vendor_extension(&mut self, name: &str, value: JavaObject) {
        if name.starts_with("x-") {
            self.vendor_extensions.put(name, value);
        }
    }

    /// `AbstractModel.addProperty(key, property)`
    pub fn add_property(&mut self, key: &str, property: Option<PropertyRef>) {
        let Some(property) = property else { return };
        let props = self.properties.get_or_insert_with(|| JMap::with_kind(MapKind::Linked)).clone();
        if let Some(req) = &self.required && req.iter().any(|r| r == key) {
            property.borrow_mut().required = true;
        }
        props.put(key, property);
    }

    /// `setProperties(map)`: `AbstractModel` adds each entry; `RefModel` stores the map.
    pub fn set_properties(&mut self, properties: Option<JMap<PropertyRef>>) {
        if self.kind == ModelKind::Ref {
            self.properties = properties;
            return;
        }
        if let Some(p) = properties {
            for (k, v) in p.entries() {
                self.add_property(&k, Some(v));
            }
        }
    }

    /// `AbstractModel.getRequired()`: computed from the properties' flags, sorted.
    pub fn get_required(&self) -> Option<Vec<String>> {
        required_of(self.properties.as_ref())
    }

    /// `AbstractModel.setRequired(list)`
    pub fn set_required(&mut self, required: Option<Vec<String>>) {
        if let (Some(req), Some(props)) = (&required, &self.properties) {
            for s in req {
                if let Some(p) = props.get(s) {
                    p.borrow_mut().required = true;
                }
            }
        }
        self.required = required;
    }

    /// `ModelImpl.setAdditionalProperties(p)`: also sets `type` to "object".
    pub fn set_additional_properties(&mut self, p: Option<PropertyRef>) {
        self.type_ = Some("object".into());
        self.additional_properties = p;
    }

    /// `ModelImpl.getDefaultValue()`: typed by `type` (Integer / BigDecimal), null on a parse failure.
    pub fn get_default_value(&self) -> Option<JavaObject> {
        let d = self.default_value.as_ref()?;
        match self.type_.as_deref() {
            Some("integer") => java_parse_int(d).map(JavaObject::Integer),
            Some("number") => BigDecimal::parse(d).ok().map(|b| JavaObject::BigDecimal(b.into())),
            _ => Some(JavaObject::string(d.as_str())),
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// parameters

/// The concrete `Parameter` class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParameterKind {
    /// `QueryParameter` (`in: query`).
    Query,
    /// `HeaderParameter` (`in: header`).
    Header,
    /// `PathParameter` (`in: path`).
    Path,
    /// `FormParameter` (`in: formData`).
    Form,
    /// `CookieParameter` (`in: cookie`).
    Cookie,
    /// `BodyParameter` (`in: body`).
    Body,
    /// `RefParameter`: a `$ref` to a shared parameter.
    Ref,
}

impl ParameterKind {
    /// `instanceof SerializableParameter` / `AbstractSerializableParameter`
    pub fn is_serializable(&self) -> bool {
        !matches!(self, ParameterKind::Body | ParameterKind::Ref)
    }
}

/// A shared, mutable [`Parameter`].
pub type ParameterRef = Shared<Parameter>;

/// `io.swagger.models.parameters.Parameter` and its implementations.
#[derive(Clone, Debug)]
pub struct Parameter {
    /// The concrete Java class this parameter stands for; decides which fields are meaningful.
    pub kind: ParameterKind,
    // AbstractParameter
    /// `vendorExtensions`: the `x-` fields of the parameter.
    pub vendor_extensions: VendorExtensions,
    /// `in`: the parameter location, preset by the class (`None` for a reference).
    pub in_: Option<String>,
    /// `name`: the parameter name.
    pub name: Option<String>,
    /// `description`: a description of the parameter.
    pub description: Option<String>,
    /// `required`: whether the parameter is mandatory (preset for a path parameter).
    pub required: bool,
    /// `access`: the access hint of the parameter.
    pub access: Option<String>,
    /// `pattern`: the regular expression a value must match.
    pub pattern: Option<String>,
    /// `readOnly`.
    pub read_only: Option<bool>,
    // AbstractSerializableParameter
    /// `type`: the Swagger `type` of a non-body parameter (set via [`Parameter::set_type`]).
    pub type_: Option<String>,
    /// `format`: the Swagger `format`.
    pub format: Option<String>,
    /// `collectionFormat`: how array values are serialized (`csv`, `ssv`, `tsv`, `pipes`, `multi`).
    pub collection_format: Option<String>,
    /// `items`: the schema of the array elements.
    pub items: Option<PropertyRef>,
    /// `exclusiveMaximum`.
    pub exclusive_maximum: Option<bool>,
    /// `maximum`.
    pub maximum: Option<BigDecimal>,
    /// `exclusiveMinimum`.
    pub exclusive_minimum: Option<bool>,
    /// `minimum`.
    pub minimum: Option<BigDecimal>,
    /// `example` as a string (typed by [`Parameter::get_example`]).
    pub example: Option<String>,
    /// `maxItems`.
    pub max_items: Option<i32>,
    /// `minItems`.
    pub min_items: Option<i32>,
    /// `allowEmptyValue`.
    pub allow_empty_value: Option<bool>,
    /// `_enum`: the allowed values as strings (typed by [`Parameter::get_enum_value`]).
    pub enum_: Option<Vec<String>>,
    /// `maxLength`.
    pub max_length: Option<i32>,
    /// `minLength`.
    pub min_length: Option<i32>,
    /// `uniqueItems`.
    pub unique_items: Option<bool>,
    /// `Number multipleOf`
    pub multiple_of: Option<JavaObject>,
    /// `defaultValue`: the `default` as a string (typed by [`Parameter::get_default`]).
    pub default_value: Option<String>,
    // BodyParameter
    /// `BodyParameter.schema`: the schema of the request body.
    pub schema: Option<ModelRef>,
    /// `BodyParameter.examples`: media type to example value.
    pub examples: Option<IndexMap<String, JavaObject>>,
    // RefParameter
    /// `RefParameter`'s reference to a shared parameter.
    pub generic_ref: Option<GenericRef>,
}

impl Parameter {
    /// The no-argument constructor of the class `kind` names, with `in` preset.
    pub fn new(kind: ParameterKind) -> Parameter {
        let in_ = match kind {
            ParameterKind::Query => Some("query"),
            ParameterKind::Header => Some("header"),
            ParameterKind::Path => Some("path"),
            ParameterKind::Form => Some("formData"),
            ParameterKind::Cookie => Some("cookie"),
            ParameterKind::Body => Some("body"),
            ParameterKind::Ref => None,
        };
        Parameter {
            kind,
            vendor_extensions: new_vendor_extensions(),
            in_: in_.map(str::to_string),
            name: None,
            description: None,
            required: kind == ParameterKind::Path,
            access: None,
            pattern: None,
            read_only: None,
            type_: None,
            format: None,
            collection_format: None,
            items: None,
            exclusive_maximum: None,
            maximum: None,
            exclusive_minimum: None,
            minimum: None,
            example: None,
            max_items: None,
            min_items: None,
            allow_empty_value: None,
            enum_: None,
            max_length: None,
            min_length: None,
            unique_items: None,
            multiple_of: None,
            default_value: None,
            schema: None,
            examples: None,
            generic_ref: None,
        }
    }

    /// `new RefParameter(ref)`: a reference to a shared parameter.
    pub fn new_ref(r: &str) -> Parameter {
        let mut p = Parameter::new(ParameterKind::Ref);
        p.generic_ref = Some(GenericRef::new(RefType::Parameter, r, None));
        p
    }

    /// Wraps the parameter in a new shared handle.
    pub fn into_ref(self) -> ParameterRef {
        shared(self)
    }

    /// `RefParameter.get$ref()`: the expanded reference; `None` unless this is a reference.
    pub fn get_ref(&self) -> Option<String> {
        self.generic_ref.as_ref().map(|g| g.ref_.clone())
    }

    /// `RefParameter.set$ref(ref)`: replaces the reference to a shared parameter.
    pub fn set_ref(&mut self, r: &str) {
        self.generic_ref = Some(GenericRef::new(RefType::Parameter, r, None));
    }

    /// `RefParameter.getRefFormat()`: `None` unless this is a reference.
    pub fn ref_format(&self) -> Option<RefFormat> {
        self.generic_ref.as_ref().map(|g| g.format)
    }

    /// `RefParameter.getSimpleRef()`: the parameter name of an internal reference; `None` unless this is a reference.
    pub fn simple_ref(&self) -> Option<String> {
        self.generic_ref.as_ref().map(|g| g.simple_ref.clone())
    }

    /// `setVendorExtension(name, value)` (`x-` names only)
    pub fn set_vendor_extension(&mut self, name: &str, value: JavaObject) {
        if name.starts_with("x-") {
            self.vendor_extensions.put(name, value);
        }
    }

    fn default_collection_format(&self) -> &'static str {
        match self.kind {
            ParameterKind::Query | ParameterKind::Form => "multi",
            _ => "csv",
        }
    }

    /// `AbstractSerializableParameter.setType(type)`: arrays get the default collection format, other
    /// types lose theirs.
    pub fn set_type(&mut self, t: Option<String>) {
        let is_array = t.as_deref() == Some("array");
        self.type_ = t;
        if is_array {
            if self.collection_format.is_none() {
                self.collection_format = Some(self.default_collection_format().into());
            }
        } else {
            self.collection_format = None;
        }
    }

    /// `AbstractSerializableParameter.setProperty(property)`
    pub fn set_property(&mut self, p: &Property) {
        self.set_type(p.type_.clone());
        self.format = p.format.clone();
        match p.kind {
            k if k.is_string_property() => self.enum_ = p.enum_strings.clone(),
            PropertyKind::Integer | PropertyKind::Long | PropertyKind::Float | PropertyKind::Double => {
                self.set_enum_value(p.enum_values.as_ref().map(|v| v.iter().map(|o| o.to_java_string()).collect()));
            }
            PropertyKind::Array => self.items = p.items.clone(),
            _ => {}
        }
    }

    /// `setEnumValue(list)`: `toString()` of each item
    pub fn set_enum_value(&mut self, v: Option<Vec<String>>) {
        self.enum_ = v;
    }

    /// `setDefault(Object)`: `toString()`
    pub fn set_default(&mut self, v: Option<String>) {
        self.default_value = v;
    }

    /// `AbstractSerializableParameter.getDefault()`
    pub fn get_default(&self) -> Option<JavaObject> {
        let d = self.default_value.as_ref().filter(|d| !d.is_empty())?;
        match self.type_.as_deref() {
            Some("integer") => {
                if let Some(l) = java_parse_long(d) {
                    return Some(JavaObject::Long(l));
                }
            }
            Some("number") => {
                if let Some(x) = jackson::javanum::parse_double(d) {
                    return Some(JavaObject::Double(x));
                }
            }
            Some("boolean") if d.eq_ignore_ascii_case("true") || d.eq_ignore_ascii_case("false") => {
                return Some(JavaObject::Boolean(d.eq_ignore_ascii_case("true")));
            }
            _ => {}
        }
        Some(JavaObject::string(d.as_str()))
    }

    /// `AbstractSerializableParameter.getDefaultValue()` (Integer / BigDecimal by type; null on failure)
    pub fn get_default_value(&self) -> Option<JavaObject> {
        let d = self.default_value.as_ref()?;
        match self.type_.as_deref() {
            Some("integer") => java_parse_int(d).map(JavaObject::Integer),
            Some("number") => BigDecimal::parse(d).ok().map(|b| JavaObject::BigDecimal(b.into())),
            _ => Some(JavaObject::string(d.as_str())),
        }
    }

    /// `AbstractSerializableParameter.getExample()` (typed by `type`)
    pub fn get_example(&self) -> Option<JavaObject> {
        let e = self.example.as_ref()?;
        if e.is_empty() {
            return Some(JavaObject::string(e.as_str()));
        }
        match self.type_.as_deref() {
            Some("integer") => {
                if let Some(l) = java_parse_long(e) {
                    return Some(JavaObject::Long(l));
                }
            }
            Some("number") => {
                if let Some(x) = jackson::javanum::parse_double(e) {
                    return Some(JavaObject::Double(x));
                }
            }
            Some("boolean") if e.eq_ignore_ascii_case("true") || e.eq_ignore_ascii_case("false") => {
                return Some(JavaObject::Boolean(e.eq_ignore_ascii_case("true")));
            }
            _ => {}
        }
        Some(JavaObject::string(e.as_str()))
    }

    /// `AbstractSerializableParameter.getEnumValue()`
    pub fn get_enum_value(&self) -> Option<Vec<JavaObject>> {
        let e = self.enum_.as_ref()?;
        if e.is_empty() {
            return Some(Vec::new());
        }
        let t = self.type_.as_deref();
        let mut out = Vec::with_capacity(e.len());
        if t != Some("integer") && t != Some("number") {
            if let Some(items) = self.items.as_ref().filter(|_| t.is_none() || t == Some("array")) {
                let kind = items.borrow().kind;
                for s in e {
                    let v = match kind {
                        k if k.is_string_property() => Some(JavaObject::string(s.as_str())),
                        PropertyKind::Integer | PropertyKind::BaseInteger => java_parse_int(s).map(JavaObject::Integer),
                        PropertyKind::Long => java_parse_long(s).map(JavaObject::Long),
                        PropertyKind::Float => parse_float(s).map(JavaObject::Float),
                        PropertyKind::Double | PropertyKind::Decimal => jackson::javanum::parse_double(s).map(JavaObject::Double),
                        _ => Some(JavaObject::string(s.as_str())),
                    };
                    out.push(v.unwrap_or_else(|| JavaObject::string(s.as_str())));
                }
            } else {
                for s in e {
                    out.push(JavaObject::string(s.as_str()));
                }
            }
        } else {
            let f = self.format.as_deref();
            for s in e {
                let v = if f == Some("int32") {
                    java_parse_int(s).map(JavaObject::Integer)
                } else if f == Some("int64") {
                    java_parse_long(s).map(JavaObject::Long)
                } else if f == Some("double") {
                    jackson::javanum::parse_double(s).map(JavaObject::Double)
                } else if f == Some("float") {
                    parse_float(s).map(JavaObject::Float)
                } else if t == Some("integer") {
                    java_parse_int(s).map(JavaObject::Integer)
                } else {
                    jackson::javanum::parse_double(s).map(JavaObject::Double)
                };
                out.push(v.unwrap_or_else(|| JavaObject::string(s.as_str())));
            }
        }
        Some(out)
    }
}

// ---------------------------------------------------------------------------------------------------------
// responses, operations, paths

/// A shared, mutable [`Response`].
pub type ResponseRef = Shared<Response>;

/// `io.swagger.models.Response` / `RefResponse`
#[derive(Clone, Debug)]
pub struct Response {
    /// `RefResponse` when `generic_ref` is set by the constructor
    pub is_ref: bool,
    /// `description`: a short description of the response.
    pub description: Option<String>,
    /// `responseSchema`: the schema of the response body.
    pub schema: Option<ModelRef>,
    /// `examples`: media type to example value.
    pub examples: Option<IndexMap<String, JavaObject>>,
    /// `headers`: header name to its schema.
    pub headers: Option<IndexMap<String, Option<PropertyRef>>>,
    /// `vendorExtensions`: the `x-` fields of the response.
    pub vendor_extensions: VendorExtensions,
    /// `RefResponse`'s reference to a shared response.
    pub generic_ref: Option<GenericRef>,
}

impl Response {
    /// `new Response()`.
    pub fn new() -> Response {
        Response { is_ref: false, description: None, schema: None, examples: None, headers: None, vendor_extensions: new_vendor_extensions(), generic_ref: None }
    }

    /// `new RefResponse(ref)`: a reference to a shared response.
    pub fn new_ref(r: &str) -> Response {
        let mut x = Response::new();
        x.is_ref = true;
        x.generic_ref = Some(GenericRef::new(RefType::Response, r, None));
        x
    }

    /// Wraps the response in a new shared handle.
    pub fn into_ref(self) -> ResponseRef {
        shared(self)
    }

    /// `RefResponse.get$ref()`: the expanded reference; `None` unless this is a reference.
    pub fn get_ref(&self) -> Option<String> {
        self.generic_ref.as_ref().map(|g| g.ref_.clone())
    }

    /// `RefResponse.set$ref(ref)`: replaces the reference to a shared response.
    pub fn set_ref(&mut self, r: &str) {
        self.generic_ref = Some(GenericRef::new(RefType::Response, r, None));
    }

    /// `RefResponse.getRefFormat()`: `None` unless this is a reference.
    pub fn ref_format(&self) -> Option<RefFormat> {
        self.generic_ref.as_ref().map(|g| g.format)
    }

    /// `setVendorExtension(name, value)` (`x-` names only)
    pub fn set_vendor_extension(&mut self, name: &str, value: JavaObject) {
        if name.starts_with("x-") {
            self.vendor_extensions.put(name, value);
        }
    }
}

impl Default for Response {
    fn default() -> Self {
        Response::new()
    }
}

/// `io.swagger.models.Responses`: a `LinkedHashMap<String, Response>` with (HashMap) vendor extensions.
/// Values may be null.
pub type Responses = IndexMap<String, Option<ResponseRef>>;

/// A shared, mutable [`Operation`].
pub type OperationRef = Shared<Operation>;

/// `io.swagger.models.Operation`
#[derive(Clone, Debug)]
pub struct Operation {
    /// `vendorExtensions`: the `x-` fields of the operation.
    pub vendor_extensions: VendorExtensions,
    /// `tags`: the tags grouping the operation.
    pub tags: Option<Vec<String>>,
    /// `summary`: a short summary of what the operation does.
    pub summary: Option<String>,
    /// `description`: a verbose explanation of the operation.
    pub description: Option<String>,
    /// `operationId`: the unique identifier of the operation.
    pub operation_id: Option<String>,
    /// (`null` elements come from unknown scheme names in bean deserialization)
    pub schemes: Option<Vec<Option<Scheme>>>,
    /// `consumes`: the MIME types the operation accepts.
    pub consumes: Option<Vec<String>>,
    /// `produces`: the MIME types the operation returns.
    pub produces: Option<Vec<String>>,
    /// `parameters`: the parameters of the operation (an empty list for a new operation).
    pub parameters: Option<Vec<ParameterRef>>,
    /// `responsesObject` (`getResponses()` returns a copy with the same entries)
    pub responses: Option<Responses>,
    /// `security`: the security requirements (scheme name to scopes) of the operation.
    pub security: Option<Vec<IndexMap<String, Vec<String>>>>,
    /// `externalDocs`: additional external documentation for the operation.
    pub external_docs: Option<ExternalDocs>,
    /// `deprecated`: whether the operation is deprecated.
    pub deprecated: Option<bool>,
}

impl Operation {
    /// `new Operation()`: empty, with an empty parameter list.
    pub fn new() -> Operation {
        Operation {
            vendor_extensions: new_vendor_extensions(),
            tags: None,
            summary: None,
            description: None,
            operation_id: None,
            schemes: None,
            consumes: None,
            produces: None,
            parameters: Some(Vec::new()),
            responses: None,
            security: None,
            external_docs: None,
            deprecated: None,
        }
    }

    /// Wraps the operation in a new shared handle.
    pub fn into_ref(self) -> OperationRef {
        shared(self)
    }

    /// `setVendorExtension(name, value)` (`x-` names only)
    pub fn set_vendor_extension(&mut self, name: &str, value: JavaObject) {
        if name.starts_with("x-") {
            self.vendor_extensions.put(name, value);
        }
    }
}

impl Default for Operation {
    fn default() -> Self {
        Operation::new()
    }
}

/// A shared, mutable [`Path`].
pub type PathRef = Shared<Path>;

/// `io.swagger.models.Path` / `RefPath`
#[derive(Clone, Debug)]
pub struct Path {
    /// `RefPath` when set: the path item is a `$ref`.
    pub is_ref: bool,
    /// `vendorExtensions`: the `x-` fields of the path item.
    pub vendor_extensions: VendorExtensions,
    /// `get`: the GET operation.
    pub get: Option<OperationRef>,
    /// `put`: the PUT operation.
    pub put: Option<OperationRef>,
    /// `post`: the POST operation.
    pub post: Option<OperationRef>,
    /// `head`: the HEAD operation.
    pub head: Option<OperationRef>,
    /// `delete`: the DELETE operation.
    pub delete: Option<OperationRef>,
    /// `patch`: the PATCH operation.
    pub patch: Option<OperationRef>,
    /// `options`: the OPTIONS operation.
    pub options: Option<OperationRef>,
    /// `parameters`: the parameters shared by all operations of the path.
    pub parameters: Option<Vec<ParameterRef>>,
    /// `RefPath`'s reference to another path item.
    pub generic_ref: Option<GenericRef>,
}

impl Path {
    /// `new Path()`.
    pub fn new() -> Path {
        Path { is_ref: false, vendor_extensions: new_vendor_extensions(), get: None, put: None, post: None, head: None, delete: None, patch: None, options: None, parameters: None, generic_ref: None }
    }

    /// `new RefPath(ref)`: a reference to another path item; `r` `None` leaves the reference unset.
    pub fn new_ref(r: Option<&str>) -> Path {
        let mut p = Path::new();
        p.is_ref = true;
        p.generic_ref = r.map(|r| GenericRef::new(RefType::Path, r, None));
        p
    }

    /// Wraps the path in a new shared handle.
    pub fn into_ref(self) -> PathRef {
        shared(self)
    }

    /// `RefPath.get$ref()`: the expanded reference; `None` unless this is a reference.
    pub fn get_ref(&self) -> Option<String> {
        self.generic_ref.as_ref().map(|g| g.ref_.clone())
    }

    /// `RefPath.getRefFormat()`: `None` unless this is a reference.
    pub fn ref_format(&self) -> Option<RefFormat> {
        self.generic_ref.as_ref().map(|g| g.format)
    }

    /// `getOperations()`: get, put, head, post, delete, patch, options
    pub fn get_operations(&self) -> Vec<OperationRef> {
        [&self.get, &self.put, &self.head, &self.post, &self.delete, &self.patch, &self.options].into_iter().flatten().cloned().collect()
    }

    /// `getOperationMap()` values: get, put, post, delete, patch, head, options
    pub fn get_operation_map(&self) -> Vec<OperationRef> {
        [&self.get, &self.put, &self.post, &self.delete, &self.patch, &self.head, &self.options].into_iter().flatten().cloned().collect()
    }

    /// `setVendorExtension(name, value)` (`x-` names only)
    pub fn set_vendor_extension(&mut self, name: &str, value: JavaObject) {
        if name.starts_with("x-") {
            self.vendor_extensions.put(name, value);
        }
    }
}

impl Default for Path {
    fn default() -> Self {
        Path::new()
    }
}

// ---------------------------------------------------------------------------------------------------------
// the document

/// `io.swagger.models.Swagger`
#[derive(Clone, Debug)]
pub struct Swagger {
    /// `Swagger.swagger`: the specification version, `2.0` by default.
    pub swagger: Option<String>,
    /// `Swagger.info`: metadata about the API.
    pub info: Option<Info>,
    /// `Swagger.host`: the host (name or IP, optional port) serving the API.
    pub host: Option<String>,
    /// `Swagger.basePath`: the base path the API is served on, relative to the host.
    pub base_path: Option<String>,
    /// `Swagger.tags`: the tags used by the document, with their metadata.
    pub tags: Option<Vec<Tag>>,
    /// (a null element is possible: `scheme(Scheme.forValue(unknown))`)
    pub schemes: Option<Vec<Option<Scheme>>>,
    /// `Swagger.consumes`: the MIME types the API accepts by default.
    pub consumes: Option<Vec<String>>,
    /// `Swagger.produces`: the MIME types the API returns by default.
    pub produces: Option<Vec<String>>,
    /// `Swagger.security`: the security requirements applied to the whole API.
    pub security: Option<Vec<SecurityRequirement>>,
    /// `Swagger.paths`: path template to its path item.
    pub paths: Option<IndexMap<String, PathRef>>,
    /// `Swagger.securityDefinitions`: scheme name to its security scheme.
    pub security_definitions: Option<IndexMap<String, SecuritySchemeDefinition>>,
    /// `Swagger.definitions`: definition name to its schema.
    pub definitions: Option<IndexMap<String, ModelRef>>,
    /// `Swagger.parameters`: shared parameter name to its parameter.
    pub parameters: Option<IndexMap<String, ParameterRef>>,
    /// `Swagger.responses`: shared response name to its response.
    pub responses: Option<IndexMap<String, Option<ResponseRef>>>,
    /// `Swagger.externalDocs`: additional external documentation for the API.
    pub external_docs: Option<ExternalDocs>,
    /// `Swagger.vendorExtensions`: the `x-` fields of the document; `None` until one is set.
    pub vendor_extensions: Option<VendorExtensions>,
}

impl Default for Swagger {
    fn default() -> Self {
        Swagger {
            swagger: Some("2.0".into()),
            info: None,
            host: None,
            base_path: None,
            tags: None,
            schemes: None,
            consumes: None,
            produces: None,
            security: None,
            paths: None,
            security_definitions: None,
            definitions: None,
            parameters: None,
            responses: None,
            external_docs: None,
            vendor_extensions: None,
        }
    }
}

impl Swagger {
    /// `vendorExtension(key, value)`
    pub fn vendor_extension(&mut self, key: &str, value: JavaObject) {
        self.vendor_extensions.get_or_insert_with(new_vendor_extensions).put(key, value);
    }

    /// `addDefinition(key, model)`
    pub fn add_definition(&mut self, key: &str, model: ModelRef) {
        self.definitions.get_or_insert_with(IndexMap::new).insert(key.to_string(), model);
    }

    /// `path(key, path)`
    pub fn path(&mut self, key: &str, path: PathRef) {
        self.paths.get_or_insert_with(IndexMap::new).insert(key.to_string(), path);
    }
}

/// `io.swagger.parser.util.SwaggerDeserializationResult`
#[derive(Clone, Debug, Default)]
pub struct SwaggerDeserializationResult {
    /// `SwaggerDeserializationResult.swagger`: the parsed document, `None` when parsing failed.
    pub swagger: Option<Shared<Swagger>>,
    /// `SwaggerDeserializationResult.messages`: the parse and validation messages, `None` until one is added.
    pub messages: Option<Vec<String>>,
}

impl SwaggerDeserializationResult {
    /// `message(m)`: appends a message and returns `self` for chaining.
    pub fn message(&mut self, m: impl Into<String>) -> &mut Self {
        self.messages.get_or_insert_with(Vec::new).push(m.into());
        self
    }
}

/// `io.swagger.models.auth.AuthorizationValue`
pub type AuthorizationValue = crate::core::AuthorizationValue;

// ---------------------------------------------------------------------------------------------------------
// Java number parsing

/// `Integer.parseInt(s)`
pub fn java_parse_int(s: &str) -> Option<i32> {
    java_parse_long(s).filter(|l| *l >= i32::MIN as i64 && *l <= i32::MAX as i64).map(|l| l as i32)
}

/// `Long.parseLong(s)`
pub fn java_parse_long(s: &str) -> Option<i64> {
    crate::models::schema::java_parse_long(s)
}

/// `Float.parseFloat(s)`
pub fn parse_float(s: &str) -> Option<f32> {
    jackson::javanum::parse_float(s)
}
