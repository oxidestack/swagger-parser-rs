//! The parts of Jackson 2.22 that swagger-parser and swagger-core rely on, reproduced byte-for-byte:
//!
//! * [`JsonNode`] — the `com.fasterxml.jackson.databind.JsonNode` tree model (node *classes* are kept apart:
//!   an `IntNode(1)` is not a `LongNode(1)`, exactly as in Jackson);
//! * [`JavaObject`] — the "untyped" Java values Jackson binds to `Object.class` (`LinkedHashMap`, `ArrayList`,
//!   `Integer`, `Long`, `BigInteger`, `Double`, `BigDecimal`, `String`, `Boolean`, …) plus the handful of JDK
//!   value types swagger models hold (`java.util.Date`, `OffsetDateTime`, `UUID`, `byte[]`) and opaque POJOs;
//! * [`BigDecimal`] — `java.math.BigDecimal` value semantics (`toString`, `toPlainString`, `equals`, `compareTo`);
//! * [`JsonGenerator`] — the streaming writer abstraction model serializers write to, with JSON (compact and
//!   `DefaultPrettyPrinter`) and YAML (`YAMLGenerator`) implementations;
//! * JSON parsing (`JsonFactory` with `StreamReadFeature.STRICT_DUPLICATE_DETECTION` optional) with Jackson's
//!   exact exception messages, and YAML tree reading (`YAMLParser` scalar typing on top of SnakeYAML events).

#![warn(missing_docs)]

pub mod base64;
pub mod bigdecimal;
pub mod convert;
pub mod generator;
pub mod javanum;
pub mod json;
pub mod node;
pub mod object;
pub mod yaml;
pub mod yamlobj;

pub use bigdecimal::BigDecimal;
pub use generator::JsonGenerator;
pub use node::{JsonNode, ObjectNode};
pub use object::{JavaObject, Pojo};

/// A Jackson exception (`JsonProcessingException` and subclasses, `IOException`, …): the Java class name
/// and the exact `getMessage()` text (which includes the location suffix for parse errors).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JacksonError {
    /// Fully-qualified Java exception class, e.g. `com.fasterxml.jackson.core.JsonParseException`.
    pub class: String,
    /// `getMessage()`
    pub message: String,
}

impl std::fmt::Display for JacksonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for JacksonError {}

impl From<JacksonError> for java_rt::JavaException {
    fn from(e: JacksonError) -> Self {
        java_rt::JavaException::new(&e.class, e.message)
    }
}
