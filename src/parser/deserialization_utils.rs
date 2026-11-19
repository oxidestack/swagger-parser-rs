//! `io.swagger.v3.parser.util.DeserializationUtils`: text → JSON tree (JSON via Jackson, YAML via SnakeYAML
//! with swagger-parser's limits, cycle checks and fallbacks).

use crate::core::{ParseOptions, SwaggerParseResult};
use crate::java::{JResult, JavaException};
use jackson::json::{ReadFeatures, read_tree as read_json_tree};
use jackson::yaml::{YamlReadFeatures, read_tree as read_yaml_tree_jackson};
use jackson::yamlobj::{yaml_convert_to_node, yaml_to_tree};
use jackson::{JacksonError, JsonNode};
use parking_lot::RwLock;
use snakeyaml::error::{YamlErrorKind, DUPLICATE_KEY_EXCEPTION};
use snakeyaml::nodes::NodeValue;
use snakeyaml::{LoaderOptions, Resolver, Yaml, YamlObject};
use std::collections::HashMap;
use std::sync::OnceLock;

/// `DeserializationUtils.Options` (defaults as with no system properties set).
#[derive(Clone, Debug)]
pub struct Options {
    /// `maxYamlDepth`: the deepest YAML nesting accepted. Default 2000.
    pub max_yaml_depth: i64,
    /// `maxYamlReferences`: how many times one YAML node may be referenced (through aliases) before parsing fails. Default 10 000 000.
    pub max_yaml_references: i64,
    /// `validateYamlInput`: run the depth, reference and cycle checks on parsed YAML. Default `true`.
    pub validate_yaml_input: bool,
    /// `yamlCycleCheck`: fail on a YAML alias cycle. Default `true`.
    pub yaml_cycle_check: bool,
    /// `maxYamlCodePoints`: the largest YAML document accepted, in code points. Default 3 MiB.
    pub max_yaml_code_points: usize,
    /// `maxYamlAliasesForCollections`: SnakeYAML's limit on aliases to collections. Default `Integer.MAX_VALUE`.
    pub max_yaml_aliases_for_collections: usize,
    /// `yamlAllowRecursiveKeys`: SnakeYAML's `allowRecursiveKeys`. Default `true`.
    pub yaml_allow_recursive_keys: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            max_yaml_depth: 2000,
            max_yaml_references: 10_000_000,
            validate_yaml_input: true,
            yaml_cycle_check: true,
            max_yaml_code_points: 3 * 1024 * 1024,
            max_yaml_aliases_for_collections: i32::MAX as usize,
            yaml_allow_recursive_keys: true,
        }
    }
}

fn options_cell() -> &'static RwLock<Options> {
    static O: OnceLock<RwLock<Options>> = OnceLock::new();
    O.get_or_init(|| RwLock::new(Options::default()))
}

/// `DeserializationUtils.getOptions()` (a process-wide mutable singleton, as in Java).
pub fn get_options() -> Options {
    options_cell().read().clone()
}

/// Mutates the process-wide options (`getOptions().setXxx(...)`).
pub fn update_options(f: impl FnOnce(&mut Options)) {
    f(&mut options_cell().write());
}

/// `isJson(contents)`
pub fn is_json(contents: &str) -> bool {
    contents.trim_start_matches(|c: char| c <= ' ').starts_with('{')
}

/// `buildLoaderOptions()`
pub fn build_loader_options() -> LoaderOptions {
    let o = get_options();
    LoaderOptions {
        max_aliases_for_collections: o.max_yaml_aliases_for_collections,
        allow_recursive_keys: o.yaml_allow_recursive_keys,
        allow_duplicate_keys: false,
        code_point_limit: o.max_yaml_code_points,
        ..LoaderOptions::default()
    }
}

/// `JsonProcessingException.getOriginalMessage()`: the message without Jackson's location suffix.
pub fn original_message(e: &JacksonError) -> String {
    match e.message.find("\n at [Source") {
        Some(i) => e.message[..i].to_string(),
        None => e.message.clone(),
    }
}

/// `ObjectMapperFactory.createJson().readTree` (strict duplicate detection)
pub fn read_json_strict(s: &str) -> Result<JsonNode, JacksonError> {
    read_json_tree(s, ReadFeatures { strict_duplicate_detection: true, ..Default::default() })
}

/// `deserializeIntoTree(contents, fileOrHost)`
pub fn deserialize_into_tree_simple(contents: &str, uri: Option<&str>) -> JResult<JsonNode> {
    deserialize_into_tree(contents, uri, None, &mut SwaggerParseResult::new())
}

/// `deserializeIntoTree(contents, uri, parseOptions, deserializationUtilsResult)`
pub fn deserialize_into_tree(contents: &str, uri: Option<&str>, options: Option<&ParseOptions>, result: &mut SwaggerParseResult) -> JResult<JsonNode> {
    if let Some(h) = crate::parser::hooks::current().deserialize_into_tree
        && let Some(r) = h(contents, uri, options, result)
    {
        return r;
    }
    let r = if is_json(contents) { read_json_strict(contents).map_err(JavaException::from) } else { read_yaml_tree(contents, options, result) };
    r.map_err(|e| {
        if e.class.starts_with("com.fasterxml.jackson") {
            JavaException::new(
                "java.lang.RuntimeException",
                format!("An exception was thrown while trying to deserialize the contents of {} into a JsonNode tree", uri.unwrap_or("null")),
            )
            .with_cause(e)
        } else {
            e
        }
    })
}

/// `readYamlTree(contents)`
pub fn read_yaml_tree_simple(contents: &str) -> JResult<JsonNode> {
    read_yaml_tree(contents, None, &mut SwaggerParseResult::new())
}

fn swagger_core_yaml_read(contents: &str) -> Result<JsonNode, JacksonError> {
    read_yaml_tree_jackson(contents, YamlReadFeatures::swagger_core())
}

/// `readYamlTree(contents, parseOptions, deserializationUtilsResult)`
pub fn read_yaml_tree(contents: &str, options: Option<&ParseOptions>, result: &mut SwaggerParseResult) -> JResult<JsonNode> {
    if options.is_some_and(|o| o.legacy_yaml_deserialization) {
        let yaml = Yaml::new(build_loader_options(), Resolver::default());
        let o = yaml.load(contents).map_err(yaml_to_java)?;
        return yaml_convert_to_node(&o).map_err(JavaException::from);
    }
    let mut messages = Vec::new();
    let r = read_yaml_tree_with(contents, build_loader_options(), false, &mut messages);
    for m in messages {
        result.message(m);
    }
    r
}

/// swagger-parser 1.0.76 `DeserializationUtils.readYamlTree(contents, errorOutput)`: the same algorithm
/// with SnakeYAML's default `allowDuplicateKeys = true`, converting the loaded objects with swagger-core
/// 1.x `Json.mapper().convertValue(o, JsonNode.class)` (null map values dropped); messages go to
/// `messages`.
pub fn read_yaml_tree_v1(contents: &str, messages: &mut Vec<String>) -> JResult<JsonNode> {
    let lo = LoaderOptions { allow_duplicate_keys: true, ..build_loader_options() };
    read_yaml_tree_with(contents, lo, true, messages)
}

fn read_yaml_tree_with(contents: &str, loader_options: LoaderOptions, convert_value: bool, messages: &mut Vec<String>) -> JResult<JsonNode> {
    let opts = get_options();
    let attempt = (|| -> JResult<JsonNode> {
        let o = load_snake_yaml(contents, &opts, loader_options.clone())?;
        if opts.validate_yaml_input && exceeds_limits(&o, None, 0, &mut HashMap::new(), Some(messages), &opts) {
            return swagger_core_yaml_read(contents).map_err(JavaException::from);
        }
        if convert_value {
            return yaml_convert_to_node(&o).map_err(JavaException::from);
        }
        match yaml_to_tree(&o) {
            Ok(n) => Ok(n),
            Err(_) => yaml_convert_to_node(&o).map_err(JavaException::from),
        }
    })();
    match attempt {
        Ok(n) => Ok(n),
        Err(e) => {
            messages.push(e.get_message().unwrap_or("null").to_string());
            match swagger_core_yaml_read(contents) {
                Ok(n) => Ok(n),
                Err(_) => Err(JavaException::runtime_wrapping(e)),
            }
        }
    }
}

fn yaml_to_java(e: snakeyaml::YamlError) -> JavaException {
    JavaException::new(e.class, e.message())
}

/// `SnakeException` (`DeserializationUtils$SnakeException`)
fn snake_exception(msg: String) -> JavaException {
    JavaException::new("io.swagger.v3.parser.util.DeserializationUtils$SnakeException", msg)
}

/// `buildSnakeYaml(constructor).load(contents)` with `CustomSnakeYamlConstructor` when validating.
fn load_snake_yaml(contents: &str, opts: &Options, loader_options: LoaderOptions) -> JResult<YamlObject> {
    let yaml = Yaml::new(loader_options, Resolver::swagger_custom());
    if !opts.validate_yaml_input {
        let node = yaml.compose(contents).map_err(yaml_to_java)?;
        return match &node {
            Some(n) if n.tag() != snakeyaml::nodes::TAG_NULL => yaml.construct(Some(n)).map_err(yaml_to_java),
            _ => Ok(YamlObject::Null),
        };
    }
    // CustomSnakeYamlConstructor.getSingleData
    let wrap = |e: snakeyaml::YamlError| -> JavaException {
        if e.class == DUPLICATE_KEY_EXCEPTION {
            let problem = e.problem().unwrap_or("").replace("found duplicate key", "Duplicate field");
            return snake_exception(problem);
        }
        let msg = match &e.kind {
            YamlErrorKind::Marked(_) | YamlErrorKind::Plain(_) | YamlErrorKind::Reader { .. } => e.message(),
        };
        snake_exception(format!("{msg}; Max code points: {}", opts.max_yaml_code_points))
    };
    let node = yaml.compose(contents).map_err(wrap)?;
    let Some(node) = node else { return Ok(YamlObject::Null) };
    if matches!(node.0.read().value, NodeValue::Mapping(_)) && !check_node(&node, 0, opts) {
        return Err(snake_exception(format!("yaml tree depth exceeds max {}", opts.max_yaml_depth)));
    }
    yaml.construct(Some(&node)).map_err(wrap)
}

/// `CustomSnakeYamlConstructor.checkNode`: depth over mapping *keys* only.
fn check_node(node: &snakeyaml::nodes::Node, depth: i64, opts: &Options) -> bool {
    if depth > opts.max_yaml_depth {
        return false;
    }
    for (k, _) in node.mapping_children() {
        if matches!(k.0.read().value, NodeValue::Mapping(_)) && !check_node(&k, depth + 1, opts) {
            return false;
        }
    }
    true
}

fn children(o: &YamlObject) -> Vec<YamlObject> {
    match o {
        YamlObject::Map(m) => {
            let e = m.entries();
            let mut v: Vec<YamlObject> = e.iter().map(|(k, _)| k.clone()).collect();
            v.extend(e.into_iter().map(|(_, v)| v));
            v
        }
        YamlObject::List(l) => l.items(),
        _ => Vec::new(),
    }
}

/// `o instanceof List || o instanceof Map` (a `LinkedHashSet` from `!!set` is neither)
fn is_container(o: &YamlObject) -> bool {
    matches!(o, YamlObject::Map(_) | YamlObject::List(_))
}

/// `exceedsLimits` (the visited map is an `IdentityHashMap` of container → reference count)
fn exceeds_limits(o: &YamlObject, parent: Option<&YamlObject>, depth: i64, visited: &mut HashMap<usize, i64>, mut out: Option<&mut Vec<String>>, opts: &Options) -> bool {
    if !is_container(o) {
        return false;
    }
    if depth > opts.max_yaml_depth {
        let msg = format!(
            "snake-yaml result exceeds max depth {}; threshold can be increased if needed by setting system property `maxYamlDepth` to a higher value.",
            opts.max_yaml_depth
        );
        if let Some(r) = out.as_deref_mut() {
            r.push(msg);
        }
        return true;
    }
    let id = o.identity().unwrap();
    if let Some(count) = visited.get(&id).copied() {
        let target = parent.unwrap_or(o);
        if opts.yaml_cycle_check && has_reference(o, target, 0, &mut HashMap::new(), out.as_deref_mut(), opts) {
            return true;
        }
        if count > opts.max_yaml_references {
            let msg = format!(
                "snake-yaml result exceeds max references {}; threshold can be increased if needed by setting system property `maxYamlReferences` to a higher value.",
                opts.max_yaml_references
            );
            if let Some(r) = out.as_deref_mut() {
                r.push(msg);
            }
            return true;
        }
        visited.insert(id, count + 1);
    } else {
        visited.insert(id, 1);
    }
    match o {
        YamlObject::Map(m) => {
            let entries = m.entries();
            for (k, _) in &entries {
                if exceeds_limits(k, Some(o), depth + 1, visited, out.as_deref_mut(), opts) {
                    return true;
                }
            }
            for (_, v) in &entries {
                if exceeds_limits(v, Some(o), depth + 1, visited, out.as_deref_mut(), opts) {
                    return true;
                }
            }
        }
        YamlObject::List(l) => {
            for v in l.items() {
                if exceeds_limits(&v, Some(o), depth + 1, visited, out.as_deref_mut(), opts) {
                    return true;
                }
            }
        }
        _ => {}
    }
    false
}

/// `hasReference`
fn has_reference(o: &YamlObject, target: &YamlObject, depth: i64, visited: &mut HashMap<usize, i64>, mut out: Option<&mut Vec<String>>, opts: &Options) -> bool {
    if !is_container(o) || !is_container(target) {
        return false;
    }
    if depth > opts.max_yaml_depth {
        let msg = format!(
            "snake-yaml result exceeds max depth {}; threshold can be increased if needed by setting system property `maxYamlDepth` to a higher value.",
            opts.max_yaml_depth
        );
        if let Some(r) = out.as_deref_mut() {
            r.push(msg);
        }
        return true;
    }
    let target_id = target.identity().unwrap();
    if visited.contains_key(&target_id) {
        return false;
    }
    visited.insert(o.identity().unwrap(), 1);
    for v in children(o) {
        if v.identity() == Some(target_id) {
            if let Some(r) = out.as_deref_mut() {
                r.push("detected cycle in snake-yaml result; cycle check can be disabled by setting system property `yamlCycleCheck` to false.".to_string());
            }
            return true;
        }
        if has_reference(&v, target, depth + 1, visited, out.as_deref_mut(), opts) {
            return true;
        }
    }
    false
}
