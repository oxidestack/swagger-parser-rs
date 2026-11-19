//! `ReferenceUtils`

use crate::java::net::JavaUri;
use crate::java::{JResult, JavaException};
use crate::parser::resolver_cache::unescape_pointer;
use jackson::JsonNode;

fn is_blank(s: Option<&str>) -> bool {
    s.is_none_or(|s| s.trim().is_empty())
}

/// `toBaseURI(uri)`: scheme + decoded scheme-specific part (the fragment dropped)
pub fn to_base_uri(uri: &str) -> JResult<String> {
    let u = JavaUri::parse(uri)?;
    Ok(match u.get_scheme() {
        Some(s) => format!("{s}:{}", u.get_scheme_specific_part()),
        None => u.get_scheme_specific_part(),
    })
}

/// `getFragment(uri)` (decoded)
pub fn get_fragment(uri: &str) -> JResult<Option<String>> {
    Ok(JavaUri::parse(uri)?.get_decoded_fragment())
}

/// `resolve(uri, baseURI)`
pub fn resolve(uri: Option<&str>, base_uri: Option<&str>) -> JResult<String> {
    if is_blank(uri) {
        return base_uri.map(str::to_string).ok_or_else(|| JavaException::npe("baseURI"));
    }
    let Some(base) = base_uri else {
        return Err(JavaException::npe("Cannot invoke \"String.length()\" because \"str\" is null"));
    };
    let b = JavaUri::parse(base)?;
    // URI.resolve(String) = resolve(URI.create(str)): an IllegalArgumentException for a bad child
    let child = JavaUri::parse(uri.unwrap()).map_err(|e| JavaException::new("java.lang.IllegalArgumentException", e.message.clone().unwrap_or_default()).with_cause(e))?;
    Ok(b.resolve(&child).to_java_string())
}

/// `ReferenceUtils.isLocalRef(ref)`: a non-blank reference starting with `#`.
pub fn is_local_ref(r: Option<&str>) -> bool {
    !is_blank(r) && r.unwrap().starts_with('#')
}

/// `ReferenceUtils.isLocalRefToComponents(ref)`: a non-blank reference starting with `#/components`.
pub fn is_local_ref_to_components(r: Option<&str>) -> bool {
    !is_blank(r) && r.unwrap().starts_with("#/components")
}

/// `ReferenceUtils.isAnchorRef(ref)`: a local reference whose fragment is a plain-name anchor (`#name`).
pub fn is_anchor_ref(r: Option<&str>) -> bool {
    if !is_blank(r) && r.unwrap().starts_with('#') {
        return is_anchor(Some(&r.unwrap()[1..]));
    }
    false
}

/// `^[A-Za-z_][A-Za-z_0-9.-]*$`
pub fn is_anchor(r: Option<&str>) -> bool {
    if is_blank(r) {
        return false;
    }
    let mut c = r.unwrap().chars();
    let Some(first) = c.next() else { return false };
    (first.is_ascii_alphabetic() || first == '_') && c.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' || ch == '-')
}

/// `jsonPointerEvaluate(fragment, tree, uri)` (a `null` tree with a blank fragment gives `null`)
pub fn json_pointer_evaluate(fragment: Option<&str>, tree: Option<&JsonNode>, uri: &str) -> JResult<Option<JsonNode>> {
    if is_blank(fragment) {
        return Ok(tree.cloned());
    }
    let fragment = fragment.unwrap();
    let mut node = tree.cloned();
    for token in crate::parser::ref_utils::java_split(fragment, "/") {
        if token.trim().is_empty() {
            continue;
        }
        let Some(n) = &node else {
            return Err(JavaException::npe("Cannot invoke \"com.fasterxml.jackson.databind.JsonNode.isArray()\" because \"node\" is null"));
        };
        let next = if let JsonNode::Array(a) = n {
            let idx = java_integer_value_of(&token)?;
            if idx < 0 { None } else { a.get(idx as usize).cloned() }
        } else {
            n.get(&unescape_pointer(&token)?).cloned()
        };
        match next {
            Some(x) => node = Some(x),
            None => return Err(JavaException::new("java.lang.RuntimeException", format!("Could not find {fragment} in contents of {uri}"))),
        }
    }
    Ok(node)
}

/// `Integer.valueOf(s)`
fn java_integer_value_of(s: &str) -> JResult<i32> {
    let nfe = || JavaException::new("java.lang.NumberFormatException", format!("For input string: \"{s}\""));
    let digits = s.strip_prefix(['-', '+']).unwrap_or(s);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(nfe());
    }
    s.parse::<i32>().map_err(|_| nfe())
}

/// `getRefName(uri)`
pub fn get_ref_name(uri: &str) -> String {
    if !uri.contains('/') && uri.starts_with('#') {
        return uri[1..].to_string();
    }
    crate::parser::ref_utils::java_split(uri, "/").last().cloned().unwrap_or_default()
}
