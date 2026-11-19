//! `java.net.URI` (RFC 2396 parsing as the JDK implements it, `resolve`, `normalize`) and `java.net.URL`
//! (spec parsing with a context URL, `toString`), with the JDK's exception texts.

use crate::error::{JResult, JavaException};

fn uri_syntax(input: &str, reason: &str, index: Option<usize>) -> JavaException {
    let msg = match index {
        Some(i) => format!("{reason} at index {i}: {input}"),
        None => format!("{reason}: {input}"),
    };
    JavaException::new("java.net.URISyntaxException", msg)
}

fn is_alpha(c: char) -> bool {
    c.is_ascii_alphabetic()
}

fn is_mark(c: char) -> bool {
    "-_.!~*'()".contains(c)
}

fn is_unreserved(c: char) -> bool {
    c.is_ascii_alphanumeric() || is_mark(c)
}

fn is_reserved(c: char) -> bool {
    ";/?:@&=+$,[]".contains(c)
}

/// "other" characters java.net.URI accepts: non-US-ASCII that are not control or space characters.
fn is_other(c: char) -> bool {
    (c as u32) > 0x7f && !c.is_control() && !c.is_whitespace()
}

/// A parsed `java.net.URI`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JavaUri {
    /// `getScheme()`: the scheme without the trailing colon, `None` for a relative URI.
    pub scheme: Option<String>,
    /// opaque scheme-specific part (for opaque URIs)
    pub opaque: Option<String>,
    /// `getRawAuthority()`: the authority as written, still percent-encoded.
    pub authority: Option<String>,
    /// `getRawUserInfo()`: the user-info part of a server-based authority.
    pub user_info: Option<String>,
    /// `getHost()`: the host of a server-based authority, `None` when the authority is registry-based or absent.
    pub host: Option<String>,
    /// `getPort()`: the port number, `-1` when undefined.
    pub port: i32,
    /// `getRawPath()`: the path as written, still percent-encoded.
    pub path: Option<String>,
    /// `getRawQuery()`: the query without the leading `?`, still percent-encoded.
    pub query: Option<String>,
    /// `getRawFragment()`: the fragment without the leading `#`, still percent-encoded.
    pub fragment: Option<String>,
    /// original string form
    string: Option<String>,
}

struct UriParser<'a> {
    input: &'a str,
    chars: Vec<char>,
}

impl<'a> UriParser<'a> {
    fn err(&self, reason: &str, i: usize) -> JavaException {
        uri_syntax(self.input, reason, Some(i))
    }

    /// Scans `[start, end)` for characters allowed by `ok` (escapes allowed); returns the first bad index.
    fn check_chars(&self, start: usize, end: usize, ok: impl Fn(char) -> bool, what: &str) -> JResult<()> {
        let mut i = start;
        while i < end {
            let c = self.chars[i];
            if c == '%' {
                if i + 2 < end && self.chars[i + 1].is_ascii_hexdigit() && self.chars[i + 2].is_ascii_hexdigit() {
                    i += 3;
                    continue;
                }
                return Err(self.err(&format!("Malformed escape pair in {what}"), i));
            }
            if !(ok(c) || is_other(c)) {
                return Err(self.err(&format!("Illegal character in {what}"), i));
            }
            i += 1;
        }
        Ok(())
    }

    fn find(&self, start: usize, end: usize, set: &str) -> usize {
        (start..end).find(|&i| set.contains(self.chars[i])).unwrap_or(end)
    }
}

impl JavaUri {
    /// `new URI(str)`
    pub fn parse(input: &str) -> JResult<JavaUri> {
        let p = UriParser { input, chars: input.chars().collect() };
        let n = p.chars.len();
        let mut uri = JavaUri { scheme: None, opaque: None, authority: None, user_info: None, host: None, port: -1, path: None, query: None, fragment: None, string: Some(input.to_string()) };
        let s: String;
        // scheme
        let colon = p.find(0, n, "/?#:");
        let mut ssp_start = 0;
        if colon < n && p.chars[colon] == ':' {
            if colon == 0 {
                return Err(p.err("Expected scheme name", 0));
            }
            if !is_alpha(p.chars[0]) {
                return Err(p.err("Illegal character in scheme name", 0));
            }
            for i in 1..colon {
                let c = p.chars[i];
                if !(c.is_ascii_alphanumeric() || "+-.".contains(c)) {
                    return Err(p.err("Illegal character in scheme name", i));
                }
            }
            s = p.chars[..colon].iter().collect();
            uri.scheme = Some(s);
            ssp_start = colon + 1;
            if ssp_start >= n {
                return Err(p.err("Expected scheme-specific part", ssp_start));
            }
            if p.chars[ssp_start] != '/' {
                // opaque
                let hash = p.find(ssp_start, n, "#");
                p.check_chars(ssp_start, hash, |c| is_unreserved(c) || is_reserved(c), "opaque part")?;
                uri.opaque = Some(p.chars[ssp_start..hash].iter().collect());
                if hash < n {
                    p.check_chars(hash + 1, n, |c| is_unreserved(c) || is_reserved(c), "fragment")?;
                    uri.fragment = Some(p.chars[hash + 1..].iter().collect());
                }
                return Ok(uri);
            }
        }
        // hierarchical
        let mut i = ssp_start;
        let end_path_etc = n;
        if i + 1 < n && p.chars[i] == '/' && p.chars[i + 1] == '/' {
            let auth_start = i + 2;
            let auth_end = p.find(auth_start, n, "/?#");
            if auth_end == auth_start {
                // empty authority: allowed only if followed by a path
                if auth_end >= n || p.chars[auth_end] != '/' {
                    return Err(p.err("Expected authority", auth_start));
                }
            } else {
                p.check_chars(auth_start, auth_end, |c| is_unreserved(c) || ";:@&=+$,[]".contains(c), "authority")?;
                let auth: String = p.chars[auth_start..auth_end].iter().collect();
                uri.parse_server_authority(&auth);
                uri.authority = Some(auth);
            }
            i = auth_end;
        }
        let path_end = p.find(i, end_path_etc, "?#");
        p.check_chars(i, path_end, |c| is_unreserved(c) || ";/:@&=+$,".contains(c), "path")?;
        uri.path = Some(p.chars[i..path_end].iter().collect());
        let mut j = path_end;
        if j < n && p.chars[j] == '?' {
            let qend = p.find(j + 1, n, "#");
            p.check_chars(j + 1, qend, |c| is_unreserved(c) || is_reserved(c), "query")?;
            uri.query = Some(p.chars[j + 1..qend].iter().collect());
            j = qend;
        }
        if j < n && p.chars[j] == '#' {
            p.check_chars(j + 1, n, |c| is_unreserved(c) || is_reserved(c), "fragment")?;
            uri.fragment = Some(p.chars[j + 1..].iter().collect());
        }
        Ok(uri)
    }

    fn parse_server_authority(&mut self, auth: &str) {
        let (ui, hp) = match auth.rfind('@') {
            Some(at) => (Some(auth[..at].to_string()), &auth[at + 1..]),
            None => (None, auth),
        };
        let (host, port) = if hp.starts_with('[') {
            match hp.find(']') {
                Some(close) => {
                    let h = &hp[..=close];
                    let rest = &hp[close + 1..];
                    (h.to_string(), rest.strip_prefix(':').and_then(|p| p.parse().ok()).unwrap_or(-1))
                }
                None => (hp.to_string(), -1),
            }
        } else {
            match hp.rfind(':') {
                Some(c) if hp[c + 1..].bytes().all(|b| b.is_ascii_digit()) => (hp[..c].to_string(), hp[c + 1..].parse().unwrap_or(-1)),
                _ => (hp.to_string(), -1),
            }
        };
        let valid_host = !host.is_empty() && host.chars().all(|c| c.is_ascii_alphanumeric() || "-.[]:".contains(c));
        if valid_host {
            self.user_info = ui;
            self.host = Some(host);
            self.port = port;
        }
    }

    /// `isAbsolute()`: `true` when the URI has a scheme.
    pub fn is_absolute(&self) -> bool {
        self.scheme.is_some()
    }

    /// `isOpaque()`: `true` for an absolute URI whose scheme-specific part does not start with `/`.
    pub fn is_opaque(&self) -> bool {
        self.opaque.is_some()
    }

    /// `getScheme()`: the scheme, `None` for a relative URI.
    pub fn get_scheme(&self) -> Option<&str> {
        self.scheme.as_deref()
    }

    /// `getHost()`: the host component, `None` when undefined.
    pub fn get_host(&self) -> Option<&str> {
        self.host.as_deref()
    }

    /// The raw (still percent-encoded) path, `None` for an opaque URI; [`Self::get_decoded_path`] decodes it.
    pub fn get_path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    /// `getRawAuthority()`
    pub fn get_raw_authority(&self) -> Option<&str> {
        self.authority.as_deref()
    }

    /// `getRawPath()`
    pub fn get_raw_path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    /// `getPath()`: the path with `%XX` escapes decoded (UTF-8)
    pub fn get_decoded_path(&self) -> Option<String> {
        self.path.as_deref().map(percent_decode)
    }

    /// The raw (still percent-encoded) fragment, `None` when undefined.
    pub fn get_fragment(&self) -> Option<&str> {
        self.fragment.as_deref()
    }

    /// The raw (still percent-encoded) query, `None` when undefined.
    pub fn get_query(&self) -> Option<&str> {
        self.query.as_deref()
    }

    /// `getRawSchemeSpecificPart()`
    pub fn get_raw_scheme_specific_part(&self) -> String {
        if let Some(o) = &self.opaque {
            return o.clone();
        }
        let s = self.to_java_string();
        let start = match &self.scheme {
            Some(sc) => sc.len() + 1,
            None => 0,
        };
        let end = match &self.fragment {
            Some(f) => s.len() - f.len() - 1,
            None => s.len(),
        };
        s[start.min(end)..end].to_string()
    }

    /// `getSchemeSpecificPart()` (decoded)
    pub fn get_scheme_specific_part(&self) -> String {
        percent_decode(&self.get_raw_scheme_specific_part())
    }

    /// `getFragment()` (decoded)
    pub fn get_decoded_fragment(&self) -> Option<String> {
        self.fragment.as_deref().map(percent_decode)
    }

    /// `URI.equals`: scheme and host case-insensitive, escapes compared with case-insensitive hex digits.
    pub fn java_equals(&self, that: &JavaUri) -> bool {
        fn eq(a: &Option<String>, b: &Option<String>) -> bool {
            match (a, b) {
                (None, None) => true,
                (Some(x), Some(y)) => escaped_eq(x, y),
                _ => false,
            }
        }
        fn eq_ic(a: &Option<String>, b: &Option<String>) -> bool {
            match (a, b) {
                (None, None) => true,
                (Some(x), Some(y)) => x.eq_ignore_ascii_case(y),
                _ => false,
            }
        }
        if self.is_opaque() != that.is_opaque() || !eq_ic(&self.scheme, &that.scheme) || !eq(&self.fragment, &that.fragment) {
            return false;
        }
        if self.is_opaque() {
            return eq(&self.opaque, &that.opaque);
        }
        if !eq(&self.path, &that.path) || !eq(&self.query, &that.query) {
            return false;
        }
        if self.authority.is_none() && that.authority.is_none() {
            return true;
        }
        if self.host.is_some() {
            eq(&self.user_info, &that.user_info) && eq_ic(&self.host, &that.host) && self.port == that.port
        } else if self.authority.is_some() {
            eq(&self.authority, &that.authority)
        } else {
            false
        }
    }

    /// `toString()`: the original string when parsed, else rebuilt from components.
    pub fn to_java_string(&self) -> String {
        if let Some(s) = &self.string {
            return s.clone();
        }
        self.define_string()
    }

    fn define_string(&self) -> String {
        let mut sb = String::new();
        if let Some(s) = &self.scheme {
            sb.push_str(s);
            sb.push(':');
        }
        if let Some(o) = &self.opaque {
            sb.push_str(o);
        } else {
            if let Some(h) = &self.host {
                sb.push_str("//");
                if let Some(u) = &self.user_info {
                    sb.push_str(u);
                    sb.push('@');
                }
                sb.push_str(h);
                if self.port != -1 {
                    sb.push(':');
                    sb.push_str(&self.port.to_string());
                }
            } else if let Some(a) = &self.authority {
                sb.push_str("//");
                sb.push_str(a);
            }
            if let Some(p) = &self.path {
                sb.push_str(p);
            }
            if let Some(q) = &self.query {
                sb.push('?');
                sb.push_str(q);
            }
        }
        if let Some(f) = &self.fragment {
            sb.push('#');
            sb.push_str(f);
        }
        sb
    }

    /// `URI.resolve(URI)` (RFC 2396 §5.2 with the JDK's normalization)
    pub fn resolve(&self, child: &JavaUri) -> JavaUri {
        if child.is_opaque() || self.is_opaque() {
            return child.clone();
        }
        // 5.2 (2): reference to current document (lone fragment)
        if child.scheme.is_none() && child.authority.is_none() && child.path.as_deref() == Some("") && child.fragment.is_some() && child.query.is_none() {
            if self.fragment.as_deref() == child.fragment.as_deref() {
                return self.clone();
            }
            let mut ru = self.clone();
            ru.string = None;
            ru.fragment = child.fragment.clone();
            return ru;
        }
        if child.scheme.is_some() {
            return child.clone();
        }
        let mut ru = JavaUri { scheme: self.scheme.clone(), opaque: None, authority: None, user_info: None, host: None, port: -1, path: None, query: child.query.clone(), fragment: child.fragment.clone(), string: None };
        if child.authority.is_none() {
            ru.authority = self.authority.clone();
            ru.host = self.host.clone();
            ru.user_info = self.user_info.clone();
            ru.port = self.port;
            let cp = child.path.clone().unwrap_or_default();
            if !cp.is_empty() && cp.starts_with('/') {
                ru.path = Some(cp);
            } else {
                ru.path = Some(resolve_path(self.path.as_deref().unwrap_or(""), &cp, self.is_absolute()));
            }
        } else {
            ru.authority = child.authority.clone();
            ru.host = child.host.clone();
            ru.user_info = child.user_info.clone();
            ru.port = child.port;
            ru.path = child.path.clone();
        }
        ru
    }

    /// `URI.normalize()`
    pub fn normalize(&self) -> JavaUri {
        if self.is_opaque() || self.path.is_none() || self.path.as_deref() == Some("") {
            return self.clone();
        }
        let np = normalize_path(self.path.as_deref().unwrap());
        if Some(np.as_str()) == self.path.as_deref() {
            return self.clone();
        }
        let mut v = self.clone();
        v.string = None;
        v.path = Some(np);
        v
    }
}

fn resolve_path(base: &str, child: &str, _absolute: bool) -> String {
    let i = base.rfind('/');
    let path = match i {
        None => child.to_string(),
        Some(i) => format!("{}{}", &base[..=i], child),
    };
    normalize_path(&path)
}

/// The JDK's `URI.normalize(String)`: removes `.` segments and `seg/..` pairs, keeping leading `..`, and
/// prefixes `./` when the first segment would otherwise be mistaken for a scheme.
pub fn normalize_path(ps: &str) -> String {
    let absolute = ps.starts_with('/');
    let segs: Vec<&str> = ps.split('/').collect();
    let trailing_slash = ps.ends_with('/') && ps.len() > 1;
    // segments excluding empty ones produced by the leading slash
    let mut out: Vec<String> = Vec::new();
    let items: Vec<&str> = if absolute { segs[1..].to_vec() } else { segs.clone() };
    let n = items.len();
    let mut ends_with_dir = trailing_slash;
    for (idx, seg) in items.iter().enumerate() {
        let last = idx == n - 1;
        match *seg {
            "." => {
                if last {
                    ends_with_dir = true;
                }
            }
            ".." => {
                if let Some(prev) = out.last() {
                    if prev != ".." {
                        out.pop();
                        if last {
                            ends_with_dir = true;
                        }
                        continue;
                    }
                }
                if absolute {
                    // cannot go above root: the JDK keeps ".." here
                    out.push("..".into());
                } else {
                    out.push("..".into());
                }
                if last {
                    ends_with_dir = true;
                }
            }
            "" => {
                if !last {
                    out.push(String::new());
                }
            }
            s => out.push(s.to_string()),
        }
    }
    let mut result = String::new();
    if absolute {
        result.push('/');
    }
    result.push_str(&out.join("/"));
    if ends_with_dir && !out.is_empty() && !result.ends_with('/') {
        result.push('/');
    }
    // maybeAddLeadingDot: a first segment containing ':' in a relative path
    if !absolute {
        if let Some(first) = out.first() {
            if first.contains(':') {
                result.insert_str(0, "./");
            }
        }
    }
    result
}

/// `java.net.URL`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JavaUrl {
    /// `getProtocol()`: the scheme, e.g. `http`.
    pub protocol: String,
    /// `getHost()`: the host, empty when the URL has none.
    pub host: String,
    /// `getPort()`: the explicit port, `-1` when none was given.
    pub port: i32,
    /// `getAuthority()`: host and port (with user info) as written, `None` when absent.
    pub authority: Option<String>,
    /// `getUserInfo()`: the user-info part of the authority, `None` when absent.
    pub user_info: Option<String>,
    /// path + ("?" + query)
    pub file: String,
    /// `getPath()`: the path without the query.
    pub path: String,
    /// `getQuery()`: the query without the leading `?`, `None` when absent.
    pub query: Option<String>,
    /// `getRef()`: the fragment (reference) without the leading `#`, `None` when absent.
    pub ref_: Option<String>,
}

const KNOWN_PROTOCOLS: &[&str] = &["http", "https", "ftp", "file", "jar", "mailto", "jrt"];

impl JavaUrl {
    /// `new URL(spec)`
    pub fn parse(spec: &str) -> JResult<JavaUrl> {
        JavaUrl::parse_with_context(None, spec)
    }

    /// `new URL(context, spec)`
    pub fn parse_with_context(context: Option<&JavaUrl>, original: &str) -> JResult<JavaUrl> {
        let chars: Vec<char> = original.chars().collect();
        let mut limit = chars.len();
        while limit > 0 && chars[limit - 1] <= ' ' {
            limit -= 1;
        }
        let mut start = 0;
        while start < limit && chars[start] <= ' ' {
            start += 1;
        }
        let mut spec: String = chars[start..limit].iter().collect();
        if spec.len() >= 4 && spec[..4].eq_ignore_ascii_case("url:") {
            spec = spec[4..].to_string();
        }
        let mut protocol: Option<String> = None;
        let sc: Vec<char> = spec.chars().collect();
        let is_rel_ref = !sc.is_empty() && sc[0] == '#';
        let mut i = 0;
        if !is_rel_ref {
            while i < sc.len() {
                let c = sc[i];
                if c == '/' {
                    break;
                }
                if c == ':' {
                    let s: String = sc[..i].iter().collect::<String>().to_lowercase();
                    if is_valid_protocol(&s) {
                        protocol = Some(s);
                        start = i + 1;
                    }
                    break;
                }
                i += 1;
            }
        }
        let mut start = if protocol.is_some() { start } else { 0 };
        let ctx = match (&protocol, context) {
            (Some(p), Some(c)) if c.protocol.eq_ignore_ascii_case(p) => {
                // "http:foo" with an http context: relative to the context unless it starts with "//"
                let rest: String = sc[start..].iter().collect();
                if c.path.starts_with('/') && rest.starts_with("//") { None } else { Some(c.clone()) }
            }
            (Some(_), _) => None,
            (None, c) => c.cloned(),
        };
        let protocol = match protocol.or_else(|| ctx.as_ref().map(|c| c.protocol.clone())) {
            Some(p) => p,
            None => return Err(JavaException::new("java.net.MalformedURLException", format!("no protocol: {original}"))),
        };
        if !KNOWN_PROTOCOLS.contains(&protocol.as_str()) {
            return Err(JavaException::new("java.net.MalformedURLException", format!("unknown protocol: {protocol}")));
        }
        let mut url = match &ctx {
            Some(c) => {
                let mut u = c.clone();
                u.ref_ = None;
                u
            }
            None => JavaUrl { protocol: protocol.clone(), host: String::new(), port: -1, authority: None, user_info: None, file: String::new(), path: String::new(), query: None, ref_: None },
        };
        url.protocol = protocol;
        // fragment
        let rest: Vec<char> = sc[start.min(sc.len())..].to_vec();
        let mut body: String = rest.iter().collect();
        if let Some(h) = body.find('#') {
            url.ref_ = Some(body[h + 1..].to_string());
            body.truncate(h);
        }
        parse_url_body(&mut url, &body, ctx.is_some())?;
        start = 0;
        let _ = start;
        Ok(url)
    }

    /// `toString()` / `toExternalForm()`
    pub fn to_external_form(&self) -> String {
        let mut s = format!("{}:", self.protocol);
        if let Some(a) = &self.authority {
            if !a.is_empty() || self.protocol == "file" && !self.host.is_empty() {
                s.push_str("//");
                s.push_str(a);
            } else if self.protocol != "file" {
                s.push_str("//");
            }
        }
        s.push_str(&self.file);
        if let Some(r) = &self.ref_ {
            s.push('#');
            s.push_str(r);
        }
        s
    }

    /// `toURI()`
    pub fn to_uri(&self) -> JResult<JavaUri> {
        JavaUri::parse(&self.to_external_form())
    }

    /// `URL.getHost()`: the host, empty when the URL has none.
    pub fn get_host(&self) -> &str {
        &self.host
    }

    /// `URL.getPort()`: the explicit port, `-1` when none was given.
    pub fn get_port(&self) -> i32 {
        self.port
    }

    /// `URL.getDefaultPort()`: `80` for http, `443` for https, `21` for ftp, `-1` otherwise.
    pub fn get_default_port(&self) -> i32 {
        match self.protocol.as_str() {
            "http" => 80,
            "https" => 443,
            "ftp" => 21,
            _ => -1,
        }
    }

    /// `URL.getProtocol()`: the scheme, e.g. `http`.
    pub fn get_protocol(&self) -> &str {
        &self.protocol
    }

    /// `URL.getPath()`: the path without the query.
    pub fn get_path(&self) -> &str {
        &self.path
    }

    /// `URL.getQuery()`: the query without the leading `?`, `None` when absent.
    pub fn get_query(&self) -> Option<&str> {
        self.query.as_deref()
    }

    /// `URL.getFile()`: the path followed by `?` and the query when there is one.
    pub fn get_file(&self) -> &str {
        &self.file
    }

    /// `URL.getRef()`: the fragment without the leading `#`, `None` when absent.
    pub fn get_ref(&self) -> Option<&str> {
        self.ref_.as_deref()
    }
}

impl std::fmt::Display for JavaUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_external_form())
    }
}

fn is_valid_protocol(s: &str) -> bool {
    let mut it = s.chars();
    match it.next() {
        Some(c) if c.is_ascii_alphabetic() => it.all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '+' || c == '-'),
        _ => false,
    }
}

/// `URLStreamHandler.parseURL` for hierarchical specs.
fn parse_url_body(url: &mut JavaUrl, spec: &str, has_context: bool) -> JResult<()> {
    let mut spec = spec.to_string();
    let mut query: Option<String> = if has_context { url.query.clone() } else { None };
    if let Some(q) = spec.find('?') {
        query = Some(spec[q + 1..].to_string());
        spec.truncate(q);
    } else if !spec.is_empty() {
        // a new path without query drops the context query
        query = None;
    }
    let mut path = if has_context { url.path.clone() } else { String::new() };
    if spec.starts_with("//") && !(url.protocol == "file" && spec.starts_with("////")) {
        let rest = &spec[2..];
        let end = rest.find('/').unwrap_or(rest.len());
        let authority = rest[..end].to_string();
        let (user_info, hostport) = match authority.rfind('@') {
            Some(at) => (Some(authority[..at].to_string()), authority[at + 1..].to_string()),
            None => (None, authority.clone()),
        };
        let (host, port) = if hostport.starts_with('[') {
            match hostport.find(']') {
                Some(c) => {
                    let p = hostport[c + 1..].strip_prefix(':').map(|p| p.to_string());
                    (hostport[..=c].to_string(), p)
                }
                None => return Err(JavaException::new("java.net.MalformedURLException", format!("Invalid host: {hostport}"))),
            }
        } else {
            match hostport.find(':') {
                Some(c) => (hostport[..c].to_string(), Some(hostport[c + 1..].to_string())),
                None => (hostport.clone(), None),
            }
        };
        let port = match port {
            Some(p) if !p.is_empty() => {
                if !p.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(JavaException::new("java.net.MalformedURLException", format!("For input string: \"{p}\"")));
                }
                p.parse::<i32>().map_err(|_| JavaException::new("java.net.MalformedURLException", format!("Invalid port number :{p}")))?
            }
            _ => -1,
        };
        url.host = host;
        url.port = port;
        url.user_info = user_info;
        url.authority = Some(authority);
        path = rest[end..].to_string();
        query = spec.find('?').map(|_| String::new()).or(query);
    } else if !spec.is_empty() {
        if spec.starts_with('/') {
            path = spec.clone();
        } else if !path.is_empty() {
            let base = match path.rfind('/') {
                Some(i) => path[..=i].to_string(),
                None => String::new(),
            };
            let base = if base.is_empty() && url.authority.is_some() { "/".to_string() } else { base };
            // only a path merged with the context's (`isRelPath`) has its dot segments removed
            path = remove_dot_segments_url(&format!("{base}{spec}"));
        } else {
            let prefix = if url.authority.is_some() { "/" } else { "" };
            path = format!("{prefix}{spec}");
        }
    }
    if url.authority.is_none() && url.protocol == "file" {
        url.authority = Some(String::new());
    }
    url.path = path.clone();
    url.query = query.clone();
    url.file = match &query {
        Some(q) => format!("{path}?{q}"),
        None => path,
    };
    Ok(())
}

/// `URLStreamHandler.parseURL` dot-segment handling: removes `/./`, `/seg/../`, a trailing `/.` and `/seg/..`.
fn remove_dot_segments_url(path: &str) -> String {
    let mut p = path.to_string();
    while let Some(i) = p.find("/./") {
        p.replace_range(i..i + 2, "");
    }
    while let Some(i) = p.find("/../") {
        if i == 0 {
            break;
        }
        match p[..i].rfind('/') {
            Some(prev) if &p[prev..i] != "/.." => {
                p.replace_range(prev..i + 3, "");
            }
            _ => break,
        }
    }
    while p.ends_with("/..") {
        let i = p.len() - 3;
        match p[..i].rfind('/') {
            Some(prev) if i > 0 => {
                p.truncate(prev + 1);
            }
            _ => break,
        }
    }
    if p.ends_with("/.") {
        p.truncate(p.len() - 1);
    }
    p
}

fn escaped_eq(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    if a.len() != b.len() || !a.contains('%') {
        return false;
    }
    let (ab, bb) = (a.as_bytes(), b.as_bytes());
    let mut i = 0;
    while i < ab.len() {
        if ab[i] == b'%' && bb[i] == b'%' && i + 2 < ab.len() {
            if !ab[i + 1].eq_ignore_ascii_case(&bb[i + 1]) || !ab[i + 2].eq_ignore_ascii_case(&bb[i + 2]) {
                return false;
            }
            i += 3;
            continue;
        }
        if ab[i] != bb[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// `%XX` decoding (UTF-8, malformed sequences replaced)
pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = ((b[i + 1] as char).to_digit(16), (b[i + 2] as char).to_digit(16)) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `isValidURL`: `new URL(s).toURI()` succeeds.
pub fn is_valid_url(s: &str) -> bool {
    JavaUrl::parse(s).and_then(|u| u.to_uri()).is_ok()
}

/// `isValidURI`: `new URI(s)` succeeds.
pub fn is_valid_uri(s: &str) -> bool {
    JavaUri::parse(s).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_errors_and_resolution() {
        assert!(JavaUri::parse("http://a/b c").unwrap_err().message.unwrap().starts_with("Illegal character in path at index 10"));
        assert_eq!(JavaUri::parse("http://x/a/b").unwrap().resolve(&JavaUri::parse("../c").unwrap()).to_java_string(), "http://x/c");
        assert_eq!(JavaUri::parse("http://x/a/b").unwrap().resolve(&JavaUri::parse("/v1").unwrap()).to_java_string(), "http://x/v1");
        assert_eq!(JavaUrl::parse_with_context(Some(&JavaUrl::parse("http://x.com/a/b.yaml").unwrap()), "./c/d.yaml").unwrap().to_external_form(), "http://x.com/a/c/d.yaml");
        assert_eq!(JavaUrl::parse_with_context(Some(&JavaUrl::parse("http://x.com/a/b/").unwrap()), "../d.yaml").unwrap().to_external_form(), "http://x.com/a/d.yaml");
        assert!(JavaUrl::parse("foo").is_err());
        assert_eq!(JavaUrl::parse("file:/tmp/a.yaml").unwrap().to_external_form(), "file:/tmp/a.yaml");
    }
}

#[cfg(test)]
mod url_context_tests {
    use super::JavaUrl;

    /// Expected values printed by `new URL(context, spec)` on JDK 25.
    #[test]
    fn dot_segments_are_removed_only_from_relative_merges() {
        let cases = [
            (Some("http://foo.bar.com/my/dir/file.yaml"), "/second/../newFile.yaml", "http://foo.bar.com/second/../newFile.yaml"),
            (Some("http://foo.bar.com/my/dir/file.yaml"), "./second/../newFile.yaml", "http://foo.bar.com/my/dir/newFile.yaml"),
            (Some("http://foo.bar.com"), "a/./b/../c", "http://foo.bar.com/a/./b/../c"),
            (Some("http://foo.bar.com/x"), "//h2/a/../b", "http://h2/a/../b"),
            (None, "http://h/a/../b/./c", "http://h/a/../b/./c"),
            (Some("file:/a/b/c.yaml"), "/d/./e/../f", "file:/d/./e/../f"),
            (Some("http://h/a/b"), "../../../x", "http://h/../../x"),
        ];
        for (ctx, spec, expected) in cases {
            let c = ctx.map(|c| JavaUrl::parse(c).unwrap());
            assert_eq!(JavaUrl::parse_with_context(c.as_ref(), spec).unwrap().to_external_form(), expected, "{ctx:?} + {spec}");
        }
    }
}
