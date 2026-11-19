//! A port of SnakeYAML 2.6: `StreamReader` → `ScannerImpl` → `ParserImpl` → `Composer` → `SafeConstructor`,
//! with the exact exception texts, plus the event API used by the Jackson YAML module.

#![warn(missing_docs)]

pub mod emitter;
pub mod error;
pub mod events;
pub mod loader;
pub mod nodes;
pub mod object;
pub mod options;
pub mod parser;
pub mod reader;
pub mod resolver;
pub mod scanner;

pub use error::{Mark, YamlError};
pub use loader::Yaml;
pub use object::{YamlList, YamlMap, YamlObject};
pub use options::LoaderOptions;
pub use resolver::Resolver;
