//! jackson-dataformat-yaml 2.22.1: `YAMLParser` (tree reading) and `YAMLGenerator`.

pub mod read;
pub mod write;

pub use read::{YamlReadFeatures, read_tree};
pub use write::{YamlGeneratorFeatures, YamlWriter};
