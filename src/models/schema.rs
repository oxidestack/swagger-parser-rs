//! `Schema<T>` and its subclasses (`StringSchema`, `ArraySchema`, `ComposedSchema`, …) as one handle type with
//! a [`SchemaKind`] tag; the subclass-specific `cast` of example/default/enum/const values is reproduced.

use super::*;
use crate::java::{IntoNullable, JList, JMap, JSet};
use jackson::convert::{civil_from_days, date_only_format, days_from_civil, uuid_from_string};
use jackson::object::OffsetDateTime;
use jackson::{BigDecimal, JacksonError, JavaObject, JsonGenerator};

/// `Schema.BIND_TYPE_AND_TYPES`: the `bind-type` switch `getType()` consults (the Java system property,
/// process-wide, off by default).
static BIND_TYPE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Sets the `bind-type` switch (`System.setProperty("bind-type", …)`).
pub fn set_bind_type(on: bool) {
    BIND_TYPE.store(on, std::sync::atomic::Ordering::SeqCst);
}

/// Whether the `bind-type` switch is on (`Boolean.parseBoolean(System.getProperty("bind-type"))`); see [`set_bind_type`].
pub fn bind_type() -> bool {
    BIND_TYPE.load(std::sync::atomic::Ordering::SeqCst)
}

/// The concrete Java class of a schema.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SchemaKind {
    /// Plain `Schema`, no subclass.
    #[default]
    Schema,
    /// `ArbitrarySchema`.
    Arbitrary,
    /// `ArraySchema`: type `array`.
    Array,
    /// `BinarySchema`: type `string`, format `binary`.
    Binary,
    /// `BooleanSchema`: type `boolean`.
    Boolean,
    /// `ByteArraySchema`: type `string`, format `byte`.
    ByteArray,
    /// `ComposedSchema` (`allOf`/`anyOf`/`oneOf`).
    Composed,
    /// `DateSchema`: type `string`, format `date`.
    Date,
    /// `DateTimeSchema`: type `string`, format `date-time`.
    DateTime,
    /// `EmailSchema`: type `string`, format `email`.
    Email,
    /// `FileSchema`: type `string`, format `binary`.
    File,
    /// `IntegerSchema`: type `integer`, format `int32`.
    Integer,
    /// `JsonSchema`: the OpenAPI 3.1 schema, spec version 3.1.
    Json,
    /// `MapSchema`: type `object`.
    Map,
    /// `NumberSchema`: type `number`.
    Number,
    /// `ObjectSchema`: type `object`.
    Object,
    /// `PasswordSchema`: type `string`, format `password`.
    Password,
    /// `StringSchema`: type `string`.
    String,
    /// `UUIDSchema`: type `string`, format `uuid`.
    Uuid,
}

impl SchemaKind {
    /// Simple class name (`getClass().getSimpleName()`).
    pub fn simple_name(&self) -> &'static str {
        match self {
            SchemaKind::Schema => "Schema",
            SchemaKind::Arbitrary => "ArbitrarySchema",
            SchemaKind::Array => "ArraySchema",
            SchemaKind::Binary => "BinarySchema",
            SchemaKind::Boolean => "BooleanSchema",
            SchemaKind::ByteArray => "ByteArraySchema",
            SchemaKind::Composed => "ComposedSchema",
            SchemaKind::Date => "DateSchema",
            SchemaKind::DateTime => "DateTimeSchema",
            SchemaKind::Email => "EmailSchema",
            SchemaKind::File => "FileSchema",
            SchemaKind::Integer => "IntegerSchema",
            SchemaKind::Json => "JsonSchema",
            SchemaKind::Map => "MapSchema",
            SchemaKind::Number => "NumberSchema",
            SchemaKind::Object => "ObjectSchema",
            SchemaKind::Password => "PasswordSchema",
            SchemaKind::String => "StringSchema",
            SchemaKind::Uuid => "UUIDSchema",
        }
    }

    /// `getClass().getName()`
    pub fn class_name(&self) -> String {
        format!("io.swagger.v3.oas.models.media.{}", self.simple_name())
    }

    /// (type, format) passed to `Schema(String type, String format)` by the subclass constructor.
    fn type_and_format(&self) -> Option<(&'static str, Option<&'static str>)> {
        Some(match self {
            SchemaKind::Array => ("array", None),
            SchemaKind::Binary => ("string", Some("binary")),
            SchemaKind::Boolean => ("boolean", None),
            SchemaKind::ByteArray => ("string", Some("byte")),
            SchemaKind::Date => ("string", Some("date")),
            SchemaKind::DateTime => ("string", Some("date-time")),
            SchemaKind::Email => ("string", Some("email")),
            SchemaKind::File => ("string", Some("binary")),
            SchemaKind::Integer => ("integer", Some("int32")),
            SchemaKind::Map => ("object", None),
            SchemaKind::Number => ("number", None),
            SchemaKind::Object => ("object", None),
            SchemaKind::Password => ("string", Some("password")),
            SchemaKind::String => ("string", None),
            SchemaKind::Uuid => ("string", Some("uuid")),
            SchemaKind::Schema | SchemaKind::Arbitrary | SchemaKind::Composed | SchemaKind::Json => return None,
        })
    }
}

/// `Schema.additionalProperties` (an `Object` restricted to `Boolean` or `Schema`).
#[derive(Clone, Debug, PartialEq)]
pub enum AdditionalProperties {
    /// `Boolean`: `additionalProperties: true|false`.
    Bool(bool),
    /// A `Schema` describing the additional properties.
    Schema(Schema),
}

impl AdditionalProperties {
    /// Writes the value with `g`: a JSON boolean, or the schema serialized for mapper `m`.
    pub fn serialize(&self, g: &mut dyn JsonGenerator, m: Mapper) -> Result<(), JacksonError> {
        match self {
            AdditionalProperties::Bool(b) => g.write_boolean(*b),
            AdditionalProperties::Schema(s) => s.serialize(g, m),
        }
    }

    /// The schema, `None` for a boolean.
    pub fn as_schema(&self) -> Option<&Schema> {
        match self {
            AdditionalProperties::Schema(s) => Some(s),
            _ => None,
        }
    }

    /// The boolean, `None` for a schema.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            AdditionalProperties::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

impl JavaHash for AdditionalProperties {
    fn java_hash_code(&self) -> i32 {
        match self {
            AdditionalProperties::Bool(b) => b.java_hash_code(),
            AdditionalProperties::Schema(s) => s.java_hash_code(),
        }
    }
}

impl From<bool> for AdditionalProperties {
    fn from(b: bool) -> Self {
        AdditionalProperties::Bool(b)
    }
}

impl From<Schema> for AdditionalProperties {
    fn from(s: Schema) -> Self {
        AdditionalProperties::Schema(s)
    }
}

impl IntoNullable<AdditionalProperties> for bool {
    fn into_nullable(self) -> Option<AdditionalProperties> {
        Some(AdditionalProperties::Bool(self))
    }
}

impl IntoNullable<AdditionalProperties> for Schema {
    fn into_nullable(self) -> Option<AdditionalProperties> {
        Some(AdditionalProperties::Schema(self))
    }
}

impl IntoNullable<JavaObject> for &str {
    fn into_nullable(self) -> Option<JavaObject> {
        Some(JavaObject::string(self))
    }
}

impl IntoNullable<JavaObject> for String {
    fn into_nullable(self) -> Option<JavaObject> {
        Some(JavaObject::string(self))
    }
}

impl IntoNullable<JavaObject> for i32 {
    fn into_nullable(self) -> Option<JavaObject> {
        Some(JavaObject::Integer(self))
    }
}

impl IntoNullable<JavaObject> for bool {
    fn into_nullable(self) -> Option<JavaObject> {
        Some(JavaObject::Boolean(self))
    }
}

impl Default for Schema {
    fn default() -> Self {
        Schema::new()
    }
}

impl Schema {
    /// `new Schema()`
    pub fn new() -> Schema {
        Schema::of(SchemaKind::Schema)
    }

    /// `new Schema(SpecVersion)`
    pub fn new_with_spec_version(v: SpecVersion) -> Schema {
        let s = Schema::new();
        s.set_spec_version(v);
        s
    }

    /// `new XxxSchema()` for the given subclass.
    pub fn of(kind: SchemaKind) -> Schema {
        let s = Schema::from_data(SchemaData { kind, spec_version: Some(SpecVersion::V30), ..Default::default() });
        if let Some((t, f)) = kind.type_and_format() {
            {
                let mut d = s.write();
                d.type_ = Some(t.to_string());
                d.format = f.map(str::to_string);
            }
            s.add_type(t);
        }
        if kind == SchemaKind::Json {
            s.set_spec_version(SpecVersion::V31);
        }
        s
    }

    /// `new StringSchema()`.
    pub fn string_schema() -> Schema {
        Schema::of(SchemaKind::String)
    }
    /// `new IntegerSchema()`.
    pub fn integer_schema() -> Schema {
        Schema::of(SchemaKind::Integer)
    }
    /// `new NumberSchema()`.
    pub fn number_schema() -> Schema {
        Schema::of(SchemaKind::Number)
    }
    /// `new BooleanSchema()`.
    pub fn boolean_schema() -> Schema {
        Schema::of(SchemaKind::Boolean)
    }
    /// `new ArraySchema()`.
    pub fn array_schema() -> Schema {
        Schema::of(SchemaKind::Array)
    }
    /// `new ObjectSchema()`.
    pub fn object_schema() -> Schema {
        Schema::of(SchemaKind::Object)
    }
    /// `new MapSchema()`.
    pub fn map_schema() -> Schema {
        Schema::of(SchemaKind::Map)
    }
    /// `new ComposedSchema()`.
    pub fn composed_schema() -> Schema {
        Schema::of(SchemaKind::Composed)
    }
    /// `new JsonSchema()`.
    pub fn json_schema() -> Schema {
        Schema::of(SchemaKind::Json)
    }

    /// The Java class this schema is an instance of.
    pub fn kind(&self) -> SchemaKind {
        self.read().kind
    }

    /// `instanceof` checks: the subclass hierarchy is flat (every subclass extends `Schema` directly).
    pub fn is_kind(&self, k: SchemaKind) -> bool {
        self.kind() == k
    }

    /// `instanceof ArraySchema`.
    pub fn is_array_schema(&self) -> bool {
        self.kind() == SchemaKind::Array
    }

    /// `instanceof ComposedSchema`.
    pub fn is_composed_schema(&self) -> bool {
        self.kind() == SchemaKind::Composed
    }

    /// `instanceof ObjectSchema`.
    pub fn is_object_schema(&self) -> bool {
        self.kind() == SchemaKind::Object
    }

    /// `instanceof MapSchema`.
    pub fn is_map_schema(&self) -> bool {
        self.kind() == SchemaKind::Map
    }

    /// `instanceof JsonSchema`.
    pub fn is_json_schema(&self) -> bool {
        self.kind() == SchemaKind::Json
    }

    // ---- type / types ----

    /// `getType()`: with `bind-type` on (see [`set_bind_type`]) a missing `type` falls back to the single
    /// entry of `types`.
    pub fn get_type(&self) -> Option<String> {
        let d = self.read();
        if bind_type() && d.type_.is_none() && let Some(types) = d.types.as_ref().filter(|t| t.len() == 1) {
            return types.items().into_iter().next();
        }
        d.type_.clone()
    }

    /// `setType(type)`: the raw `type` field; `None` clears it.
    pub fn set_type(&self, t: impl IntoNullable<String>) {
        self.write().type_ = t.into_nullable();
    }

    /// `type(type)`: [`Self::set_type`], returning this schema for chaining.
    pub fn type_(&self, t: impl IntoNullable<String>) -> Schema {
        self.set_type(t);
        self.clone()
    }

    /// `getTypes()`: the 3.1 `type` set, `None` when unset.
    pub fn get_types(&self) -> Option<JSet> {
        self.read().types.clone()
    }

    /// `setTypes(types)`.
    pub fn set_types(&self, t: impl IntoNullable<JSet>) {
        self.write().types = t.into_nullable();
    }

    /// `types(types)`: [`Self::set_types`], returning this schema for chaining.
    pub fn types(&self, t: impl IntoNullable<JSet>) -> Schema {
        self.set_types(t);
        self.clone()
    }

    /// `addType(type)` (creates a `LinkedHashSet`)
    pub fn add_type(&self, t: &str) -> bool {
        let set = { self.write().types.get_or_insert_with(JSet::new).clone() };
        set.add(t)
    }

    /// `typesItem(type)`: [`Self::add_type`], returning this schema for chaining.
    pub fn types_item(&self, t: &str) -> Schema {
        self.add_type(t);
        self.clone()
    }

    // ---- $ref ----

    /// `get$ref()`: the reference as stored, `None` when this schema is not a reference.
    pub fn get_ref(&self) -> Option<String> {
        self.read().ref_.clone()
    }

    /// `set$ref`: a bare name becomes `#/components/schemas/<name>`
    pub fn set_ref(&self, r: impl IntoNullable<String>) {
        let r = r.into_nullable().map(|r| {
            if !r.starts_with('#') && !r.contains('.') && !r.contains('/') { format!("#/components/schemas/{r}") } else { r }
        });
        self.write().ref_ = r;
    }

    /// `$ref(ref)`: [`Self::set_ref`], returning this schema for chaining.
    pub fn ref_(&self, r: impl IntoNullable<String>) -> Schema {
        self.set_ref(r);
        self.clone()
    }

    /// `raw$ref`: no prefixing
    pub fn raw_ref(&self, r: impl IntoNullable<String>) -> Schema {
        self.write().ref_ = r.into_nullable();
        self.clone()
    }

    // ---- required ----

    /// `getRequired()`: the required property names, `None` when unset.
    pub fn get_required(&self) -> Option<JList<String>> {
        self.read().required.clone()
    }

    /// `setRequired`: keeps entries present in `properties` (if any), de-duplicated and sorted; empty → null.
    pub fn set_required(&self, required: impl IntoNullable<JList<String>>) {
        let props = self.get_properties();
        let mut list: Vec<String> = Vec::new();
        if let Some(req) = required.into_nullable() {
            for r in req.to_vec() {
                if props.as_ref().is_none_or(|p| p.contains_key(&r)) && !list.contains(&r) {
                    list.push(r);
                }
            }
        }
        list.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
        self.write().required = if list.is_empty() { None } else { Some(JList::from_vec(list)) };
    }

    /// `required(List)`: null leaves the field unchanged? No: Java sets the field to null.
    pub fn required(&self, required: impl IntoNullable<JList<String>>) -> Schema {
        match required.into_nullable() {
            None => self.write().required = None,
            Some(r) => {
                let mut v: Vec<String> = Vec::new();
                for x in r.to_vec() {
                    if !v.contains(&x) {
                        v.push(x);
                    }
                }
                v.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
                self.write().required = Some(JList::from_vec(v));
            }
        }
        self.clone()
    }

    /// `addRequiredItem`
    pub fn add_required_item(&self, item: &str) -> Schema {
        let list = { self.write().required.get_or_insert_with(JList::new).clone() };
        if !list.contains(&item.to_string()) {
            list.add(item.to_string());
            let mut v = list.to_vec();
            v.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
            list.clear();
            for x in v {
                list.add(x);
            }
        }
        self.clone()
    }

    // ---- properties ----

    /// `addProperty(key, schema)` (creates a `LinkedHashMap`)
    pub fn add_property(&self, key: &str, s: Schema) -> Schema {
        let map = { self.write().properties.get_or_insert_with(JMap::new).clone() };
        map.put(key, s);
        self.clone()
    }

    /// `addProperties(key, schema)`: same as [`Self::add_property`].
    pub fn add_properties(&self, key: &str, s: Schema) -> Schema {
        self.add_property(key, s)
    }

    /// `addPatternProperty(key, schema)`: puts into `patternProperties`, creating the map.
    pub fn add_pattern_property(&self, key: &str, s: Schema) -> Schema {
        let map = { self.write().pattern_properties.get_or_insert_with(JMap::new).clone() };
        map.put(key, s);
        self.clone()
    }

    /// `addAllOfItem(schema)`: appends to `allOf`, creating the list.
    pub fn add_all_of_item(&self, s: Schema) -> Schema {
        let l = { self.write().all_of.get_or_insert_with(JList::new).clone() };
        l.add(s);
        self.clone()
    }

    /// `addAnyOfItem(schema)`: appends to `anyOf`, creating the list.
    pub fn add_any_of_item(&self, s: Schema) -> Schema {
        let l = { self.write().any_of.get_or_insert_with(JList::new).clone() };
        l.add(s);
        self.clone()
    }

    /// `addOneOfItem(schema)`: appends to `oneOf`, creating the list.
    pub fn add_one_of_item(&self, s: Schema) -> Schema {
        let l = { self.write().one_of.get_or_insert_with(JList::new).clone() };
        l.add(s);
        self.clone()
    }

    /// `addPrefixItem(schema)`: appends to `prefixItems`, creating the list.
    pub fn add_prefix_item(&self, s: Schema) -> Schema {
        let l = { self.write().prefix_items.get_or_insert_with(JList::new).clone() };
        l.add(s);
        self.clone()
    }

    // ---- additionalProperties ----

    /// `getAdditionalProperties()`: a boolean or a schema, `None` when unset.
    pub fn get_additional_properties(&self) -> Option<AdditionalProperties> {
        self.read().additional_properties.clone()
    }

    /// `setAdditionalProperties(value)`.
    pub fn set_additional_properties(&self, v: impl IntoNullable<AdditionalProperties>) {
        self.write().additional_properties = v.into_nullable();
    }

    /// `additionalProperties(value)`: [`Self::set_additional_properties`], returning this schema for chaining.
    pub fn additional_properties(&self, v: impl IntoNullable<AdditionalProperties>) -> Schema {
        self.set_additional_properties(v);
        self.clone()
    }

    // ---- example / default / const / enum / examples (typed by subclass) ----

    /// `getExample()`: the example after the subclass cast, `None` when unset or explicitly null.
    pub fn get_example(&self) -> Option<JavaObject> {
        self.read().example.clone()
    }

    /// `setExample(Object)`: casts; the set-flag records an explicit null (or a successful cast).
    pub fn set_example(&self, v: impl IntoNullable<JavaObject>) {
        let v = v.into_nullable().filter(|o| !o.is_null());
        let cast = v.as_ref().and_then(|o| self.cast(o));
        let mut d = self.write();
        let flag = v.is_none() || cast.is_some();
        d.example = cast;
        if flag {
            d.example_set_flag = true;
        }
    }

    /// `example(Object)`: `ObjectSchema`/`ArbitrarySchema` override it to store `example.toString()`.
    pub fn example(&self, v: impl IntoNullable<JavaObject>) -> Schema {
        let v = v.into_nullable().filter(|o| !o.is_null());
        match self.kind() {
            SchemaKind::Object | SchemaKind::Arbitrary => self.set_example(v.map(|o| JavaObject::string(o.to_java_string()))),
            _ => self.set_example(v),
        }
        self.clone()
    }

    /// `getDefault()`: the default after the subclass cast, `None` when unset or explicitly null.
    pub fn get_default(&self) -> Option<JavaObject> {
        self.read().default_.clone()
    }

    /// `setDefault(Object)`: casts like [`Self::set_example`]; the set-flag records an explicit null (or a successful cast).
    pub fn set_default(&self, v: impl IntoNullable<JavaObject>) {
        let v = v.into_nullable().filter(|o| !o.is_null());
        let cast = v.as_ref().and_then(|o| self.cast(o));
        let mut d = self.write();
        let flag = v.is_none() || cast.is_some();
        d.default_ = cast;
        if flag {
            d.default_set_flag = true;
        }
    }

    /// `_default(value)`: [`Self::set_default`], returning this schema for chaining.
    pub fn default_(&self, v: impl IntoNullable<JavaObject>) -> Schema {
        self.set_default(v);
        self.clone()
    }

    /// `getConst()`: the 3.1 `const` after the subclass cast.
    pub fn get_const(&self) -> Option<JavaObject> {
        self.read().const_.clone()
    }

    /// `setConst(Object)`: casts the value; a failed cast stores `None`.
    pub fn set_const(&self, v: impl IntoNullable<JavaObject>) {
        let v = v.into_nullable().filter(|o| !o.is_null());
        let cast = v.as_ref().and_then(|o| self.cast(o));
        self.write().const_ = cast;
    }

    /// `_const(value)`: [`Self::set_const`], returning this schema for chaining.
    pub fn const_(&self, v: impl IntoNullable<JavaObject>) -> Schema {
        self.set_const(v);
        self.clone()
    }

    /// `getEnum()`: the enum values, `None` when unset.
    pub fn get_enum(&self) -> Option<JList<JavaObject>> {
        self.read().enum_.clone()
    }

    /// `setEnum(list)`: stores the list as is, without casting.
    pub fn set_enum(&self, v: impl IntoNullable<JList<JavaObject>>) {
        self.write().enum_ = v.into_nullable();
    }

    /// `_enum(list)`: [`Self::set_enum`], returning this schema for chaining.
    pub fn enum_(&self, v: impl IntoNullable<JList<JavaObject>>) -> Schema {
        self.set_enum(v);
        self.clone()
    }

    /// `addEnumItemObject(T)`: the item is cast (a failed cast adds null); `BooleanSchema.addEnumItem` does not cast.
    pub fn add_enum_item_object(&self, v: JavaObject) {
        let item = if v.is_null() { JavaObject::Null } else { self.cast(&v).unwrap_or(JavaObject::Null) };
        let l = { self.write().enum_.get_or_insert_with(JList::new).clone() };
        l.add(item);
    }

    /// `getExamples()`: the 3.1 `examples` list, `None` when unset.
    pub fn get_examples(&self) -> Option<JList<JavaObject>> {
        self.read().examples.clone()
    }

    /// `setExamples(list)`.
    pub fn set_examples(&self, v: impl IntoNullable<JList<JavaObject>>) {
        self.write().examples = v.into_nullable();
    }

    /// `examples(list)`: [`Self::set_examples`], returning this schema for chaining.
    pub fn examples(&self, v: impl IntoNullable<JList<JavaObject>>) -> Schema {
        self.set_examples(v);
        self.clone()
    }

    /// `addExample(value)`: appends to `examples` uncast, creating the list.
    pub fn add_example(&self, v: JavaObject) {
        let l = { self.write().examples.get_or_insert_with(JList::new).clone() };
        l.add(v);
    }

    // ---- extensions ----

    /// `addExtension(name, value)`: in 3.0 only `x-` names; in 3.1 any non-empty name.
    pub fn add_extension(&self, name: &str, value: JavaObject) {
        let mut d = self.write();
        if !name.is_empty() && (d.spec_version != Some(SpecVersion::V30) || name.starts_with("x-")) {
            put_extension(&mut d.extensions, name, value);
        }
    }

    /// `setExtensions(map)`: replaces the extensions map.
    pub fn set_extensions(&self, v: impl IntoNullable<JMap<JavaObject>>) {
        self.write().extensions = v.into_nullable();
    }

    /// `extensions(map)`: [`Self::set_extensions`], returning this schema for chaining.
    pub fn extensions(&self, v: impl IntoNullable<JMap<JavaObject>>) -> Schema {
        self.set_extensions(v);
        self.clone()
    }

    /// The subclass-specific `cast(Object)`; `None` where Java returns null.
    pub fn cast(&self, value: &JavaObject) -> Option<JavaObject> {
        cast_value(self.kind(), value, || self.resolve_json_type())
    }

    /// `JsonSchema.resolveType()`
    fn resolve_json_type(&self) -> String {
        match self.get_types() {
            None => "null".into(),
            Some(t) => {
                let items = t.items();
                if items.len() == 1 {
                    let ty = &items[0];
                    if ty == "integer" || ty == "number" { "number".into() } else { ty.clone() }
                } else if t.contains("object") {
                    "object".into()
                } else if t.contains("string") {
                    "string".into()
                } else if t.contains("array") {
                    "array".into()
                } else if t.contains("integer") || t.contains("number") {
                    "number".into()
                } else if t.contains("boolean") {
                    "boolean".into()
                } else {
                    items.first().cloned().unwrap_or_default()
                }
            }
        }
    }

    /// Writes an example/default/enum item; `DateSchemaMixin` formats `Date` values as `yyyy-MM-dd` (UTC)
    /// for example/default/enum on the 3.0 and 3.1 mappers.
    pub(crate) fn write_typed_value(&self, g: &mut dyn JsonGenerator, v: &JavaObject, m: Mapper, field: &str) -> Result<(), JacksonError> {
        if self.kind() == SchemaKind::Date && m != Mapper::Converter && matches!(field, "example" | "default" | "enum")
            && let JavaObject::Date(ms) = v
        {
            return g.write_string(&date_only_format(*ms));
        }
        if field == "example" {
            return write_object_content_always(g, v, m);
        }
        write_object(g, v, m)
    }
}

/// `Schema.cast` per subclass.
pub(crate) fn cast_value(kind: SchemaKind, value: &JavaObject, json_type: impl Fn() -> String) -> Option<JavaObject> {
    match kind {
        SchemaKind::String | SchemaKind::Email | SchemaKind::Password => Some(JavaObject::string(value.to_java_string())),
        SchemaKind::Integer => {
            let s = value.to_java_string();
            let l = java_parse_long(&s)?;
            if l >= i32::MIN as i64 && l <= i32::MAX as i64 { Some(JavaObject::Integer(l as i32)) } else { Some(JavaObject::Long(l)) }
        }
        SchemaKind::Number => BigDecimal::parse(&value.to_java_string()).ok().map(|b| JavaObject::BigDecimal(b.into())),
        SchemaKind::Boolean => Some(JavaObject::Boolean(value.to_java_string().eq_ignore_ascii_case("true"))),
        SchemaKind::Date => match value {
            JavaObject::Date(_) => Some(value.clone()),
            JavaObject::String(s) => parse_date_utc(s).map(JavaObject::Date),
            _ => None,
        },
        SchemaKind::DateTime => match value {
            JavaObject::Date(ms) => Some(JavaObject::OffsetDateTime(OffsetDateTime {
                epoch_second: ms.div_euclid(1000),
                nano: (ms.rem_euclid(1000) * 1_000_000) as u32,
                offset_seconds: 0,
            })),
            JavaObject::String(s) => parse_offset_date_time(s).map(JavaObject::OffsetDateTime),
            JavaObject::OffsetDateTime(_) => Some(value.clone()),
            _ => None,
        },
        SchemaKind::Uuid => uuid_from_string(&value.to_java_string()).map(JavaObject::Uuid),
        SchemaKind::Binary | SchemaKind::ByteArray => match value {
            JavaObject::Bytes(_) => Some(value.clone()),
            other => Some(JavaObject::Bytes(other.to_java_string().into_bytes().into())),
        },
        SchemaKind::Json => match value {
            JavaObject::String(s) => {
                let t = json_type();
                if t == "number" {
                    // NumberFormat.getInstance().parse(..) must succeed, then Integer/Long.parseLong of the text
                    if java_number_format_parses(s) && let Some(l) = java_parse_long(s) {
                        if l >= i32::MIN as i64 && l <= i32::MAX as i64 {
                            return Some(JavaObject::Integer(l as i32));
                        }
                        return Some(JavaObject::Long(l));
                    }
                    Some(value.clone())
                } else if t == "boolean" {
                    Some(JavaObject::Boolean(s.eq_ignore_ascii_case("true")))
                } else {
                    Some(value.clone())
                }
            }
            _ => Some(value.clone()),
        },
        SchemaKind::Schema | SchemaKind::Arbitrary | SchemaKind::Array | SchemaKind::Composed | SchemaKind::File | SchemaKind::Map | SchemaKind::Object => {
            Some(value.clone())
        }
    }
}

/// `Long.parseLong(s)` (optional sign, decimal digits, range-checked).
pub(crate) fn java_parse_long(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.is_empty() {
        return None;
    }
    let digits = if b[0] == b'-' || b[0] == b'+' { &s[1..] } else { s };
    if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    s.parse::<i64>().ok()
}

/// Whether `NumberFormat.getInstance()` (en_US) parses a leading number from `s`.
fn java_number_format_parses(s: &str) -> bool {
    let t = s.trim_start_matches('-');
    t.starts_with(|c: char| c.is_ascii_digit()) || t.starts_with('.') && t[1..].starts_with(|c: char| c.is_ascii_digit())
}

/// `new SimpleDateFormat("yyyy-MM-dd Z").parse(value + " UTC")` (lenient).
pub(crate) fn parse_date_utc(value: &str) -> Option<i64> {
    let parts: Vec<&str> = value.split('-').collect();
    if parts.len() != 3 || parts.iter().any(|p| p.is_empty() || !p.bytes().all(|c| c.is_ascii_digit())) {
        return None;
    }
    let y: i64 = parts[0].parse().ok()?;
    let m: i64 = parts[1].parse().ok()?;
    let d: i64 = parts[2].parse().ok()?;
    let month0 = m - 1;
    let yy = y + month0.div_euclid(12);
    let mm = month0.rem_euclid(12) + 1;
    let days = days_from_civil(yy, mm as u32, 1) + (d - 1);
    Some(days * 86_400_000)
}

/// `OffsetDateTime.parse(text)` (ISO_OFFSET_DATE_TIME).
pub(crate) fn parse_offset_date_time(s: &str) -> Option<OffsetDateTime> {
    let re = regex_lite(s)?;
    Some(re)
}

fn regex_lite(s: &str) -> Option<OffsetDateTime> {
    // yyyy-MM-ddTHH:mm[:ss[.fffffffff]](Z|±HH:MM[:SS]|±HHMM|±HH)
    let b = s.as_bytes();
    let num = |i: usize, n: usize| -> Option<i64> {
        let sl = s.get(i..i + n)?;
        if sl.bytes().all(|c| c.is_ascii_digit()) { sl.parse().ok() } else { None }
    };
    if b.len() < 17 || b[4] != b'-' || b[7] != b'-' || (b[10] != b'T' && b[10] != b't') || b[13] != b':' {
        return None;
    }
    let (y, mo, d, h, mi) = (num(0, 4)?, num(5, 2)?, num(8, 2)?, num(11, 2)?, num(14, 2)?);
    let mut i = 16;
    let mut sec = 0;
    let mut nano: u32 = 0;
    if b.get(i) == Some(&b':') {
        sec = num(i + 1, 2)?;
        i += 3;
        if b.get(i) == Some(&b'.') {
            let start = i + 1;
            let mut j = start;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            let frac = &s[start..j];
            if frac.is_empty() || frac.len() > 9 {
                return None;
            }
            nano = format!("{frac:0<9}").parse().ok()?;
            i = j;
        }
    }
    let rest = &s[i..];
    let offset = if rest == "Z" || rest == "z" {
        0
    } else {
        let sign = match rest.as_bytes().first()? {
            b'+' => 1,
            b'-' => -1,
            _ => return None,
        };
        let body = &rest[1..];
        let (oh, om, os) = match body.len() {
            2 => (body.parse::<i64>().ok()?, 0, 0),
            5 if body.as_bytes()[2] == b':' => (body[0..2].parse().ok()?, body[3..5].parse().ok()?, 0),
            8 if body.as_bytes()[2] == b':' && body.as_bytes()[5] == b':' => (body[0..2].parse().ok()?, body[3..5].parse().ok()?, body[6..8].parse().ok()?),
            _ => return None,
        };
        if oh > 18 || om > 59 || os > 59 {
            return None;
        }
        sign * (oh * 3600 + om * 60 + os)
    };
    if !(1..=12).contains(&mo) || d < 1 || d > days_in_month(y, mo) || h > 23 || mi > 59 || sec > 59 {
        return None;
    }
    let days = days_from_civil(y, mo as u32, d as u32);
    let local = days * 86400 + h * 3600 + mi * 60 + sec;
    Some(OffsetDateTime { epoch_second: local - offset, nano, offset_seconds: offset as i32 })
}

fn days_in_month(y: i64, m: i64) -> i64 {
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    days_from_civil(ny, nm as u32, 1) - days_from_civil(y, m as u32, 1)
}

#[allow(dead_code)]
fn unused() {
    let _ = civil_from_days(0);
}
