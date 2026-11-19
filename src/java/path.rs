//! `java.nio.file.Path` (the Unix provider's string semantics) and the few `Files` / `File` operations the
//! parser uses.

use std::fmt;

/// A `UnixPath`: `/`-separated, redundant slashes collapsed, no trailing slash (except the root).
///
/// A path of the default file system has no `archive`; a path of a zip file system
/// (`FileSystems.newFileSystem(URI.create("jar:" + zip.toUri()), env).getPath(...)`) names its archive
/// file, and its file operations read the archive's entries.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct JavaPath {
    path: String,
    archive: Option<std::sync::Arc<str>>,
}

fn normalize_and_check(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut prev_slash = false;
    for c in input.chars() {
        if c == '/' {
            if prev_slash {
                continue;
            }
            prev_slash = true;
        } else {
            prev_slash = false;
        }
        out.push(c);
    }
    if out.len() > 1 && out.ends_with('/') {
        out.pop();
    }
    out
}

impl JavaPath {
    /// `Paths.get(first, more...)`
    pub fn get(first: &str, more: &[&str]) -> JavaPath {
        let mut s = first.to_string();
        for m in more {
            if m.is_empty() {
                continue;
            }
            if s.is_empty() {
                s = m.to_string();
            } else {
                s.push('/');
                s.push_str(m);
            }
        }
        JavaPath { path: normalize_and_check(&s), archive: None }
    }

    /// `Paths.get(s)`: a default-file-system path, slashes collapsed.
    pub fn new(s: &str) -> JavaPath {
        JavaPath::get(s, &[])
    }

    /// `zipFileSystem.getPath(path)`: a path inside the zip archive `archive` (a file of the default file
    /// system). Relative paths resolve against the archive's root directory.
    pub fn in_archive(archive: &str, path: &str) -> JavaPath {
        JavaPath { path: normalize_and_check(path), archive: Some(archive.into()) }
    }

    /// The archive file of a zip-file-system path (`None` for the default file system)
    pub fn archive(&self) -> Option<&str> {
        self.archive.as_deref()
    }

    /// Another path of the same file system
    fn derive(&self, path: String) -> JavaPath {
        JavaPath { path, archive: self.archive.clone() }
    }

    /// `Paths.get(URI)` for a `file:` URI (the path component, percent-decoded)
    pub fn from_file_uri(uri: &str) -> Option<JavaPath> {
        let rest = uri.strip_prefix("file:")?;
        let path = rest.strip_prefix("//").map(|r| match r.find('/') {
            Some(i) => &r[i..],
            None => "/",
        });
        let path = path.unwrap_or(rest);
        let path = path.split(['?', '#']).next().unwrap_or("");
        Some(JavaPath::new(&percent_decode(path)))
    }

    /// The normalized path string (`toString()`).
    pub fn as_str(&self) -> &str {
        &self.path
    }

    /// `isAbsolute()`: the path starts with `/`.
    pub fn is_absolute(&self) -> bool {
        self.path.starts_with('/')
    }

    /// Whether this is the empty path `""`.
    pub fn is_empty(&self) -> bool {
        self.path.is_empty()
    }

    /// `resolve(other)`
    pub fn resolve(&self, other: &str) -> JavaPath {
        let o = self.derive(normalize_and_check(other));
        if o.is_absolute() {
            return o;
        }
        if o.is_empty() {
            return self.clone();
        }
        if self.is_empty() {
            return o;
        }
        if self.path == "/" {
            return self.derive(format!("/{}", o.path));
        }
        self.derive(format!("{}/{}", self.path, o.path))
    }

    fn names(&self) -> Vec<&str> {
        self.path.split('/').filter(|s| !s.is_empty()).collect()
    }

    /// `normalize()`
    pub fn normalize(&self) -> JavaPath {
        let abs = self.is_absolute();
        let mut out: Vec<&str> = Vec::new();
        for n in self.names() {
            match n {
                "." => {}
                ".." => match out.last() {
                    Some(&p) if p != ".." => {
                        out.pop();
                    }
                    _ => {
                        if !abs {
                            out.push("..");
                        }
                    }
                },
                n => out.push(n),
            }
        }
        let joined = out.join("/");
        if abs {
            self.derive(format!("/{joined}"))
        } else {
            self.derive(joined)
        }
    }

    /// `getParent()`
    pub fn get_parent(&self) -> Option<JavaPath> {
        let names = self.names();
        if names.is_empty() {
            return None;
        }
        if names.len() == 1 {
            return if self.is_absolute() { Some(self.derive("/".into())) } else { None };
        }
        let joined = names[..names.len() - 1].join("/");
        Some(self.derive(if self.is_absolute() { format!("/{joined}") } else { joined }))
    }

    /// `getFileName()`
    pub fn get_file_name(&self) -> Option<String> {
        self.names().last().map(|s| s.to_string())
    }

    /// `toAbsolutePath()`
    pub fn to_absolute_path(&self) -> JavaPath {
        if self.is_absolute() {
            return self.clone();
        }
        if self.archive.is_some() {
            return self.derive(format!("/{}", self.path));
        }
        let cwd = std::env::current_dir().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|_| "/".into());
        JavaPath::new(&cwd).resolve(&self.path)
    }

    /// `Files.exists(path)`
    pub fn exists(&self) -> bool {
        match self.archive_entry() {
            Some(e) => e.is_some(),
            None => std::fs::metadata(self.fs_path()).is_ok(),
        }
    }

    /// `Files.isRegularFile(path)`
    pub fn is_regular_file(&self) -> bool {
        match self.archive_entry() {
            Some(e) => e == Some(false),
            None => std::fs::metadata(self.fs_path()).map(|m| m.is_file()).unwrap_or(false),
        }
    }

    /// `Files.isDirectory(path)`: `false` when the path does not exist or cannot be read.
    pub fn is_directory(&self) -> bool {
        match self.archive_entry() {
            Some(e) => e == Some(true),
            None => std::fs::metadata(self.fs_path()).map(|m| m.is_dir()).unwrap_or(false),
        }
    }

    fn fs_path(&self) -> &str {
        if self.path.is_empty() { "." } else { &self.path }
    }

    /// The entry name of an archive path (relative to the archive root, no leading `/`)
    fn entry_name(&self) -> String {
        self.to_absolute_path().normalize().path.trim_start_matches('/').to_string()
    }

    /// For an archive path: `Some(Some(is_directory))` when the entry exists (directories may be implicit),
    /// `Some(None)` when it does not; `None` for the default file system.
    fn archive_entry(&self) -> Option<Option<bool>> {
        let archive = self.archive.as_deref()?;
        let name = self.entry_name();
        let Ok(f) = std::fs::File::open(archive) else { return Some(None) };
        let Ok(z) = zip::ZipArchive::new(f) else { return Some(None) };
        if name.is_empty() {
            return Some(Some(true));
        }
        let dir = format!("{name}/");
        let mut found = None;
        // a name the archive cannot decode matches no entry name asked for
        for n in z.file_names().filter_map(Result::ok) {
            if n == name {
                found = Some(false);
            } else if n.starts_with(&dir) {
                return Some(Some(true));
            }
        }
        Some(found)
    }

    /// Reads the file as UTF-8 (malformed input replaced, like `IOUtils.toString(in, UTF_8)`).
    pub fn read_to_string(&self) -> std::io::Result<String> {
        self.read_bytes().map(|b| String::from_utf8_lossy(&b).into_owned())
    }

    /// `Files.readAllBytes(path)`
    pub fn read_bytes(&self) -> std::io::Result<Vec<u8>> {
        if let Some(archive) = self.archive.as_deref() {
            use std::io::Read;
            let mut z = zip::ZipArchive::new(std::fs::File::open(archive)?).map_err(std::io::Error::other)?;
            let mut f = z.by_name(&self.entry_name()).map_err(|_| std::io::Error::new(std::io::ErrorKind::NotFound, self.path.clone()))?;
            let mut b = Vec::new();
            f.read_to_end(&mut b)?;
            return Ok(b);
        }
        std::fs::read(self.fs_path())
    }

    /// `toUri()`: `file:///abs/path` (percent-encoded), with a trailing `/` for an existing directory.
    pub fn to_uri(&self) -> String {
        let abs = self.to_absolute_path();
        let mut s = String::from("file://");
        s.push_str(&percent_encode_path(&abs.path));
        if abs.is_directory() && !s.ends_with('/') {
            s.push('/');
        }
        s
    }
}

impl fmt::Display for JavaPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.path)
    }
}

/// `URI` path encoding of `UnixUriUtils.toUri`: unreserved and a few safe characters stay, the rest `%XX`.
fn percent_encode_path(p: &str) -> String {
    let mut out = String::new();
    for b in p.bytes() {
        let c = b as char;
        if c.is_ascii_alphanumeric() || "-_.!~*'()/,;=$&+:@".contains(c) {
            out.push(c);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

/// `%XX` decoding (UTF-8)
pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len()
            && let (Some(h), Some(l)) = ((b[i + 1] as char).to_digit(16), (b[i + 2] as char).to_digit(16))
        {
            out.push((h * 16 + l) as u8);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `URLDecoder.decode(s, "UTF-8")`: `+` → space, `%XX` decoded (malformed escapes decoded leniently).
pub fn url_decode(s: &str) -> String {
    percent_decode(&s.replace('+', " "))
}

/// `URLDecoder.decode(s, "UTF-8")` with its `IllegalArgumentException`s.
pub fn url_decode_strict(s: &str) -> crate::java::JResult<String> {
    let iae = |m: String| crate::java::JavaException::new("java.lang.IllegalArgumentException", m);
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '+' => {
                out.push(' ');
                i += 1;
            }
            '%' => {
                let mut bytes = Vec::new();
                while i + 2 < chars.len() && chars[i] == '%' {
                    let (h, l) = (chars[i + 1], chars[i + 2]);
                    match (h.to_digit(16), l.to_digit(16)) {
                        (Some(x), Some(y)) => bytes.push((x * 16 + y) as u8),
                        _ => {
                            let bad = if h.is_ascii_hexdigit() { l } else { h };
                            return Err(iae(format!("URLDecoder: Illegal hex characters in escape (%) pattern - not a hexadecimal digit: \"{bad}\" = {}", bad as u32)));
                        }
                    }
                    i += 3;
                }
                if i < chars.len() && chars[i] == '%' {
                    return Err(iae("URLDecoder: Incomplete trailing escape (%) pattern".into()));
                }
                out.push_str(&String::from_utf8_lossy(&bytes));
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_path_semantics() {
        assert_eq!(JavaPath::new("a//b/").as_str(), "a/b");
        assert_eq!(JavaPath::new("/a/b/../c/./d").normalize().as_str(), "/a/c/d");
        assert_eq!(JavaPath::new("../a/../../b").normalize().as_str(), "../../b");
        assert_eq!(JavaPath::new("/../a").normalize().as_str(), "/a");
        assert_eq!(JavaPath::new("a").get_parent(), None);
        assert_eq!(JavaPath::new("/a").get_parent().unwrap().as_str(), "/");
        assert_eq!(JavaPath::get("src/test", &["./a.yaml"]).normalize().as_str(), "src/test/a.yaml");
    }
}
