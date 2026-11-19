//! The 1.1 → 1.2 migrations of swagger-compat-spec-parser (`ResourceListingMigrator`,
//! `ApiDeclarationMigrator` and the migrators they compose), including the JSON Patch operations and
//! `MutableJsonTree` they are built on.

use crate::java::net::JavaUri;
use crate::java::{JResult, JavaException};
use jackson::JsonNode;

/// A failed migration step: a `SwaggerMigrationException` (caught by `SwaggerJsonMigrator`, which then
/// keeps the unmigrated node) or any other exception (which escapes it).
#[derive(Debug)]
pub enum MigrateErr {
    Migration,
    Other(JavaException),
}

type MResult<T> = Result<T, MigrateErr>;

fn other(e: JavaException) -> MigrateErr {
    MigrateErr::Other(e)
}

fn pointer_tokens(p: &str) -> Vec<String> {
    p.split('/').skip(1).map(|t| t.replace("~1", "/").replace("~0", "~")).collect()
}

fn node_at<'a>(node: &'a JsonNode, tokens: &[String]) -> Option<&'a JsonNode> {
    let mut cur = node;
    for t in tokens {
        cur = match cur {
            JsonNode::Object(o) => o.get(t)?,
            JsonNode::Array(a) => a.get(t.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(cur)
}

fn node_at_mut<'a>(node: &'a mut JsonNode, tokens: &[String]) -> Option<&'a mut JsonNode> {
    let mut cur = node;
    for t in tokens {
        cur = match cur {
            JsonNode::Object(_) => cur.as_object_mut()?.get_mut(t)?,
            JsonNode::Array(_) => {
                let i = t.parse::<usize>().ok()?;
                cur.as_array_mut()?.get_mut(i)?
            }
            _ => return None,
        };
    }
    Some(cur)
}

/// The JSON Patch operations the migrations use.
#[derive(Clone, Debug)]
enum PatchOp {
    Add(&'static str, JsonNode),
    Remove(&'static str),
    Replace(&'static str, JsonNode),
    Move(&'static str, &'static str),
}

fn patch_add(node: &mut JsonNode, path: &str, value: JsonNode) -> MResult<()> {
    let tokens = pointer_tokens(path);
    let Some((last, parent_tokens)) = tokens.split_last() else {
        *node = value;
        return Ok(());
    };
    let parent = node_at_mut(node, parent_tokens).ok_or(MigrateErr::Migration)?;
    match parent {
        JsonNode::Object(_) => {
            parent.as_object_mut().unwrap().insert(last.clone(), value);
            Ok(())
        }
        JsonNode::Array(_) => {
            let a = parent.as_array_mut().unwrap();
            if last == "-" {
                a.push(value);
                return Ok(());
            }
            let i: usize = last.parse().map_err(|_| MigrateErr::Migration)?;
            if i > a.len() {
                return Err(MigrateErr::Migration);
            }
            a.insert(i, value);
            Ok(())
        }
        _ => Err(MigrateErr::Migration),
    }
}

fn patch_remove(node: &mut JsonNode, path: &str) -> MResult<JsonNode> {
    let tokens = pointer_tokens(path);
    if node_at(node, &tokens).is_none() {
        return Err(MigrateErr::Migration);
    }
    let Some((last, parent_tokens)) = tokens.split_last() else {
        return Ok(std::mem::replace(node, JsonNode::Missing));
    };
    let parent = node_at_mut(node, parent_tokens).unwrap();
    match parent {
        JsonNode::Object(_) => Ok(parent.as_object_mut().unwrap().shift_remove(last).unwrap()),
        _ => {
            let i: usize = last.parse().unwrap();
            Ok(parent.as_array_mut().unwrap().remove(i))
        }
    }
}

/// `JsonPatch.apply(input)` (fge json-patch semantics for add/remove/replace/move)
fn apply_patch(input: &JsonNode, ops: &[PatchOp]) -> MResult<JsonNode> {
    let mut node = input.clone();
    for op in ops {
        match op {
            PatchOp::Add(p, v) => patch_add(&mut node, p, v.clone())?,
            PatchOp::Remove(p) => {
                patch_remove(&mut node, p)?;
            }
            PatchOp::Replace(p, v) => {
                let tokens = pointer_tokens(p);
                match node_at_mut(&mut node, &tokens) {
                    Some(t) => *t = v.clone(),
                    None => return Err(MigrateErr::Migration),
                }
            }
            PatchOp::Move(from, to) => {
                if from == to {
                    continue;
                }
                let moved = patch_remove(&mut node, from)?;
                patch_add(&mut node, to, moved)?;
            }
        }
    }
    Ok(node)
}

fn text(s: &str) -> JsonNode {
    JsonNode::text(s)
}

/// `/patches/v1.1/dataType.json`, falling back to `DEFAULT_PATCH` (`dataType` becomes `$ref`)
fn data_type_patch(data_type: &str) -> Vec<PatchOp> {
    use PatchOp::*;
    match data_type {
        "boolean" | "string" | "File" => vec![Move("/dataType", "/type")],
        "int" => vec![Remove("/dataType"), Add("/type", text("integer")), Add("/format", text("int32"))],
        "long" => vec![Remove("/dataType"), Add("/type", text("integer")), Add("/format", text("int64"))],
        "float" | "double" => vec![Move("/dataType", "/format"), Add("/type", text("number"))],
        "byte" => vec![Move("/dataType", "/format"), Add("/type", text("string"))],
        "Date" => vec![Remove("/dataType"), Add("/type", text("string")), Add("/format", text("date-time"))],
        "List" | "Array" => vec![Remove("/dataType"), Add("/type", text("array"))],
        "Set" => vec![Remove("/dataType"), Add("/type", text("array")), Add("/uniqueItems", JsonNode::Boolean(true))],
        _ => vec![Move("/dataType", "/$ref")],
    }
}

fn class_cast_object_node(n: &JsonNode) -> JavaException {
    JavaException::class_cast(&format!("com.fasterxml.jackson.databind.node.{}", n.node_class_name()), "com.fasterxml.jackson.databind.node.ObjectNode")
}

/// `SwaggerMigrators.membersToString(names…)`
fn members_to_string(input: &JsonNode, names: &[&str]) -> MResult<JsonNode> {
    if !input.is_object() {
        return Err(other(class_cast_object_node(input)));
    }
    let mut ret = input.clone();
    for name in names {
        let Some(n) = input.get(name) else { continue };
        if n.is_container_node() {
            return Err(MigrateErr::Migration);
        }
        if !n.is_textual() {
            let t = n.as_text();
            ret.as_object_mut().unwrap().insert(name.to_string(), text(&t));
        }
    }
    Ok(ret)
}

/// `SwaggerMigrators.renameMember(from, to)`
fn rename_member(input: &JsonNode, from: &str, to: &str) -> MResult<JsonNode> {
    if input.get(from).is_none() {
        return Ok(input.clone());
    }
    if input.get(to).is_some() {
        return Err(MigrateErr::Migration);
    }
    let mut ret = input.clone();
    let o = ret.as_object_mut().ok_or_else(|| other(class_cast_object_node(input)))?;
    let v = o.get(from).cloned().unwrap();
    o.insert(to.to_string(), v);
    o.shift_remove(from);
    Ok(ret)
}

/// `MutableJsonTree`: a base node and a current pointer
struct MutableJsonTree {
    base: JsonNode,
    pointer: Vec<String>,
}

impl MutableJsonTree {
    fn new(node: &JsonNode) -> MutableJsonTree {
        MutableJsonTree { base: node.clone(), pointer: Vec::new() }
    }

    /// `setPointer`: the pointed-to node must exist (an `IllegalArgumentException` otherwise)
    fn set_pointer(&mut self, tokens: &[&str]) -> MResult<()> {
        let tokens: Vec<String> = tokens.iter().map(|s| s.to_string()).collect();
        if node_at(&self.base, &tokens).is_none() {
            return Err(other(JavaException::illegal_argument(format!(
                "base node has no element at pointer {}",
                tokens.iter().map(|t| format!("/{t}")).collect::<String>()
            ))));
        }
        self.pointer = tokens;
        Ok(())
    }

    fn current(&self) -> &JsonNode {
        node_at(&self.base, &self.pointer).unwrap()
    }

    /// `applyMigrator(migrator)`
    fn apply(&mut self, f: impl FnOnce(&JsonNode) -> MResult<JsonNode>) -> MResult<()> {
        let parent_tokens: Vec<String> = if self.pointer.is_empty() { Vec::new() } else { self.pointer[..self.pointer.len() - 1].to_vec() };
        if !node_at(&self.base, &parent_tokens).is_some_and(|p| p.is_object()) {
            return Err(MigrateErr::Migration);
        }
        let patched = f(self.current())?;
        if self.pointer.is_empty() {
            self.base = patched;
        } else {
            let name = self.pointer.last().unwrap().clone();
            node_at_mut(&mut self.base, &parent_tokens).unwrap().as_object_mut().unwrap().insert(name, patched);
        }
        Ok(())
    }

    /// `applyMigratorToElements(migrator)`
    fn apply_to_elements(&mut self, mut f: impl FnMut(&JsonNode) -> MResult<JsonNode>) -> MResult<()> {
        let Some(elements) = self.current().as_array().cloned() else { return Err(MigrateErr::Migration) };
        let mut transformed = Vec::with_capacity(elements.len());
        for e in &elements {
            transformed.push(f(e)?);
        }
        let pointer = self.pointer.clone();
        *node_at_mut(&mut self.base, &pointer).unwrap().as_array_mut().unwrap() = transformed;
        Ok(())
    }
}

/// `V11AllowableValuesMigrator`
fn allowable_values(input: &JsonNode) -> MResult<JsonNode> {
    if input.get("allowableValues").is_none() {
        return Ok(input.clone());
    }
    let Some(value_type) = input.get("allowableValues").and_then(|a| a.get("valueType")) else {
        return Err(other(JavaException::without_message("java.lang.NullPointerException")));
    };
    if value_type.text_value() == Some("LIST") {
        apply_patch(input, &[PatchOp::Move("/allowableValues/values", "/enum"), PatchOp::Remove("/allowableValues")])
    } else {
        Ok(input.clone())
    }
}

/// `V11TypeMigrator`
fn data_type(input: &JsonNode) -> MResult<JsonNode> {
    let migrated = match input.get("dataType") {
        None => input.clone(),
        Some(JsonNode::Text(dt)) => apply_patch(input, &data_type_patch(dt))?,
        Some(_) => return Err(MigrateErr::Migration),
    };
    // postMigrate: `File` becomes a form parameter consuming multipart/form-data
    if migrated.get("type").and_then(|t| t.text_value()) == Some("File") {
        return apply_patch(&migrated, &[PatchOp::Add("/paramType", text("form")), PatchOp::Add("/consumes", text("multipart/form-data"))]);
    }
    Ok(migrated)
}

/// `OperationParametersMigrator`
fn operation_parameters(input: &JsonNode) -> MResult<JsonNode> {
    data_type(&allowable_values(input)?)
}

/// `ApiObjectMigrator`
fn api_object(input: &JsonNode) -> MResult<JsonNode> {
    let mut on = input.clone();
    let obj = on.as_object_mut().ok_or_else(|| other(class_cast_object_node(input)))?;
    if obj.get("type").is_none() {
        let mut ty: Option<JsonNode> = None;
        if let Some(JsonNode::Array(rms)) = obj.get("responseMessages") {
            for rm in rms.iter() {
                if let Some(code) = rm.get("code") && code.to_java_string() == "200" {
                    ty = Some(rm.clone());
                }
            }
        }
        let v = match ty.as_ref().and_then(|t| t.get("type")) {
            Some(tt) => tt.clone(),
            None => text("void"),
        };
        obj.insert("type".into(), v);
    }
    if obj.get("parameters").is_none() {
        obj.insert("parameters".into(), JsonNode::array(Vec::new()));
    }
    let mut tree = MutableJsonTree::new(&on);
    tree.apply(|n| rename_member(n, "httpMethod", "method"))?;
    tree.apply(|n| rename_member(n, "errorResponses", "responseMessages"))?;
    if tree.base.get("responseMessages").is_some() {
        tree.set_pointer(&["responseMessages"])?;
        tree.apply_to_elements(|e| rename_member(e, "reason", "message"))?;
    }
    tree.set_pointer(&["parameters"])?;
    tree.apply_to_elements(operation_parameters)?;
    Ok(tree.base)
}

/// `ApiOperationMigrator`
fn api_operation(input: &JsonNode) -> MResult<JsonNode> {
    let mut tree = MutableJsonTree::new(input);
    tree.set_pointer(&["operations"])?;
    tree.apply_to_elements(api_object)?;
    Ok(tree.base)
}

/// `/patches/v1.1/versionChange.json`
fn version_change(input: &JsonNode) -> MResult<JsonNode> {
    apply_patch(input, &[PatchOp::Replace("/swaggerVersion", text("1.2"))])
}

/// `V11ApiDeclarationMigrator`
pub fn api_declaration(input: &JsonNode) -> MResult<JsonNode> {
    let mut tree = MutableJsonTree::new(input);
    tree.apply(|n| members_to_string(n, &["swaggerVersion", "apiVersion"]))?;
    tree.apply(version_change)?;
    tree.set_pointer(&["apis"])?;
    tree.apply_to_elements(api_operation)?;
    Ok(tree.base)
}

/// Guava `InternetDomainName.isValid(name)`
fn is_valid_domain_name(name: &str) -> bool {
    let mut name: String = name.chars().map(|c| if matches!(c, '\u{3002}' | '\u{ff0e}' | '\u{ff61}') { '.' } else { c }).collect();
    if name.ends_with('.') {
        name.pop();
    }
    if name.chars().count() > 253 {
        return false;
    }
    let parts: Vec<&str> = name.split('.').collect();
    if parts.len() > 127 {
        return false;
    }
    let last = parts.len() - 1;
    for (i, part) in parts.iter().enumerate() {
        let len = part.chars().count();
        if !(1..=63).contains(&len) {
            return false;
        }
        if !part.chars().filter(|c| c.is_ascii()).all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            return false;
        }
        let first = part.chars().next().unwrap();
        let end = part.chars().last().unwrap();
        if first == '-' || first == '_' || end == '-' || end == '_' {
            return false;
        }
        if i == last && first.is_ascii_digit() {
            return false;
        }
    }
    true
}

/// `V11ResourceListingMigrator.checkLegalBasePath` (any failure is an `IllegalArgumentException`)
fn check_legal_base_path(input: &str) -> Option<String> {
    let uri = JavaUri::parse(input).ok()?.normalize();
    if !uri.is_absolute() {
        return None;
    }
    let scheme = uri.get_scheme().unwrap_or("");
    if !(scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")) {
        return None;
    }
    if !is_valid_domain_name(uri.get_host().unwrap_or("")) {
        return None;
    }
    if uri.get_path().unwrap_or("").ends_with('/') {
        return None;
    }
    Some(uri.to_java_string())
}

/// `V11ResourceListingMigrator`
pub fn resource_listing(input: &JsonNode) -> MResult<JsonNode> {
    let mut tree = MutableJsonTree::new(input);
    tree.apply(|n| members_to_string(n, &["swaggerVersion", "apiVersion"]))?;
    tree.apply(version_change)?;
    if let Some(JsonNode::Text(base_path)) = input.get("basePath") {
        tree.base.as_object_mut().unwrap().shift_remove("basePath");
        let base_path = check_legal_base_path(base_path).ok_or(MigrateErr::Migration)?;
        tree.set_pointer(&["apis"])?;
        tree.apply_to_elements(|e| {
            if !e.is_object() {
                return Err(MigrateErr::Migration);
            }
            let Some(JsonNode::Text(old)) = e.get("path") else { return Err(MigrateErr::Migration) };
            let mut n = e.clone();
            let p = format!("{base_path}{old}");
            n.as_object_mut().unwrap().insert("path".into(), text(&p));
            Ok(n)
        })?;
    }
    Ok(tree.base)
}

/// `SwaggerJsonMigrator.migrate(builder, node)`: a migration error keeps the node as it was; other
/// exceptions escape.
pub fn run(node: &JsonNode, f: fn(&JsonNode) -> MResult<JsonNode>) -> JResult<JsonNode> {
    match f(node) {
        Ok(n) => Ok(n),
        Err(MigrateErr::Migration) => Ok(node.clone()),
        Err(MigrateErr::Other(e)) => Err(e),
    }
}
