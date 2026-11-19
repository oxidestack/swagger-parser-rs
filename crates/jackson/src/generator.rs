//! `com.fasterxml.jackson.core.JsonGenerator`: the streaming writer model serializers write to.
//!
//! Implementations: compact JSON (`ObjectMapper.writeValueAsString`), JSON with Jackson's
//! `DefaultPrettyPrinter` (`writerWithDefaultPrettyPrinter()`), and YAML (`YAMLGenerator`, configured per
//! [`crate::yaml::YamlGeneratorFeatures`]).

use crate::JacksonError;
use crate::bigdecimal::BigDecimal;
use num_bigint::BigInt;

/// `com.fasterxml.jackson.core.JsonGenerator`: the streaming calls a serializer makes to write one value; each returns `Err` when the underlying writer fails.
pub trait JsonGenerator {
    /// `writeStartObject()`: opens an object.
    fn write_start_object(&mut self) -> Result<(), JacksonError>;
    /// `writeEndObject()`: closes the innermost open object.
    fn write_end_object(&mut self) -> Result<(), JacksonError>;
    /// `writeStartArray()`: opens an array.
    fn write_start_array(&mut self) -> Result<(), JacksonError>;
    /// `writeEndArray()`: closes the innermost open array.
    fn write_end_array(&mut self) -> Result<(), JacksonError>;
    /// `writeFieldName(name)`: the name of the next field of the open object.
    fn write_field_name(&mut self, name: &str) -> Result<(), JacksonError>;
    /// `writeString(text)`: a string value.
    fn write_string(&mut self, value: &str) -> Result<(), JacksonError>;
    /// `writeBoolean(state)`: a boolean value.
    fn write_boolean(&mut self, value: bool) -> Result<(), JacksonError>;
    /// `writeNull()`: a null value.
    fn write_null(&mut self) -> Result<(), JacksonError>;
    /// `writeNumber(short)`.
    fn write_number_i16(&mut self, value: i16) -> Result<(), JacksonError>;
    /// `writeNumber(int)`.
    fn write_number_i32(&mut self, value: i32) -> Result<(), JacksonError>;
    /// `writeNumber(long)`.
    fn write_number_i64(&mut self, value: i64) -> Result<(), JacksonError>;
    /// `writeNumber(BigInteger)`.
    fn write_number_big_integer(&mut self, value: &BigInt) -> Result<(), JacksonError>;
    /// `writeNumber(float)`.
    fn write_number_f32(&mut self, value: f32) -> Result<(), JacksonError>;
    /// `writeNumber(double)`.
    fn write_number_f64(&mut self, value: f64) -> Result<(), JacksonError>;
    /// Honours `WRITE_BIGDECIMAL_AS_PLAIN` as configured on the generator.
    fn write_number_big_decimal(&mut self, value: &BigDecimal) -> Result<(), JacksonError>;
    /// `writeBinary(byte[])` — base64 (MIME-no-linefeeds variant) in JSON, `!!binary` in YAML.
    fn write_binary(&mut self, value: &[u8]) -> Result<(), JacksonError>;
    /// Called by a bean serializer before it writes its `@JsonAnyGetter` entries (`AnyGetterWriter`); only
    /// generators that reorder bean properties care.
    fn mark_any_getter(&mut self) {}
}
