//! The swagger-core 2.2.53 `io.swagger.v3.oas.models` classes and their Jackson serialization as configured
//! by `Json.mapper()` / `Yaml.mapper()` (OpenAPI 3.0), `Json31.mapper()` / `Yaml31.mapper()` (3.1) and
//! `Json31.converterMapper()`.
//!
//! Every model is a shared handle (`Arc<RwLock<…Data>>`): cloning aliases the same object, getters return
//! clones of the (shared) field values, setters mutate in place — the Java object graph semantics the parser
//! algorithms rely on. Collections are [`JList`]/[`JMap`]/[`JSet`] handles for the same reason.

mod custom;
pub mod deserialize;
mod generated;
mod generated_de;
pub mod schema;
mod serialize;

pub use generated::*;
pub use schema::{AdditionalProperties, SchemaKind};
pub use serialize::*;
pub use deserialize::FromTree;

use crate::java::{JList, JMap, JSet};
use jackson::JavaObject;

/// `SpecVersion`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SpecVersion {
    /// `SpecVersion.V30`: OpenAPI 3.0.
    #[default]
    V30,
    /// `SpecVersion.V31`: OpenAPI 3.1.
    V31,
}

macro_rules! string_enum {
    ($(#[$m:meta])* $name:ident { $($variant:ident => $text:expr),* $(,)? }) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum $name { $(#[doc = concat!("`", $text, "`.")] $variant),* }

        impl $name {
            /// `toString()` (used for serialization with `WRITE_ENUMS_USING_TO_STRING`)
            pub fn as_str(&self) -> &'static str {
                match self { $($name::$variant => $text),* }
            }

            /// The constant whose `toString()` equals `s`.
            pub fn from_str_value(s: &str) -> Option<$name> {
                match s { $($text => Some($name::$variant),)* _ => None }
            }

            /// `values()`: every constant, in declaration order.
            pub const VALUES: &'static [$name] = &[$($name::$variant),*];
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        /// Enum constants hash by identity in Java; any fixed per-constant value keeps the semantics.
        impl JavaHash for $name {
            fn java_hash_code(&self) -> i32 {
                java_compat::lang::string_hash(self.as_str())
            }
        }
    };
}

/// `Object.hashCode()` as the swagger-core models (and the JDK collections they hold) compute it.
pub trait JavaHash {
    /// The Java `hashCode()` of this value.
    fn java_hash_code(&self) -> i32;
}

impl JavaHash for String {
    fn java_hash_code(&self) -> i32 {
        java_compat::lang::string_hash(self)
    }
}

impl JavaHash for bool {
    fn java_hash_code(&self) -> i32 {
        if *self { 1231 } else { 1237 }
    }
}

impl JavaHash for i32 {
    fn java_hash_code(&self) -> i32 {
        *self
    }
}

impl JavaHash for jackson::BigDecimal {
    fn java_hash_code(&self) -> i32 {
        jackson::BigDecimal::java_hash_code(self)
    }
}

impl JavaHash for JavaObject {
    fn java_hash_code(&self) -> i32 {
        JavaObject::java_hash_code(self)
    }
}

impl JavaHash for SpecVersion {
    fn java_hash_code(&self) -> i32 {
        match self {
            SpecVersion::V30 => java_compat::lang::string_hash("V30"),
            SpecVersion::V31 => java_compat::lang::string_hash("V31"),
        }
    }
}

impl<T: JavaHash> JavaHash for Option<T> {
    fn java_hash_code(&self) -> i32 {
        match self {
            Some(x) => x.java_hash_code(),
            None => 0,
        }
    }
}

/// `List.hashCode()`
impl<T: JavaHash + Clone> JavaHash for JList<T> {
    fn java_hash_code(&self) -> i32 {
        self.to_vec().iter().fold(1i32, |acc, e| acc.wrapping_mul(31).wrapping_add(e.java_hash_code()))
    }
}

/// `AbstractMap.hashCode()`: the sum of `key.hashCode() ^ value.hashCode()`
impl<V: JavaHash + Clone> JavaHash for JMap<V> {
    fn java_hash_code(&self) -> i32 {
        self.entries().iter().fold(0i32, |acc, (k, v)| acc.wrapping_add(java_compat::lang::string_hash(k) ^ v.java_hash_code()))
    }
}

/// `AbstractSet.hashCode()`
impl JavaHash for JSet {
    fn java_hash_code(&self) -> i32 {
        self.items().iter().fold(0i32, |acc, e| acc.wrapping_add(java_compat::lang::string_hash(e)))
    }
}

/// `Objects.hash(values...)`
pub fn objects_hash(values: &[i32]) -> i32 {
    values.iter().fold(1i32, |acc, h| acc.wrapping_mul(31).wrapping_add(*h))
}

/// The hand-rolled `result = 31 * result + h` chain (first element seeds the result).
pub fn chain_hash(values: &[i32]) -> i32 {
    let mut it = values.iter();
    let first = it.next().copied().unwrap_or(0);
    it.fold(first, |acc, h| acc.wrapping_mul(31).wrapping_add(*h))
}

string_enum!(
    /// `Parameter.StyleEnum`
    ParameterStyle { Matrix => "matrix", Label => "label", Form => "form", Simple => "simple", SpaceDelimited => "spaceDelimited", PipeDelimited => "pipeDelimited", DeepObject => "deepObject" }
);
string_enum!(
    /// `Header.StyleEnum`
    HeaderStyle { Simple => "simple" }
);
string_enum!(
    /// `Encoding.StyleEnum`
    EncodingStyle { Form => "form", SpaceDelimited => "spaceDelimited", PipeDelimited => "pipeDelimited", DeepObject => "deepObject" }
);
string_enum!(
    /// `EncodingProperty.StyleEnum`
    EncodingPropertyStyle { Form => "form", SpaceDelimited => "spaceDelimited", PipeDelimited => "pipeDelimited", DeepObject => "deepObject" }
);
string_enum!(
    /// `SecurityScheme.Type`
    SecuritySchemeType { ApiKey => "apiKey", Http => "http", Oauth2 => "oauth2", OpenIdConnect => "openIdConnect", MutualTls => "mutualTLS" }
);
string_enum!(
    /// `SecurityScheme.In`
    SecuritySchemeIn { Cookie => "cookie", Header => "header", Query => "query" }
);
string_enum!(
    /// `PathItem.HttpMethod` (`toString()` is the constant name)
    HttpMethod { Post => "POST", Get => "GET", Put => "PUT", Patch => "PATCH", Delete => "DELETE", Head => "HEAD", Options => "OPTIONS", Trace => "TRACE" }
);

/// Which `Parameter` subclass an instance is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ParameterKind {
    /// `Parameter`: no subclass, `in` unset.
    #[default]
    Parameter,
    /// `QueryParameter`: `in: query`.
    Query,
    /// `PathParameter`: `in: path`, required.
    Path,
    /// `HeaderParameter`: `in: header`.
    Header,
    /// `CookieParameter`: `in: cookie`.
    Cookie,
}

impl ParameterKind {
    /// `getClass().getName()` of the subclass.
    pub fn class_name(&self) -> &'static str {
        match self {
            ParameterKind::Parameter => "io.swagger.v3.oas.models.parameters.Parameter",
            ParameterKind::Query => "io.swagger.v3.oas.models.parameters.QueryParameter",
            ParameterKind::Path => "io.swagger.v3.oas.models.parameters.PathParameter",
            ParameterKind::Header => "io.swagger.v3.oas.models.parameters.HeaderParameter",
            ParameterKind::Cookie => "io.swagger.v3.oas.models.parameters.CookieParameter",
        }
    }
}

/// `addExtension(name, value)` shared rule: only `x-` names are kept (and `addExtension31` also drops
/// `x-oas-`/`x-oai-`).
pub(crate) fn extension_allowed(name: &str) -> bool {
    !name.is_empty() && name.starts_with("x-")
}

pub(crate) fn extension31_allowed(name: &str) -> bool {
    !(name.starts_with("x-oas-") || name.starts_with("x-oai-"))
}

pub(crate) fn put_extension(slot: &mut Option<JMap<JavaObject>>, name: &str, value: JavaObject) {
    slot.get_or_insert_with(JMap::new).put(name, value);
}

/// `set$ref` rule shared by Header/Example/Link/Callback/RequestBody/ApiResponse/SecurityScheme:
/// a bare name (no `.` and no `/`) is prefixed with the component path.
pub(crate) fn component_ref(r: Option<String>, prefix: &str) -> Option<String> {
    r.map(|r| if !r.contains('.') && !r.contains('/') { format!("{prefix}{r}") } else { r })
}
