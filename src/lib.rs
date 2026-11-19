//! A port of swagger-parser 2.1.48: OpenAPI 3.x / Swagger 2.0 parsing, reference resolution and conversion,
//! including the swagger-core 2.2.53 v3 model and its Jackson (de)serialization.
//!
//! A [`core::SwaggerParserExtension`] reads a document from a location or from its text into a
//! [`core::SwaggerParseResult`]: the [`models`] tree (`OpenAPI`) and the messages swagger-parser reports.
//! [`models::json_string`] writes the model as swagger-core's `Json.mapper()` does:
//!
//! ```
//! use swagger_parser::core::{ParseOptions, SwaggerParserExtension};
//! use swagger_parser::parser::OpenAPIV3Parser;
//!
//! let spec = r#"
//! openapi: 3.0.1
//! info: {title: Pets, version: "1.0"}
//! paths:
//!   /pets:
//!     get:
//!       responses:
//!         "200": {description: ok}
//! "#;
//! let options = ParseOptions { resolve: true, ..Default::default() };
//! let result = OpenAPIV3Parser::new().read_contents(spec, None, Some(&options));
//! let openapi = result.openapi.expect("a valid document");
//! let json = swagger_parser::models::json_string(&openapi).expect("serialisable");
//! assert!(json.starts_with(r#"{"openapi":"3.0.1","info":{"title":"Pets","version":"1.0"}"#));
//! ```
//!
//! [`parser_extensions`] is every extension of the full distribution, `OpenAPIV3Parser` then the Swagger 2.0
//! converter ([`converter`]), and [`OpenAPIParser`] tries them in turn as `io.swagger.parser.OpenAPIParser` does.

#![warn(missing_docs)]

pub mod java;
pub mod models;
pub mod core;
pub mod parser;
pub mod v2;
pub mod converter;
pub mod compat;
pub mod openapi_parser;

pub use openapi_parser::{parser_extensions, OpenAPIParser};
pub mod cli;
