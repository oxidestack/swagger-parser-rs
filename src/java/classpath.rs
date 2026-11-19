//! The class path: `Class.getResourceAsStream` / `ClassLoader.getResource` over a configurable list of roots (a JVM
//! application's class path entries): a directory holds resources as files, a jar (any other regular file, read as a
//! zip archive) as entries. Roots are process-wide, like the JVM class path; one that does not exist is skipped.

use super::path::JavaPath;
use parking_lot::{Mutex, RwLock};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};
use std::time::SystemTime;

fn roots() -> &'static RwLock<Vec<String>> {
    static R: OnceLock<RwLock<Vec<String>>> = OnceLock::new();
    R.get_or_init(|| RwLock::new(Vec::new()))
}

/// Replaces the class path roots (directories and jars searched for resources, in order).
pub fn set_classpath_roots(dirs: Vec<String>) {
    *roots().write() = dirs;
}

/// Adds a class path root if not present.
pub fn add_classpath_root(dir: &str) {
    let mut r = roots().write();
    if !r.iter().any(|d| d == dir) {
        r.push(dir.to_string());
    }
}

/// The current class path roots, in search order.
pub fn classpath_roots() -> Vec<String> {
    roots().read().clone()
}

/// The file entries of a jar root, read once per modification time.
struct JarIndex {
    modified: Option<SystemTime>,
    names: HashSet<String>,
}

/// The entries of the jar at `path`, `None` when it cannot be read as a zip archive (skipped, as the JVM skips an
/// unreadable class path entry).
fn jar_entries(path: &str, modified: Option<SystemTime>) -> Option<Arc<JarIndex>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Arc<JarIndex>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(index) = cache.lock().get(path).filter(|i| i.modified == modified) {
        return Some(index.clone());
    }
    let archive = zip::ZipArchive::new(std::fs::File::open(path).ok()?).ok()?;
    // a name the archive cannot decode is no name a class path lookup could ask for, so it is left out
    let names = archive.file_names().filter_map(Result::ok).filter(|n| !n.ends_with('/')).map(String::from).collect();
    let index = Arc::new(JarIndex { modified, names });
    cache.lock().insert(path.to_string(), index.clone());
    Some(index)
}

/// A class path root as it is now: a directory, a jar's entries, or nothing.
enum Root {
    Directory(JavaPath),
    Jar(String, Arc<JarIndex>),
}

fn root(path: &str) -> Option<Root> {
    let metadata = std::fs::metadata(if path.is_empty() { "." } else { path }).ok()?;
    if metadata.is_dir() {
        return Some(Root::Directory(JavaPath::new(path)));
    }
    jar_entries(path, metadata.modified().ok()).map(|index| Root::Jar(path.to_string(), index))
}

/// The file a resource name maps to (`ClassLoader.getResource(name)` semantics: no leading slash). A jar's resource
/// is a path of the jar's zip file system.
pub fn find_resource(name: &str) -> Option<JavaPath> {
    if name.starts_with('/') {
        return None;
    }
    for path in roots().read().iter() {
        match root(path) {
            Some(Root::Directory(dir)) => {
                let p = dir.resolve(name);
                if p.is_regular_file() {
                    return Some(p);
                }
            }
            Some(Root::Jar(jar, index)) if index.names.contains(name) => return Some(JavaPath::in_archive(&jar, name)),
            _ => {}
        }
    }
    None
}

/// `ClasspathHelper.class.getResourceAsStream(file)` then the class loader lookups.
pub fn find_class_resource(file: &str) -> Option<JavaPath> {
    match file.strip_prefix('/') {
        // absolute class resource name: looked up from the root
        Some(rest) => find_resource(rest),
        // relative to io/swagger/v3/parser/util/ first (never present), then the loader lookups
        None => find_resource(file),
    }
}

/// ClassGraph's `getResourcesWithExtension(extension)`: the names (relative to their root, `/`-separated) of every
/// resource whose file name ends in `.<extension>`, root by root in class path order.
pub fn resources_with_extension(extension: &str) -> Vec<String> {
    let suffix = format!(".{extension}");
    let mut found = Vec::new();
    for path in roots().read().iter() {
        match root(path) {
            Some(Root::Directory(_)) => walk(std::path::Path::new(if path.is_empty() { "." } else { path }), "", &suffix, &mut found),
            Some(Root::Jar(_, index)) => {
                let mut names: Vec<&String> = index.names.iter().filter(|n| n.ends_with(&suffix)).collect();
                names.sort();
                found.extend(names.into_iter().cloned());
            }
            None => {}
        }
    }
    found
}

fn walk(dir: &std::path::Path, prefix: &str, suffix: &str, found: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = entries.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let name = format!("{prefix}{}", entry.file_name().to_string_lossy());
        let path = entry.path();
        if path.is_dir() {
            walk(&path, &format!("{name}/"), suffix, found);
        } else if name.ends_with(suffix) {
            found.push(name);
        }
    }
}
