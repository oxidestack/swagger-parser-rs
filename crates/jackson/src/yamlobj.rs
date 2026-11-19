//! Serializing SnakeYAML's constructed object graph the way swagger-parser's `DeserializationUtils` does:
//! `JSON_MAPPER_FOR_YAML.writeValueAsString(o)` (a plain `ObjectMapper` + `JavaTimeModule`, ISO dates,
//! plain BigDecimals) followed by `readTree`.

use crate::JacksonError;
use crate::convert::std_date_format;
use crate::generator::JsonGenerator;
use crate::json::{JsonWriter, ReadFeatures, WriteFeatures, read_tree};
use crate::node::JsonNode;
use snakeyaml::YamlObject;
use std::collections::HashSet;

/// Java `toString` of a map key, as Jackson's fallback/std key serializers render it.
fn key_string(k: &YamlObject) -> Result<Option<String>, ()> {
    Ok(Some(match k {
        YamlObject::Null => return Ok(None),
        YamlObject::String(s) => s.to_string(),
        YamlObject::Date(ms) => std_date_format(*ms),
        YamlObject::Bytes(b) => crate::json::write::base64_mime_no_linefeeds(b),
        other => other.to_java_string(),
    }))
}

struct Ctx {
    path: Vec<String>,
    active: HashSet<usize>,
    /// `WRITE_NULL_MAP_VALUES`
    write_null_map_values: bool,
    /// class of the null-key exception
    null_key_class: &'static str,
}

fn write(g: &mut dyn JsonGenerator, o: &YamlObject, ctx: &mut Ctx) -> Result<(), JacksonError> {
    if let Some(id) = o.identity() {
        if !ctx.active.insert(id) {
            return Err(JacksonError { class: "java.lang.StackOverflowError".into(), message: String::new() });
        }
    }
    let r = write_inner(g, o, ctx);
    if let Some(id) = o.identity() {
        ctx.active.remove(&id);
    }
    r
}

fn write_inner(g: &mut dyn JsonGenerator, o: &YamlObject, ctx: &mut Ctx) -> Result<(), JacksonError> {
    match o {
        YamlObject::Null => g.write_null(),
        YamlObject::Boolean(b) => g.write_boolean(*b),
        YamlObject::Integer(v) => g.write_number_i32(*v),
        YamlObject::Long(v) => g.write_number_i64(*v),
        YamlObject::BigInteger(v) => g.write_number_big_integer(v),
        YamlObject::Double(v) => g.write_number_f64(*v),
        YamlObject::String(s) => g.write_string(s),
        YamlObject::Bytes(b) => g.write_binary(b),
        YamlObject::Date(ms) => g.write_string(&std_date_format(*ms)),
        YamlObject::Map(m) => {
            g.write_start_object()?;
            for (k, v) in m.entries() {
                let key = match key_string(&k) {
                    Ok(Some(s)) => s,
                    _ => {
                        let mut chain = ctx.path.clone();
                        chain.push("java.util.LinkedHashMap[\"null\"]".to_string());
                        return Err(JacksonError {
                            class: ctx.null_key_class.into(),
                            message: format!("Null key for a Map not allowed in JSON (use a converting NullKeySerializer?) (through reference chain: {})", chain.join("->")),
                        });
                    }
                };
                if v.is_null() && !ctx.write_null_map_values {
                    continue;
                }
                g.write_field_name(&key)?;
                ctx.path.push(format!("java.util.LinkedHashMap[\"{key}\"]"));
                write(g, &v, ctx)?;
                ctx.path.pop();
            }
            g.write_end_object()
        }
        YamlObject::List(l) | YamlObject::Set(l) => {
            let class = if matches!(o, YamlObject::Set(_)) { "java.util.LinkedHashSet" } else { "java.util.ArrayList" };
            g.write_start_array()?;
            for (i, v) in l.items().iter().enumerate() {
                ctx.path.push(format!("{class}[{i}]"));
                write(g, v, ctx)?;
                ctx.path.pop();
            }
            g.write_end_array()
        }
        YamlObject::Pairs(p) => {
            g.write_start_array()?;
            for (i, (k, v)) in p.iter().enumerate() {
                ctx.path.push(format!("java.util.ArrayList[{i}]"));
                g.write_start_array()?;
                write(g, k, ctx)?;
                write(g, v, ctx)?;
                g.write_end_array()?;
                ctx.path.pop();
            }
            g.write_end_array()
        }
    }
}

/// `JSON_MAPPER_FOR_YAML.writeValueAsString(o)`
pub fn yaml_to_json_string(o: &YamlObject) -> Result<String, JacksonError> {
    let mut w = JsonWriter::new(WriteFeatures { big_decimal_as_plain: true, ..Default::default() });
    let mut ctx = Ctx { path: Vec::new(), active: HashSet::new(), write_null_map_values: true, null_key_class: "com.fasterxml.jackson.databind.JsonMappingException" };
    write(&mut w, o, &mut ctx)?;
    Ok(w.into_string())
}

/// `JSON_MAPPER_FOR_YAML.readTree(JSON_MAPPER_FOR_YAML.writeValueAsString(o))`
pub fn yaml_to_tree(o: &YamlObject) -> Result<JsonNode, JacksonError> {
    let s = yaml_to_json_string(o)?;
    read_tree(&s, ReadFeatures::default())
}

/// `Json.mapper().convertValue(o, JsonNode.class)` (swagger-core mapper: null map values dropped).
pub fn yaml_convert_to_node(o: &YamlObject) -> Result<JsonNode, JacksonError> {
    let mut g = crate::convert::TreeGenerator::new();
    let mut ctx = Ctx { path: Vec::new(), active: HashSet::new(), write_null_map_values: false, null_key_class: "java.lang.IllegalArgumentException" };
    write(&mut g, o, &mut ctx)?;
    // TokenBuffer → tree: binary stays BinaryNode, doubles DoubleNode
    Ok(g.into_node())
}
