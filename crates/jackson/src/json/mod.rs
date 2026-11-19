//! JSON reading (`ReaderBasedJsonParser` + `JsonNodeDeserializer`) and writing (`WriterBasedJsonGenerator`,
//! optionally with `DefaultPrettyPrinter`).

pub mod read;
pub mod write;

pub use read::{ReadFeatures, is_java_identifier_part, is_java_identifier_start, read_tree, read_tree_bytes};
pub use write::{JsonWriter, WriteFeatures};
