//! `io.swagger.parser.SwaggerResolver` (swagger-parser 1.0.76): external reference resolution for Swagger
//! 2.0 documents, with its `ResolverCache`, processors, `RefUtils` and `PathUtils`.

use super::bean::{self, LoadType, Loaded};
use super::deserializer::{ParseResult, SwaggerDeserializer};
use super::models::*;
use super::parser::{deserialize_into_tree, is_json, read_json, read_yaml};
use crate::core::AuthorizationValue;
use crate::java::net::JavaUri;
use crate::java::path::{url_decode, JavaPath};
use crate::java::{JMap, JResult, JavaException, MapKind};
use indexmap::IndexMap;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

fn runtime(msg: impl Into<String>) -> JavaException {
    JavaException::new("java.lang.RuntimeException", msg)
}

/// Java `String.split(regex)` for a literal separator (trailing empty strings removed).
fn java_split(s: &str, sep: &str) -> Vec<String> {
    crate::parser::ref_utils::java_split(s, sep)
}

fn format_name(f: RefFormat) -> &'static str {
    match f {
        RefFormat::Url => "URL",
        RefFormat::Relative => "RELATIVE",
        RefFormat::Internal => "INTERNAL",
    }
}

// ---------------------------------------------------------------------------------------------------------
// RefUtils / PathUtils (1.0.76)

/// `RefUtils.computeDefinitionName(ref)`
pub fn compute_definition_name(r: &str) -> JResult<String> {
    let parts = java_split(r, "#/");
    if parts.len() > 2 {
        return Err(runtime(format!("Invalid ref format: {r}")));
    }
    let file = parts.first().cloned().unwrap_or_default();
    let definition_path = if parts.len() == 2 { Some(parts[1].clone()) } else { None };
    Ok(match definition_path {
        Some(dp) => java_split(&dp, "/").last().cloned().unwrap_or_default(),
        None => {
            let name = java_split(&file, "/").last().cloned().unwrap_or_default();
            java_split(&name, ".").first().cloned().unwrap_or_default()
        }
    })
}

/// `RefUtils.isAnExternalRefFormat(format)`: whether the reference points outside the document (a URL or a relative file).
pub fn is_an_external_ref_format(f: Option<RefFormat>) -> bool {
    matches!(f, Some(RefFormat::Url) | Some(RefFormat::Relative))
}

/// `ExternalRefProcessor.join(source, fragment)`
pub fn join(source: &str, fragment: &str) -> String {
    let attempt = || -> JResult<String> {
        let is_relative = source.starts_with('/') || source.starts_with('.');
        let mut uri = JavaUri::parse(source)?;
        let path_empty = uri.get_path() == Some("");
        if (!source.ends_with('/') && fragment.starts_with("./") && path_empty) || (path_empty && !fragment.starts_with('/')) {
            uri = JavaUri::parse(&format!("{source}/"))?;
        }
        let f = JavaUri::parse(fragment)?;
        let normalized = uri.resolve(&f).normalize().to_java_string();
        let first = normalized.chars().next().ok_or_else(|| JavaException::new("java.lang.StringIndexOutOfBoundsException", "index 0, length 0"))?;
        Ok(if first.is_alphabetic() && is_relative { format!("./{normalized}") } else { normalized })
    };
    attempt().unwrap_or_else(|_| source.to_string())
}

/// `RefUtils.readExternalUrlRef(file, refFormat, auths, rootPath)`
pub fn read_external_url_ref(file: &str, format: RefFormat, auths: &[AuthorizationValue], root_path: &str) -> JResult<String> {
    if !is_an_external_ref_format(Some(format)) {
        return Err(runtime("Ref is not external"));
    }
    let attempt = || -> JResult<String> {
        if format == RefFormat::Url {
            crate::parser::remote_url::url_to_string(file, auths, None)
        } else {
            let url = join(root_path, file);
            read_external_ref(&url, RefFormat::Url, auths, None)
        }
    };
    attempt().map_err(|e| runtime(format!("Unable to load {} ref: {file} path:{root_path}", format_name(format))).with_cause(e))
}

/// `RefUtils.readExternalRef(file, refFormat, auths, parentDirectory)`
pub fn read_external_ref(file: &str, format: RefFormat, auths: &[AuthorizationValue], parent_directory: Option<&JavaPath>) -> JResult<String> {
    if !is_an_external_ref_format(Some(format)) {
        return Err(runtime("Ref is not external"));
    }
    let attempt = || -> JResult<String> {
        if format == RefFormat::Url {
            return crate::parser::remote_url::url_to_string(file, auths, None);
        }
        let Some(pd) = parent_directory else {
            return Err(JavaException::npe("Cannot invoke \"java.nio.file.Path.resolve(String)\" because \"parentDirectory\" is null"));
        };
        let mut result: Option<String> = None;
        let path_to_use = pd.resolve(file).normalize();
        if path_to_use.exists() {
            result = Some(read_all(&path_to_use)?);
        } else {
            let start = file.find('.').map(|i| i as i64).unwrap_or(-1) + if file.contains("..") { 2 } else { 1 };
            let start = start as usize;
            if start > file.len() {
                return Err(JavaException::new("java.lang.StringIndexOutOfBoundsException", format!("begin {start}, end {}, length {}", file.len(), file.len())));
            }
            let url = format!("{pd}{}", &file[start..]);
            let path_to_use2 = pd.resolve(&url).normalize();
            if path_to_use2.exists() {
                result = Some(read_all(&path_to_use2)?);
            }
        }
        match result {
            Some(r) => Ok(r),
            None => crate::parser::ref_utils::load_file_from_classpath(file),
        }
    };
    attempt().map_err(|e| {
        let pd = parent_directory.map(|p| p.to_string()).unwrap_or_else(|| "null".into());
        runtime(format!("Unable to load {} ref: {file} path: {pd}", format_name(format))).with_cause(e)
    })
}

fn read_all(path: &JavaPath) -> JResult<String> {
    if path.is_directory() {
        return Err(JavaException::new("java.io.FileNotFoundException", format!("{path} (Is a directory)")));
    }
    path.read_to_string().map_err(|e| JavaException::new("java.io.IOException", e.to_string()))
}

/// `PathUtils.getParentDirectoryOfFile(location)` (1.0.76: a missing file resolves against its absolute
/// path when it is not on the class path either)
pub fn get_parent_directory_of_file(location: &str) -> Option<JavaPath> {
    let location = location.replace('\\', "/");
    let file = if location.to_lowercase().starts_with("file:") {
        let uri = crate::parser::path_utils::uri_create(&location).ok()?;
        crate::parser::path_utils::path_of_uri(&uri).ok()?.to_absolute_path()
    } else {
        JavaPath::new(&location).to_absolute_path()
    };
    if !file.exists() && let Some(p) = crate::java::classpath::find_class_resource(&location) {
        return p.to_absolute_path().get_parent();
    }
    file.to_absolute_path().get_parent()
}

// ---------------------------------------------------------------------------------------------------------
// ResolverCache

/// `io.swagger.parser.ResolverCache`
pub struct ResolverCache {
    swagger: Shared<Swagger>,
    auths: Vec<AuthorizationValue>,
    parent_directory: Option<JavaPath>,
    root_path: Option<String>,
    resolution_cache: RefCell<HashMap<String, Loaded>>,
    external_file_cache: RefCell<HashMap<String, String>>,
    referenced_model_keys: RefCell<HashSet<String>>,
    rename_cache: JMap<String>,
}

impl ResolverCache {
    /// `new ResolverCache(swagger, auths, parentFileLocation)`: relative files resolve against the parent directory
    /// of `parent_file_location` (none for an `http` location, the working directory when `None`).
    pub fn new(swagger: Shared<Swagger>, auths: Vec<AuthorizationValue>, parent_file_location: Option<&str>) -> ResolverCache {
        let parent_directory = match parent_file_location {
            Some(l) if l.starts_with("http") => None,
            Some(l) => get_parent_directory_of_file(l),
            None => Some(JavaPath::new(".")),
        };
        ResolverCache {
            swagger,
            auths,
            parent_directory,
            root_path: parent_file_location.map(str::to_string),
            resolution_cache: RefCell::new(HashMap::new()),
            external_file_cache: RefCell::new(HashMap::new()),
            referenced_model_keys: RefCell::new(HashSet::new()),
            rename_cache: JMap::with_kind(MapKind::Hash),
        }
    }

    /// `loadRef(ref, refFormat, expectedType)`; `Ok(None)` where Java returns null (including a failed cast
    /// of an internal ref).
    pub fn load_ref(&self, r: &str, format: Option<RefFormat>, expected: LoadType) -> JResult<Option<Loaded>> {
        if format == Some(RefFormat::Internal) {
            return Ok(self.load_internal_ref(r).filter(|l| loaded_type(l) == expected));
        }
        let parts = java_split(r, "#/");
        if parts.len() > 2 {
            return Err(runtime(format!("Invalid ref format: {r}")));
        }
        let file = parts.first().cloned().unwrap_or_default();
        let definition_path = if parts.len() == 2 { Some(parts[1].clone()) } else { None };
        if let Some(prev) = self.resolution_cache.borrow().get(r).cloned() {
            if loaded_type(&prev) != expected {
                return Err(JavaException::new("java.lang.ClassCastException", format!("Cannot cast {} to {}", loaded_class(&prev), expected.class_string().split(' ').nth(1).unwrap_or(""))));
            }
            return Ok(Some(prev));
        }
        let cached = self.external_file_cache.borrow().get(&file).cloned();
        let contents = match cached {
            Some(c) => c,
            None => {
                let format = format.unwrap_or(RefFormat::Internal);
                let c = if let Some(pd) = &self.parent_directory {
                    read_external_ref(&file, format, &self.auths, Some(pd))?
                } else if let Some(root) = &self.root_path {
                    read_external_url_ref(&file, format, &self.auths, root)?
                } else {
                    return Err(JavaException::without_message("java.lang.NullPointerException"));
                };
                self.external_file_cache.borrow_mut().insert(file.clone(), c.clone());
                c
            }
        };
        let Some(definition_path) = definition_path else {
            let result = deserialize_contents(&contents, &file, expected)?;
            if let Some(res) = &result {
                self.resolution_cache.borrow_mut().insert(r.to_string(), res.clone());
            }
            return Ok(result);
        };
        let mut tree = deserialize_into_tree(&contents, &file)?;
        for el in java_split(&definition_path, "/") {
            match tree.get(&unescape_pointer(&el)).cloned() {
                Some(t) => tree = t,
                None => return Err(runtime(format!("Could not find {definition_path} in contents of {file}"))),
            }
        }
        let result = if expected == LoadType::Model {
            if !tree.is_object() {
                return Err(JavaException::class_cast(&format!("com.fasterxml.jackson.databind.node.{}", tree.node_class_name()), "com.fasterxml.jackson.databind.node.ObjectNode"));
            }
            let mut pr = ParseResult::new();
            SwaggerDeserializer::new().definition(Some(&tree), &definition_path.replace('/', "."), &mut pr)?.map(Loaded::Model)
        } else {
            bean::convert(&tree, expected).map_err(|e| deserialize_error(&file, expected, e))?
        };
        if let Some(res) = &result {
            self.update_local_refs(&file, res);
            self.resolution_cache.borrow_mut().insert(r.to_string(), res.clone());
            if let Loaded::Parameter(p) = res && p.borrow().kind == ParameterKind::Body {
                self.load_body_parameter_ref(r, format, p)?;
            }
        }
        Ok(result)
    }

    fn load_body_parameter_ref(&self, r: &str, format: Option<RefFormat>, bp: &ParameterRef) -> JResult<()> {
        let schema = bp.borrow().schema.clone();
        let Some(schema) = schema else { return Ok(()) };
        if schema.borrow().kind != ModelKind::Ref || format == Some(RefFormat::Internal) {
            return Ok(());
        }
        let root_ref = &r[..r.find('#').unwrap_or(0)];
        let (external_ref, simple) = {
            let m = schema.borrow();
            let reference = m.get_ref().unwrap_or_default();
            let ext = if is_an_external_ref_format(m.ref_format()) { reference } else { format!("{root_ref}{reference}") };
            (ext, m.simple_ref().unwrap_or_default())
        };
        let deref = self.load_ref(&external_ref, format, LoadType::Model)?;
        if let Some(Loaded::Model(m)) = deref {
            self.swagger.borrow_mut().add_definition(&simple, m);
        }
        Ok(())
    }

    fn update_local_refs(&self, file: &str, result: &Loaded) {
        match result {
            Loaded::Response(r) => {
                let schema = r.borrow().schema.clone();
                if let Some(s) = schema {
                    self.update_local_refs_model(file, &s);
                }
            }
            Loaded::Model(m) => self.update_local_refs_model(file, m),
            _ => {}
        }
    }

    fn update_local_refs_model(&self, file: &str, m: &ModelRef) {
        let kind = m.borrow().kind;
        match kind {
            ModelKind::Ref => {
                let updated = merge(file, &m.borrow().get_ref().unwrap_or_default());
                m.borrow_mut().set_ref(&updated);
            }
            ModelKind::Impl => {
                let props = m.borrow().properties.clone();
                if let Some(props) = props {
                    for p in props.values() {
                        if p.borrow().kind == PropertyKind::Ref {
                            let updated = merge(file, &p.borrow().get_ref().unwrap_or_default());
                            p.borrow_mut().set_ref(&updated);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn load_internal_ref(&self, r: &str) -> Option<Loaded> {
        let sw = self.swagger.borrow();
        let mut result: Option<Loaded> = None;
        if r.starts_with("#/definitions") {
            result = from_map(r, "#/parameters/", sw.parameters.as_ref()).map(Loaded::Parameter);
        } else if r.starts_with("#/responses") {
            result = from_map(r, "#/responses/", sw.responses.as_ref().map(|m| m.iter().filter_map(|(k, v)| v.clone().map(|v| (k.clone(), v))).collect::<IndexMap<_, _>>()).as_ref()).map(Loaded::Response);
        } else if r.starts_with("#/parameters") {
            result = from_map(r, "#/parameters/", sw.parameters.as_ref()).map(Loaded::Parameter);
        } else if r.starts_with("#/paths") {
            result = from_map(r, "#/paths/", sw.paths.as_ref()).map(Loaded::Path);
        }
        if result.is_none() {
            result = from_map(r, "#/definitions/", sw.definitions.as_ref()).map(Loaded::Model);
        }
        result
    }

    /// `hasReferencedKey(key)`: whether a definition key was already taken by an imported external model.
    pub fn has_referenced_key(&self, key: &str) -> bool {
        self.referenced_model_keys.borrow().contains(key)
    }

    /// `addReferencedKey(key)`: records a definition key as taken by an imported external model.
    pub fn add_referenced_key(&self, key: &str) {
        self.referenced_model_keys.borrow_mut().insert(key.to_string());
    }

    /// `getRenamedRef(originalRef)`: the definition name an external reference was imported under, `None` if not yet imported.
    pub fn get_renamed_ref(&self, original: &str) -> Option<String> {
        self.rename_cache.get(original)
    }

    /// `putRenamedRef(originalRef, newRef)`: records the definition name an external reference was imported under.
    pub fn put_renamed_ref(&self, original: &str, new_ref: &str) {
        self.rename_cache.put(original, new_ref.to_string());
    }

    /// `getRenameCache()` entries: each imported external reference with the definition name it got.
    pub fn rename_cache_entries(&self) -> Vec<(String, String)> {
        self.rename_cache.entries()
    }

    /// `getResolutionCache().get(key)`: the object already loaded for a reference, `None` if not loaded yet.
    pub fn resolution_cache_get(&self, key: &str) -> Option<Loaded> {
        self.resolution_cache.borrow().get(key).cloned()
    }

    /// `getSwagger()`: the document being resolved.
    pub fn swagger(&self) -> Shared<Swagger> {
        self.swagger.clone()
    }
}

fn loaded_type(l: &Loaded) -> LoadType {
    match l {
        Loaded::Model(_) => LoadType::Model,
        Loaded::Parameter(_) => LoadType::Parameter,
        Loaded::Response(_) => LoadType::Response,
        Loaded::Path(_) => LoadType::Path,
        Loaded::Property(_) => LoadType::Model,
    }
}

fn loaded_class(l: &Loaded) -> &'static str {
    match l {
        Loaded::Model(_) => "io.swagger.models.Model",
        Loaded::Parameter(_) => "io.swagger.models.parameters.Parameter",
        Loaded::Response(_) => "io.swagger.models.Response",
        Loaded::Path(_) => "io.swagger.models.Path",
        Loaded::Property(_) => "io.swagger.models.properties.Property",
    }
}

fn deserialize_error(file: &str, t: LoadType, e: JavaException) -> JavaException {
    runtime(format!("An exception was thrown while trying to deserialize the contents of {file} into type {}", t.class_string())).with_cause(e)
}

/// `DeserializationUtils.deserialize(contents, file, expectedType)` for text contents
fn deserialize_contents(contents: &str, file: &str, t: LoadType) -> JResult<Option<Loaded>> {
    let attempt = || -> JResult<Option<Loaded>> {
        let tree = if is_json(contents) { read_json(contents)? } else { read_yaml(contents, None)? };
        bean::convert(&tree, t)
    };
    attempt().map_err(|e| deserialize_error(file, t, e))
}

/// `ResolverCache.merge(host, ref)`
fn merge(host: &str, r: &str) -> String {
    if host.trim().is_empty() {
        return r.to_string();
    }
    if r.starts_with("http:") || r.starts_with("https:") {
        return r.to_string();
    }
    if !host.starts_with("http:") && !host.starts_with("https:") {
        return r.to_string();
    }
    if r.starts_with('.') {
        return r.to_string();
    }
    if host.ends_with('/') && r.starts_with('/') { format!("{host}{}", &r[1..]) } else { format!("{host}{r}") }
}

/// `unescapePointer`: URL-decode, then `~1` → `/`, `~0` → `~`
fn unescape_pointer(s: &str) -> String {
    url_decode(s).replace("~1", "/").replace("~0", "~")
}

fn from_map<V: Clone>(r: &str, prefix: &str, map: Option<&IndexMap<String, V>>) -> Option<V> {
    let name = r.strip_prefix(prefix)?;
    if name.is_empty() {
        return None;
    }
    map?.get(&unescape_pointer(name)).cloned()
}

// ---------------------------------------------------------------------------------------------------------
// processors

struct ExternalRefProcessor {
    cache: Rc<ResolverCache>,
}

impl ExternalRefProcessor {
    fn swagger(&self) -> Shared<Swagger> {
        self.cache.swagger()
    }

    /// `processRefToExternalDefinition($ref, refFormat)`
    fn process_ref_to_external_definition(&self, r: &str, format: Option<RefFormat>) -> JResult<String> {
        if let Some(renamed) = self.cache.get_renamed_ref(r) {
            return Ok(renamed);
        }
        let model = match self.cache.load_ref(r, format, LoadType::Model)? {
            Some(Loaded::Model(m)) => m,
            _ => return Ok(r.to_string()),
        };
        let definitions: IndexMap<String, ModelRef> = self.swagger().borrow().definitions.clone().unwrap_or_default();
        let possibly_conflicting = compute_definition_name(r)?;
        let mut try_name: Option<String> = None;
        let mut existing = definitions.get(&possibly_conflicting).cloned();
        if let Some(e) = &existing {
            if e.borrow().kind == ModelKind::Ref {
                existing = None;
            } else {
                let mut i = 2;
                for name in definitions.keys() {
                    if *name == possibly_conflicting {
                        let t = format!("{possibly_conflicting}_{i}");
                        existing = definitions.get(&t).cloned();
                        try_name = Some(t);
                        i += 1;
                    }
                }
            }
        }
        let new_ref = match &try_name {
            Some(t) if !t.trim().is_empty() => t.clone(),
            _ => possibly_conflicting,
        };
        self.cache.put_renamed_ref(r, &new_ref);
        if existing.is_none() {
            self.swagger().borrow_mut().add_definition(&new_ref, model.clone());
            self.cache.add_referenced_key(&new_ref);
            let file = java_split(r, "#/").first().cloned().unwrap_or_default();
            let kind = model.borrow().kind;
            if kind == ModelKind::Ref {
                let (mref, mfmt) = { let m = model.borrow(); (m.get_ref().unwrap_or_default(), m.ref_format()) };
                let new = if is_an_external_ref_format(mfmt) {
                    self.process_ref_to_external_definition(&mref, mfmt)?
                } else {
                    self.process_ref_to_external_definition(&format!("{file}{mref}"), Some(RefFormat::Relative))?
                };
                model.borrow_mut().set_ref(&new);
            }
            if kind == ModelKind::Composed {
                let all_of = model.borrow().all_of.clone();
                for item in all_of {
                    let ik = item.borrow().kind;
                    if ik == ModelKind::Ref {
                        let (iref, ifmt) = { let m = item.borrow(); (m.get_ref().unwrap_or_default(), m.ref_format()) };
                        if is_an_external_ref_format(ifmt) {
                            let joined = join(&file, &iref);
                            let new = self.process_ref_to_external_definition(&joined, ifmt)?;
                            item.borrow_mut().set_ref(&new);
                        } else {
                            self.process_ref_to_external_definition(&format!("{file}{iref}"), Some(RefFormat::Relative))?;
                        }
                    } else if ik == ModelKind::Impl {
                        let props = item.borrow().properties.clone();
                        self.process_properties_map(props.as_ref(), &file)?;
                    }
                }
            }
            let props = model.borrow().properties.clone();
            self.process_properties_map(props.as_ref(), &file)?;
            if kind == ModelKind::Impl {
                let (disc, props, ap) = { let m = model.borrow(); (m.discriminator.clone(), m.properties.clone(), m.additional_properties.clone()) };
                if let Some(d) = disc {
                    self.process_discriminator(&d, props.as_ref(), &file)?;
                }
                self.process_properties(&[ap], &file)?;
            }
            if kind == ModelKind::Array {
                let items = model.borrow().items.clone();
                if let Some(items) = &items && items.borrow().kind == PropertyKind::Ref {
                    self.process_ref_property(items, &file)?;
                }
                if let Some(items) = &items {
                    let ik = items.borrow().kind;
                    if ik == PropertyKind::Object {
                        let props = items.borrow().properties.clone();
                        self.process_properties_map(props.as_ref(), &file)?;
                    }
                }
            }
        }
        Ok(new_ref)
    }

    /// `processRefToExternalResponse($ref, refFormat)`
    fn process_ref_to_external_response(&self, r: &str, format: Option<RefFormat>) -> JResult<String> {
        if let Some(renamed) = self.cache.get_renamed_ref(r) {
            return Ok(renamed);
        }
        let response = match self.cache.load_ref(r, format, LoadType::Response)? {
            Some(Loaded::Response(x)) => Some(x),
            _ => None,
        };
        let possibly_conflicting = compute_definition_name(r)?;
        self.cache.put_renamed_ref(r, &possibly_conflicting);
        if let Some(response) = response {
            let schema = response.borrow().schema.clone();
            if let Some(schema) = schema {
                self.process_ref_schema_object(&schema, r)?;
            }
        }
        Ok(possibly_conflicting)
    }

    fn process_ref_schema_object(&self, schema: &ModelRef, r: &str) -> JResult<()> {
        let file = java_split(r, "#/").first().cloned().unwrap_or_default();
        if schema.borrow().kind == ModelKind::Ref {
            let (sref, fmt) = { let m = schema.borrow(); (m.get_ref().unwrap_or_default(), m.ref_format()) };
            if is_an_external_ref_format(fmt) {
                self.process_ref_model(schema, r)?;
            } else {
                self.process_ref_to_external_definition(&format!("{file}{sref}"), Some(RefFormat::Relative))?;
            }
        } else {
            self.process_schema(schema, &file)?;
        }
        Ok(())
    }

    fn process_schema(&self, m: &ModelRef, file: &str) -> JResult<()> {
        let kind = m.borrow().kind;
        if kind == ModelKind::Ref {
            self.process_ref_model(m, file)?;
        }
        let props = m.borrow().properties.clone();
        if props.is_some() {
            self.process_properties_map(props.as_ref(), file)?;
        }
        if kind == ModelKind::Composed {
            let all_of = m.borrow().all_of.clone();
            for x in all_of {
                self.process_schema(&x, file)?;
            }
        }
        Ok(())
    }

    fn process_discriminator(&self, discriminator: &str, properties: Option<&JMap<PropertyRef>>, file: &str) -> JResult<()> {
        let Some(properties) = properties else { return Ok(()) };
        for (k, v) in properties.entries() {
            if k != discriminator {
                continue;
            }
            let kind = v.borrow().kind;
            if kind.is_string_property() {
                let e = v.borrow().enum_strings.clone();
                if let Some(e) = e {
                    for name in e {
                        let p = Property::new_ref(&format!("#/definitions/{name}"), None).into_ref();
                        self.process_ref_property(&p, file)?;
                    }
                }
            } else if kind == PropertyKind::Ref {
                let r = v.borrow().simple_ref().unwrap_or_default();
                for (key, value) in self.cache.rename_cache_entries() {
                    if value == r && let Some(Loaded::Model(resolved)) = self.cache.resolution_cache_get(&key) {
                        let e = { let m = resolved.borrow(); if m.kind == ModelKind::Impl { m.enum_.clone() } else { None } };
                        if let Some(e) = e {
                            for name in e {
                                let p = Property::new_ref(&format!("#/definitions/{name}"), None).into_ref();
                                self.process_ref_property(&p, file)?;
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn process_properties_map(&self, props: Option<&JMap<PropertyRef>>, file: &str) -> JResult<()> {
        if let Some(p) = props && !p.is_empty() {
            let values: Vec<Option<PropertyRef>> = p.values().into_iter().map(Some).collect();
            self.process_properties(&values, file)?;
        }
        Ok(())
    }

    fn process_properties(&self, props: &[Option<PropertyRef>], file: &str) -> JResult<()> {
        for prop in props.iter().flatten() {
            let kind = prop.borrow().kind;
            match kind {
                PropertyKind::Ref => self.process_ref_property(prop, file)?,
                PropertyKind::Array => {
                    let items = prop.borrow().items.clone();
                    self.process_properties(&[items], file)?;
                }
                PropertyKind::Map => {
                    let ap = prop.borrow().additional_properties.clone();
                    self.process_properties(&[ap], file)?;
                }
                PropertyKind::Object => {
                    let p = prop.borrow().properties.clone();
                    self.process_properties_map(p.as_ref(), file)?;
                }
                PropertyKind::Composed => {
                    let all_of = prop.borrow().all_of.clone().unwrap_or_default();
                    if !all_of.is_empty() {
                        self.process_properties(&all_of, file)?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn process_ref_property(&self, sub: &PropertyRef, external_file: &str) -> JResult<()> {
        let (sref, fmt) = { let p = sub.borrow(); (p.get_ref().unwrap_or_default(), p.ref_format()) };
        if is_an_external_ref_format(fmt) {
            let joined = join(external_file, &sref);
            let processed = self.process_ref_to_external_definition(&joined, fmt)?;
            if !processed.starts_with("http") && !processed.starts_with("https:") {
                sub.borrow_mut().set_ref(&format!("#/definitions/{processed}"));
            } else {
                sub.borrow_mut().set_ref(&processed);
            }
        } else {
            let processed = self.process_ref_to_external_definition(&format!("{external_file}{sref}"), Some(RefFormat::Relative))?;
            sub.borrow_mut().set_ref(&format!("#/definitions/{processed}"));
        }
        Ok(())
    }

    fn process_ref_model(&self, sub: &ModelRef, external_file: &str) -> JResult<()> {
        let (sref, fmt) = { let m = sub.borrow(); (m.get_ref().unwrap_or_default(), m.ref_format()) };
        if !is_an_external_ref_format(fmt) {
            let processed = self.process_ref_to_external_definition(&format!("{external_file}{sref}"), Some(RefFormat::Relative))?;
            sub.borrow_mut().set_ref(&format!("#/definitions/{processed}"));
        } else {
            let sub_path = get_external_path(&sref);
            if fmt == Some(RefFormat::Relative) && sub_path.as_deref() != Some(external_file) {
                sub.borrow_mut().set_ref(&sref);
            } else {
                self.process_ref_to_external_definition(&sref, fmt)?;
            }
        }
        Ok(())
    }
}

/// `ExternalRefProcessor.getExternalPath(ref)`: the last `#/`-separated element of length 2
fn get_external_path(r: &str) -> Option<String> {
    let mut element = None;
    for e in java_split(r, "#/") {
        if e.chars().count() == 2 {
            element = Some(e);
        }
    }
    element
}

struct PropertyProcessor {
    ext: Rc<ExternalRefProcessor>,
}

impl PropertyProcessor {
    fn process_property(&self, p: &PropertyRef) -> JResult<()> {
        let kind = p.borrow().kind;
        match kind {
            PropertyKind::Ref => {
                let (r, f) = { let x = p.borrow(); (x.get_ref().unwrap_or_default(), x.ref_format()) };
                if is_an_external_ref_format(f) {
                    let new = self.ext.process_ref_to_external_definition(&r, f)?;
                    p.borrow_mut().set_ref(&format!("#/definitions/{new}"));
                }
            }
            PropertyKind::Array => {
                let items = p.borrow().items.clone();
                if let Some(i) = items {
                    self.process_property(&i)?;
                }
            }
            PropertyKind::Map => {
                let ap = p.borrow().additional_properties.clone();
                if let Some(a) = ap {
                    self.process_property(&a)?;
                }
            }
            PropertyKind::Object => {
                let props = p.borrow().properties.clone();
                if let Some(props) = props {
                    for x in props.values() {
                        self.process_property(&x)?;
                    }
                }
            }
            PropertyKind::Composed => {
                let all_of = p.borrow().all_of.clone();
                if let Some(all_of) = all_of {
                    for x in all_of.iter().flatten() {
                        self.process_property(x)?;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}

struct ModelProcessor {
    props: PropertyProcessor,
    ext: Rc<ExternalRefProcessor>,
}

impl ModelProcessor {
    fn process_model(&self, m: Option<&ModelRef>) -> JResult<()> {
        let Some(m) = m else { return Ok(()) };
        let kind = m.borrow().kind;
        match kind {
            ModelKind::Ref => self.process_ref_model(m)?,
            ModelKind::Array => {
                let items = m.borrow().items.clone();
                if let Some(i) = items {
                    self.props.process_property(&i)?;
                }
            }
            ModelKind::Composed => {
                let (parent, child, interfaces) = { let x = m.borrow(); (x.parent.clone(), x.child.clone(), x.interfaces.clone()) };
                self.process_model(parent.as_ref())?;
                self.process_model(child.as_ref())?;
                if let Some(interfaces) = interfaces {
                    for i in interfaces {
                        self.process_ref_model(&i)?;
                    }
                }
                self.process_model_properties(m)?;
            }
            ModelKind::Impl => self.process_model_properties(m)?,
            ModelKind::BooleanValue => {}
        }
        Ok(())
    }

    fn process_model_properties(&self, m: &ModelRef) -> JResult<()> {
        let props = m.borrow().properties.clone();
        if let Some(props) = props {
            for p in props.values() {
                self.props.process_property(&p)?;
            }
        }
        Ok(())
    }

    fn process_ref_model(&self, m: &ModelRef) -> JResult<()> {
        let (r, f) = { let x = m.borrow(); (x.get_ref().unwrap_or_default(), x.ref_format()) };
        if is_an_external_ref_format(f) {
            let new = self.ext.process_ref_to_external_definition(&r, f)?;
            m.borrow_mut().set_ref(&format!("#/definitions/{new}"));
        }
        Ok(())
    }
}

struct ParameterProcessor {
    cache: Rc<ResolverCache>,
    models: ModelProcessor,
}

impl ParameterProcessor {
    fn process_parameters(&self, parameters: Option<&Vec<ParameterRef>>) -> JResult<Option<Vec<ParameterRef>>> {
        let Some(parameters) = parameters else { return Ok(None) };
        let mut processed: Vec<ParameterRef> = Vec::new();
        let mut ref_parameters: Vec<ParameterRef> = Vec::new();
        for parameter in parameters {
            let kind = parameter.borrow().kind;
            if kind == ParameterKind::Ref {
                let (r, f) = { let p = parameter.borrow(); (p.get_ref().unwrap_or_default(), p.ref_format()) };
                let resolved = match self.cache.load_ref(&r, f, LoadType::Parameter)? {
                    Some(Loaded::Parameter(p)) => Some(p),
                    _ => None,
                };
                match resolved {
                    None => processed.push(parameter.clone()),
                    Some(resolved) => {
                        let name = resolved.borrow().name.clone();
                        let mut matched = processed.iter().any(|p| p.borrow().name.is_some() && p.borrow().name == name);
                        if !matched {
                            matched = parameters.iter().any(|p| p.borrow().name.is_some() && p.borrow().name == name);
                        }
                        if resolved.borrow().kind == ParameterKind::Body {
                            let schema = resolved.borrow().schema.clone();
                            self.models.process_model(schema.as_ref())?;
                        }
                        if matched {
                            ref_parameters.push(resolved);
                        } else {
                            processed.push(resolved);
                        }
                    }
                }
            } else {
                if kind == ParameterKind::Body {
                    let schema = parameter.borrow().schema.clone();
                    self.models.process_model(schema.as_ref())?;
                }
                processed.push(parameter.clone());
            }
        }
        for resolved in ref_parameters {
            let name = resolved.borrow().name.clone();
            for p in processed.iter_mut() {
                let pn = p.borrow().name.clone();
                let Some(pn) = pn else {
                    return Err(JavaException::without_message("java.lang.NullPointerException"));
                };
                if Some(&pn) == name.as_ref() {
                    *p = resolved.clone();
                    break;
                }
            }
        }
        Ok(Some(processed))
    }
}

struct ResponseProcessor {
    models: ModelProcessor,
    ext: Rc<ExternalRefProcessor>,
}

impl ResponseProcessor {
    fn process_response(&self, response: &ResponseRef) -> JResult<()> {
        let (schema, is_ref) = { let r = response.borrow(); (r.schema.clone(), r.is_ref) };
        if is_ref {
            let (f, r) = { let x = response.borrow(); (x.ref_format(), x.get_ref().unwrap_or_default()) };
            if is_an_external_ref_format(f) {
                self.ext.process_ref_to_external_response(&r, f)?;
            }
        }
        self.models.process_model(schema.as_ref())
    }
}

struct OperationProcessor {
    cache: Rc<ResolverCache>,
    parameters: ParameterProcessor,
    responses: ResponseProcessor,
}

impl OperationProcessor {
    fn process_operation(&self, op: &OperationRef) -> JResult<()> {
        let params = op.borrow().parameters.clone();
        let processed = self.parameters.process_parameters(params.as_ref())?;
        op.borrow_mut().parameters = processed;
        let responses = op.borrow().responses.clone();
        if let Some(responses) = responses {
            for (code, response) in responses {
                let Some(response) = response else { continue };
                let mut response = response;
                if response.borrow().is_ref {
                    self.responses.process_response(&response)?;
                    let (r, f) = { let x = response.borrow(); (x.get_ref().unwrap_or_default(), x.ref_format()) };
                    if let Some(Loaded::Response(resolved)) = self.cache.load_ref(&r, f, LoadType::Response)? {
                        response = resolved.clone();
                        if let Some(m) = op.borrow_mut().responses.as_mut() {
                            m.insert(code.clone(), Some(resolved));
                        }
                    }
                }
                self.responses.process_response(&response)?;
            }
        }
        Ok(())
    }
}

fn external(cache: &Rc<ResolverCache>) -> Rc<ExternalRefProcessor> {
    Rc::new(ExternalRefProcessor { cache: cache.clone() })
}

fn model_processor(cache: &Rc<ResolverCache>) -> ModelProcessor {
    ModelProcessor { props: PropertyProcessor { ext: external(cache) }, ext: external(cache) }
}

fn parameter_processor(cache: &Rc<ResolverCache>) -> ParameterProcessor {
    ParameterProcessor { cache: cache.clone(), models: model_processor(cache) }
}

fn operation_processor(cache: &Rc<ResolverCache>) -> OperationProcessor {
    OperationProcessor {
        cache: cache.clone(),
        parameters: parameter_processor(cache),
        responses: ResponseProcessor { models: model_processor(cache), ext: external(cache) },
    }
}

/// `SwaggerResolver.Settings`
#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// `addParametersToEachOperation`: copy path-level parameters into each operation of the path (default `true`).
    pub add_parameters_to_each_operation: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { add_parameters_to_each_operation: true }
    }
}

/// `io.swagger.parser.SwaggerResolver`
pub struct SwaggerResolver {
    swagger: Option<Shared<Swagger>>,
    cache: Option<Rc<ResolverCache>>,
    settings: Settings,
}

impl SwaggerResolver {
    /// `new SwaggerResolver(swagger, auths, parentFileLocation)`; a `None` document makes [`SwaggerResolver::resolve`] return `Ok(None)`.
    pub fn new(swagger: Option<Shared<Swagger>>, auths: Vec<AuthorizationValue>, parent_file_location: Option<&str>) -> SwaggerResolver {
        let cache = swagger.as_ref().map(|s| Rc::new(ResolverCache::new(s.clone(), auths, parent_file_location)));
        SwaggerResolver { swagger, cache, settings: Settings::default() }
    }

    /// `setSettings(settings)` in builder form: replaces the resolver settings.
    pub fn with_settings(mut self, settings: Settings) -> SwaggerResolver {
        self.settings = settings;
        self
    }

    /// `resolve()`
    pub fn resolve(&self) -> JResult<Option<Shared<Swagger>>> {
        let (Some(swagger), Some(cache)) = (&self.swagger, &self.cache) else { return Ok(None) };
        // top-level relative parameter refs
        let params = swagger.borrow().parameters.clone();
        if let Some(params) = params {
            let pp = parameter_processor(cache);
            for (name, param) in params {
                let is_relative_ref = { let p = param.borrow(); p.kind == ParameterKind::Ref && p.ref_format() == Some(RefFormat::Relative) };
                if is_relative_ref {
                    let processed = pp.process_parameters(Some(&vec![param.clone()]))?.unwrap_or_default();
                    if let Some(first) = processed.into_iter().next()
                        && let Some(m) = swagger.borrow_mut().parameters.as_mut()
                    {
                        m.insert(name, first);
                    }
                }
            }
        }
        self.process_paths(swagger, cache)?;
        self.process_definitions(swagger, cache)?;
        let paths = swagger.borrow().paths.clone();
        if let Some(paths) = paths {
            let op = operation_processor(cache);
            for (_, path) in paths {
                let ops = path.borrow().get_operations();
                for o in ops {
                    op.process_operation(&o)?;
                }
            }
        }
        Ok(Some(swagger.clone()))
    }

    /// `PathsProcessor.processPaths()`
    fn process_paths(&self, swagger: &Shared<Swagger>, cache: &Rc<ResolverCache>) -> JResult<()> {
        let paths = swagger.borrow().paths.clone();
        let Some(paths) = paths else { return Ok(()) };
        let pp = parameter_processor(cache);
        let op = operation_processor(cache);
        for (path_str, path) in paths {
            let mut path = path;
            if self.settings.add_parameters_to_each_operation {
                let parameters = path.borrow().parameters.clone();
                if let Some(parameters) = parameters {
                    let operations = path.borrow().get_operations();
                    for operation in operations {
                        let mut to_add = Vec::new();
                        let existing = operation.borrow().parameters.clone().unwrap_or_default();
                        for p in &parameters {
                            let mut matched = false;
                            for e in &existing {
                                let (pi, pn) = { let x = p.borrow(); (x.in_.clone(), x.name.clone()) };
                                let (ei, en) = { let x = e.borrow(); (x.in_.clone(), x.name.clone()) };
                                if pi.is_some() && pi == ei {
                                    if pn.is_none() {
                                        return Err(JavaException::without_message("java.lang.NullPointerException"));
                                    }
                                    if pn == en {
                                        matched = true;
                                    }
                                }
                            }
                            if !matched {
                                to_add.push(p.clone());
                            }
                        }
                        if !to_add.is_empty() {
                            let mut ob = operation.borrow_mut();
                            let list = ob.parameters.get_or_insert_with(Vec::new);
                            for (i, p) in to_add.into_iter().enumerate() {
                                list.insert(i, p);
                            }
                        }
                    }
                }
                path.borrow_mut().parameters = None;
            }
            if path.borrow().is_ref {
                let (r, f) = { let p = path.borrow(); (p.get_ref(), p.ref_format()) };
                let r = r.ok_or_else(|| JavaException::without_message("java.lang.NullPointerException"))?;
                let resolved = match cache.load_ref(&r, f, LoadType::Path)? {
                    Some(Loaded::Path(p)) => Some(p),
                    _ => None,
                };
                let path_ref = java_split(&r, "#").first().cloned().unwrap_or_default();
                if let Some(resolved) = resolved {
                    update_local_refs_path(&resolved, &path_ref);
                    swagger.borrow_mut().path(&path_str, resolved.clone());
                    path = resolved;
                }
            }
            let params = path.borrow().parameters.clone();
            let processed = pp.process_parameters(params.as_ref())?;
            path.borrow_mut().parameters = processed;
            let ops = path.borrow().get_operation_map();
            for o in ops {
                op.process_operation(&o)?;
            }
        }
        Ok(())
    }

    /// `DefinitionsProcessor.processDefinitions()`
    fn process_definitions(&self, swagger: &Shared<Swagger>, cache: &Rc<ResolverCache>) -> JResult<()> {
        if swagger.borrow().definitions.is_none() {
            return Ok(());
        }
        let mp = model_processor(cache);
        let mut key_set: indexmap::IndexSet<String> = indexmap::IndexSet::new();
        loop {
            let defs = swagger.borrow().definitions.clone().unwrap_or_default();
            if defs.len() <= key_set.len() {
                break;
            }
            for k in defs.keys() {
                key_set.insert(k.clone());
            }
            for model_name in key_set.clone() {
                let model = swagger.borrow().definitions.as_ref().and_then(|d| d.get(&model_name).cloned());
                let Some(model) = model else {
                    // `definitions.get(name)` is null for a removed key; Java would dereference it
                    return Err(JavaException::without_message("java.lang.NullPointerException"));
                };
                let original_ref = { let m = model.borrow(); if m.kind == ModelKind::Ref { m.get_ref() } else { None } };
                mp.process_model(Some(&model))?;
                if model.borrow().kind == ModelKind::Ref {
                    let renamed = cache.get_renamed_ref(original_ref.as_deref().unwrap_or(""));
                    if let Some(renamed) = renamed {
                        let resolved = swagger.borrow().definitions.as_ref().and_then(|d| d.get(&renamed).cloned());
                        if !cache.has_referenced_key(&renamed)
                            && let Some(d) = swagger.borrow_mut().definitions.as_mut()
                        {
                            d.shift_remove(&renamed);
                        }
                        if let Some(resolved) = resolved && let Some(d) = swagger.borrow_mut().definitions.as_mut() {
                            d.insert(model_name.clone(), resolved);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

/// `PathsProcessor.updateLocalRefs(path, pathRef)`
fn update_local_refs_path(path: &PathRef, path_ref: &str) {
    let (params, ops) = { let p = path.borrow(); (p.parameters.clone(), p.get_operations()) };
    for p in params.unwrap_or_default() {
        update_local_refs_param(&p, path_ref);
    }
    for op in ops {
        let (params, responses) = { let o = op.borrow(); (o.parameters.clone(), o.responses.clone()) };
        for p in params.unwrap_or_default() {
            update_local_refs_param(&p, path_ref);
        }
        for r in responses.unwrap_or_default().values().flatten() {
            let schema = r.borrow().schema.clone();
            if let Some(s) = schema {
                update_local_refs_model(&s, path_ref);
            }
        }
    }
}

fn update_local_refs_param(p: &ParameterRef, path_ref: &str) {
    let schema = { let x = p.borrow(); if x.kind == ParameterKind::Body { x.schema.clone() } else { None } };
    if let Some(s) = schema {
        update_local_refs_model(&s, path_ref);
    }
}

fn update_local_refs_model(m: &ModelRef, path_ref: &str) {
    let kind = m.borrow().kind;
    match kind {
        ModelKind::Ref => {
            let (r, f) = { let x = m.borrow(); (x.get_ref().unwrap_or_default(), x.ref_format()) };
            match f {
                Some(RefFormat::Internal) => m.borrow_mut().set_ref(&format!("{path_ref}{r}")),
                Some(RefFormat::Relative) => m.borrow_mut().set_ref(&compute_relative_ref(&r, path_ref)),
                _ => {}
            }
        }
        ModelKind::Impl => {
            let props = m.borrow().properties.clone();
            if let Some(props) = props {
                for p in props.values() {
                    update_local_refs_property(&p, path_ref);
                }
            }
        }
        ModelKind::Composed => {
            let all_of = m.borrow().all_of.clone();
            for x in all_of {
                update_local_refs_model(&x, path_ref);
            }
        }
        ModelKind::Array => {
            let items = m.borrow().items.clone();
            if let Some(i) = items {
                update_local_refs_property(&i, path_ref);
            }
        }
        ModelKind::BooleanValue => {}
    }
}

fn update_local_refs_property(p: &PropertyRef, path_ref: &str) {
    if p.borrow().kind != PropertyKind::Ref {
        return;
    }
    let (r, f) = { let x = p.borrow(); (x.get_ref().unwrap_or_default(), x.ref_format()) };
    match f {
        Some(RefFormat::Internal) => p.borrow_mut().set_ref(&format!("{path_ref}{r}")),
        Some(RefFormat::Relative) => p.borrow_mut().set_ref(&compute_relative_ref(&r, path_ref)),
        _ => {}
    }
}

fn compute_relative_ref(r: &str, prefix: &str) -> String {
    match prefix.rfind('/') {
        Some(i) if i > 1 => format!("{}{r}", &prefix[..i + 1]),
        _ => r.to_string(),
    }
}
