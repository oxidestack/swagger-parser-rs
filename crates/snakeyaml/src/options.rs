//! `LoaderOptions` and the style enums of `DumperOptions`.

/// `LoaderOptions` with SnakeYAML 2.6 defaults.
#[derive(Clone, Debug)]
pub struct LoaderOptions {
    /// `setAllowDuplicateKeys`: when `false`, a repeated mapping key is a `DuplicateKeyException`; when `true`, the last value wins.
    pub allow_duplicate_keys: bool,
    /// `setWarnOnDuplicateKeys`: log a warning for a repeated key when duplicates are allowed.
    pub warn_on_duplicate_keys: bool,
    /// `setWrappedToRootException`: wrap exceptions other than `YAMLException` into one.
    pub wrapped_to_root_exception: bool,
    /// `setMaxAliasesForCollections`: the most aliases to collections a document may use (the "billion laughs" guard).
    pub max_aliases_for_collections: usize,
    /// `setAllowRecursiveKeys`: allow a mapping key that contains the mapping itself.
    pub allow_recursive_keys: bool,
    /// `setProcessComments`: emit comment events and nodes.
    pub process_comments: bool,
    /// `setEnumCaseSensitive`: match enum constant names case-sensitively.
    pub enum_case_sensitive: bool,
    /// `setNestingDepthLimit`: the deepest collection nesting the parser accepts.
    pub nesting_depth_limit: usize,
    /// `setCodePointLimit`: the most code points a single document may contain.
    pub code_point_limit: usize,
    /// `setMergeOnCompose`: resolve `<<` merge keys while composing nodes rather than while constructing objects.
    pub merge_on_compose: bool,
}

impl Default for LoaderOptions {
    fn default() -> Self {
        LoaderOptions {
            allow_duplicate_keys: true,
            warn_on_duplicate_keys: true,
            wrapped_to_root_exception: false,
            max_aliases_for_collections: 50,
            allow_recursive_keys: false,
            process_comments: false,
            enum_case_sensitive: true,
            nesting_depth_limit: 50,
            code_point_limit: 3_145_728,
            merge_on_compose: false,
        }
    }
}

/// `DumperOptions.ScalarStyle`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScalarStyle {
    /// `"` double-quoted.
    DoubleQuoted,
    /// `'` single-quoted.
    SingleQuoted,
    /// `|` literal block.
    Literal,
    /// `>` folded block.
    Folded,
    /// `JSON_SCALAR_STYLE`: plain when the value can be written plain, double-quoted otherwise.
    JsonScalarStyle,
    /// Plain (unquoted).
    Plain,
}

impl ScalarStyle {
    /// `getChar()`: the style's indicator character, `None` for plain.
    pub fn char(self) -> Option<char> {
        match self {
            ScalarStyle::DoubleQuoted => Some('"'),
            ScalarStyle::SingleQuoted => Some('\''),
            ScalarStyle::Literal => Some('|'),
            ScalarStyle::Folded => Some('>'),
            ScalarStyle::JsonScalarStyle => Some('J'),
            ScalarStyle::Plain => None,
        }
    }
}

/// `DumperOptions.FlowStyle`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowStyle {
    /// `[a, b]` / `{a: b}` flow style.
    Flow,
    /// Block (indented) style.
    Block,
    /// `AUTO`: no preference; a representer picks flow for collections of scalars, the emitter writes it as block.
    Auto,
}

/// `DumperOptions.Version`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Version {
    /// YAML 1.0.
    V1_0,
    /// YAML 1.1.
    V1_1,
}
