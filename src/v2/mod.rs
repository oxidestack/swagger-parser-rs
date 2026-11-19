//! The Swagger 2.0 stack behind the v2 → v3 converter: swagger-models 1.6.16, the parts of swagger-core
//! 1.6.16 it relies on (`PropertyDeserializer`, the model deserializers, bean (de)serialization) and
//! swagger-parser 1.0.76 (`SwaggerParser`, `SwaggerDeserializer`, `SwaggerResolver`).

pub mod bean;
pub mod deserializer;
pub mod json;
pub mod models;
pub mod property_builder;
pub mod parser;
pub mod resolver;
