//! A port of swagger-parser-safe-url-resolver 2.1.48 (`io.swagger.v3.parser.urlresolver`): the allow/deny
//! list and restricted-IP check applied to remote `$ref` URLs.
//!
//! Java's `NetUtils` is a class of static methods that the upstream tests replace with JMockit; here it is
//! the [`NetUtils`] trait, [`SystemNetUtils`] being the real implementation, and checkers and matchers take
//! the implementation to use.

#![warn(missing_docs)]

use java_rt::inet::InetAddress;
use java_rt::net::JavaUrl;
use java_rt::{JResult, JavaException, idn};
use std::fmt;
use std::sync::Arc;

pub use java_rt;

/// The class name of `HostDeniedException`, thrown when a `$ref` URL is not permitted.
pub const HOST_DENIED_EXCEPTION: &str = "io.swagger.v3.parser.urlresolver.exceptions.HostDeniedException";

fn host_denied(message: String, cause: Option<JavaException>) -> JavaException {
    let e = JavaException::new(HOST_DENIED_EXCEPTION, message);
    match cause {
        Some(c) => e.with_cause(c),
        None => e,
    }
}

/// `io.swagger.v3.parser.urlresolver.utils.NetUtils`
pub trait NetUtils: Send + Sync {
    /// `getHostByName(hostname)` (`UnknownHostException`)
    fn get_host_by_name(&self, hostname: &str) -> JResult<InetAddress> {
        system::get_host_by_name(hostname)
    }
    /// `getHostFromUrl(url)` (`MalformedURLException`)
    fn get_host_from_url(&self, url: &str) -> JResult<String> {
        system::get_host_from_url(url)
    }
    /// `setHost(url, host)` (`MalformedURLException`)
    fn set_host(&self, url: &str, host: &str) -> JResult<String> {
        system::set_host(url, host)
    }
    /// `isIPv4(ipAddress)`: `true` when the text resolves to an IPv4 address.
    fn is_ipv4(&self, ip_address: &str) -> bool {
        system::is_ipv4(ip_address)
    }
    /// `isIPv6(ipAddress)`: `true` when the text resolves to an IPv6 address.
    fn is_ipv6(&self, ip_address: &str) -> bool {
        system::is_ipv6(ip_address)
    }
    /// `isUniqueLocalAddress(ip)`: `true` for an IPv6 address in `fc00::/7`.
    fn is_unique_local_address(&self, ip: &InetAddress) -> bool {
        system::is_unique_local_address(ip)
    }
    /// `isNAT64Address(ip)`: `true` for an IPv6 address in the `64:ff9b::/96` well-known prefix.
    fn is_nat64_address(&self, ip: &InetAddress) -> bool {
        system::is_nat64_address(ip)
    }
}

/// The real `NetUtils`.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemNetUtils;

impl NetUtils for SystemNetUtils {}

/// The static `NetUtils` methods.
pub mod system {
    use super::*;

    /// `NetUtils.getHostByName(hostname)`: `InetAddress.getByName`, `Err` (`UnknownHostException`) when it does not resolve.
    pub fn get_host_by_name(hostname: &str) -> JResult<InetAddress> {
        InetAddress::get_by_name(hostname)
    }

    /// `NetUtils.getHostFromUrl(url)`: the URL's host with IPv6 brackets stripped, `Err` (`MalformedURLException`) on a bad URL.
    pub fn get_host_from_url(url: &str) -> JResult<String> {
        let h = JavaUrl::parse(url)?.get_host().to_string();
        if h.chars().count() > 2 && h.starts_with('[') && h.ends_with(']') {
            return Ok(h[1..h.len() - 1].to_string());
        }
        Ok(h)
    }

    /// `NetUtils.setHost(url, host)`: the URL with its host text replaced by `host` (bracketed when IPv6), `Err` on a bad URL.
    pub fn set_host(url: &str, host: &str) -> JResult<String> {
        let parsed = JavaUrl::parse(url)?;
        let target = if is_ipv6(host) { format!("[{host}]") } else { host.to_string() };
        Ok(java_replace(url, parsed.get_host(), &target))
    }

    /// `NetUtils.isIPv4(ipAddress)`: `true` when the text resolves to an IPv4 address.
    pub fn is_ipv4(ip_address: &str) -> bool {
        matches!(InetAddress::get_by_name(ip_address), Ok(InetAddress::V4 { .. }))
    }

    /// `NetUtils.isIPv6(ipAddress)`: `true` when the text resolves to an IPv6 address.
    pub fn is_ipv6(ip_address: &str) -> bool {
        matches!(InetAddress::get_by_name(ip_address), Ok(InetAddress::V6 { .. }))
    }

    /// `NetUtils.isUniqueLocalAddress(ip)`: `true` for an IPv6 address in `fc00::/7`.
    pub fn is_unique_local_address(ip: &InetAddress) -> bool {
        match ip {
            InetAddress::V4 { .. } => false,
            InetAddress::V6 { addr, .. } => addr[0] == 0xfc || addr[0] == 0xfd,
        }
    }

    /// `NetUtils.isNAT64Address(ip)`: `true` for an IPv6 address in the `64:ff9b::/96` well-known prefix.
    pub fn is_nat64_address(ip: &InetAddress) -> bool {
        match ip {
            InetAddress::V4 { .. } => false,
            InetAddress::V6 { addr, .. } => addr[0] == 0x00 && addr[1] == 0x64 && addr[2] == 0xff && addr[3] == 0x9b,
        }
    }

    /// `String.replace(CharSequence, CharSequence)` (an empty target inserts the replacement between chars)
    fn java_replace(s: &str, target: &str, replacement: &str) -> String {
        if !target.is_empty() {
            return s.replace(target, replacement);
        }
        let mut out = String::from(replacement);
        for c in s.chars() {
            out.push(c);
            out.push_str(replacement);
        }
        out
    }
}

/// `org.apache.commons.io.FilenameUtils.wildcardMatch(name, pattern)` (case-sensitive; `?` one character,
/// `*` any run).
pub fn wildcard_match(name: &str, pattern: &str) -> bool {
    let n: Vec<char> = name.chars().collect();
    let p: Vec<char> = pattern.chars().collect();
    let (mut i, mut j) = (0usize, 0usize);
    let mut star: Option<(usize, usize)> = None;
    while i < n.len() {
        if j < p.len() && (p[j] == '?' || p[j] == n[i]) {
            i += 1;
            j += 1;
        } else if j < p.len() && p[j] == '*' {
            star = Some((j, i));
            j += 1;
        } else if let Some((sj, si)) = star {
            j = sj + 1;
            i = si + 1;
            star = Some((sj, si + 1));
        } else {
            return false;
        }
    }
    while j < p.len() && p[j] == '*' {
        j += 1;
    }
    j == p.len()
}

/// `io.swagger.v3.parser.urlresolver.matchers.UrlPatternMatcher`
#[derive(Clone, Debug)]
pub struct UrlPatternMatcher {
    patterns: Vec<String>,
}

impl UrlPatternMatcher {
    /// `new UrlPatternMatcher(patterns)` with the system `NetUtils` (`IDN.toASCII` may throw
    /// `IllegalArgumentException`).
    pub fn new<S: AsRef<str>>(patterns: &[S]) -> JResult<UrlPatternMatcher> {
        UrlPatternMatcher::with_net_utils(patterns, &SystemNetUtils)
    }

    /// [`UrlPatternMatcher::new`] with the given [`NetUtils`] in place of the system one.
    pub fn with_net_utils<S: AsRef<str>>(patterns: &[S], net: &dyn NetUtils) -> JResult<UrlPatternMatcher> {
        let mut out = Vec::new();
        for pattern in patterns {
            let pattern = pattern.as_ref();
            let lower = pattern.to_lowercase();
            let host_and_port = if pattern.contains(':') { lower } else { format!("{lower}:*") };
            let split = java_split_colon(&host_and_port);
            let Some(first) = split.first() else {
                return Err(JavaException::new("java.lang.ArrayIndexOutOfBoundsException", "Index 0 out of bounds for length 0"));
            };
            let Some(c0) = first.chars().next() else {
                return Err(JavaException::new("java.lang.StringIndexOutOfBoundsException", "Index 0 out of bounds for length 0"));
            };
            let host = if c0.is_numeric() { first.to_string() } else { idn::to_ascii(first)? };
            let port = if split.len() > 1 { split[1].clone() } else { "*".to_string() };
            // Ignore domains that end in a wildcard
            if host.chars().count() > 1 && !net.is_ipv4(&host.replace('*', "0")) && host.ends_with('*') {
                continue;
            }
            out.push(format!("{host}:{port}"));
        }
        Ok(UrlPatternMatcher { patterns: out })
    }

    /// The normalized `host:port` patterns.
    pub fn patterns(&self) -> &[String] {
        &self.patterns
    }

    /// `matches(url)`
    pub fn matches(&self, url: &str) -> JResult<bool> {
        let Ok(parsed) = JavaUrl::parse(&url.to_lowercase()) else { return Ok(false) };
        let host = idn::to_ascii(parsed.get_host())?;
        let host_and_port = if parsed.get_port() == -1 {
            match parsed.get_protocol() {
                "http" => format!("{host}:80"),
                "https" => format!("{host}:443"),
                _ => return Ok(false),
            }
        } else {
            format!("{host}:{}", parsed.get_port())
        };
        Ok(self.patterns.iter().any(|p| wildcard_match(&host_and_port, p)))
    }
}

/// `String.split(":")`: trailing empty strings removed (an input made only of separators gives nothing).
fn java_split_colon(s: &str) -> Vec<String> {
    if s.is_empty() {
        return vec![String::new()];
    }
    let mut v: Vec<String> = s.split(':').map(str::to_string).collect();
    while v.last().is_some_and(|x| x.is_empty()) {
        v.pop();
    }
    v
}

/// `io.swagger.v3.parser.urlresolver.models.ResolvedUrl`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedUrl {
    /// `getUrl()`: the URL to fetch, its host possibly replaced by a resolved IP address.
    pub url: String,
    /// `getHostHeader()`: the original host, sent as the `Host` header.
    pub host_header: String,
}

impl ResolvedUrl {
    /// `new ResolvedUrl(url, hostHeader)`.
    pub fn new(url: impl Into<String>, host_header: impl Into<String>) -> ResolvedUrl {
        ResolvedUrl { url: url.into(), host_header: host_header.into() }
    }
    /// `getUrl()`: the URL to fetch.
    pub fn get_url(&self) -> &str {
        &self.url
    }
    /// `getHostHeader()`: the value for the `Host` header.
    pub fn get_host_header(&self) -> &str {
        &self.host_header
    }
    /// `setUrl(url)`.
    pub fn set_url(&mut self, url: impl Into<String>) {
        self.url = url.into();
    }
    /// `setHostHeader(hostHeader)`.
    pub fn set_host_header(&mut self, h: impl Into<String>) {
        self.host_header = h.into();
    }
}

impl fmt::Display for ResolvedUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ResolvedUrl{{url='{}', hostHeader='{}'}}", self.url, self.host_header)
    }
}

/// An overriding `isRestrictedIpRange(InetAddress)`.
type RestrictedIpRange = Arc<dyn Fn(&InetAddress) -> bool + Send + Sync>;

/// `io.swagger.v3.parser.urlresolver.PermittedUrlsChecker`
#[derive(Clone)]
pub struct PermittedUrlsChecker {
    /// `allowlist`: a URL (or its IP-substituted form) it matches is permitted before the restricted-IP check.
    pub allowlist_matcher: UrlPatternMatcher,
    /// Matches the denylist patterns; a URL it matches is denied.
    pub denylist_matcher: UrlPatternMatcher,
    net: Arc<dyn NetUtils>,
    /// An overridden `isRestrictedIpRange` (a subclass in Java)
    restricted_ip_range: Option<RestrictedIpRange>,
}

impl fmt::Debug for PermittedUrlsChecker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PermittedUrlsChecker")
            .field("allowlist_matcher", &self.allowlist_matcher)
            .field("denylist_matcher", &self.denylist_matcher)
            .finish()
    }
}

impl Default for PermittedUrlsChecker {
    /// `new PermittedUrlsChecker()`
    fn default() -> Self {
        PermittedUrlsChecker {
            allowlist_matcher: UrlPatternMatcher { patterns: vec![] },
            denylist_matcher: UrlPatternMatcher { patterns: vec![] },
            net: Arc::new(SystemNetUtils),
            restricted_ip_range: None,
        }
    }
}

impl PermittedUrlsChecker {
    /// `new PermittedUrlsChecker(allowlist, denylist)` (null lists are empty)
    pub fn new(allowlist: Option<&[String]>, denylist: Option<&[String]>) -> JResult<PermittedUrlsChecker> {
        PermittedUrlsChecker::with_net_utils(allowlist, denylist, Arc::new(SystemNetUtils))
    }

    /// [`PermittedUrlsChecker::new`] with the given [`NetUtils`] in place of the system one.
    pub fn with_net_utils(allowlist: Option<&[String]>, denylist: Option<&[String]>, net: Arc<dyn NetUtils>) -> JResult<PermittedUrlsChecker> {
        let empty: &[String] = &[];
        Ok(PermittedUrlsChecker {
            allowlist_matcher: UrlPatternMatcher::with_net_utils(allowlist.unwrap_or(empty), net.as_ref())?,
            denylist_matcher: UrlPatternMatcher::with_net_utils(denylist.unwrap_or(empty), net.as_ref())?,
            net,
            restricted_ip_range: None,
        })
    }

    /// Replaces `isRestrictedIpRange` (what a Java subclass overriding it does).
    pub fn with_restricted_ip_range(mut self, f: Arc<dyn Fn(&InetAddress) -> bool + Send + Sync>) -> PermittedUrlsChecker {
        self.restricted_ip_range = Some(f);
        self
    }

    /// `verify(url)`: `HostDeniedException` (class [`HOST_DENIED_EXCEPTION`]) when the URL is not permitted.
    pub fn verify(&self, url: &str) -> JResult<ResolvedUrl> {
        let parsed = match JavaUrl::parse(url) {
            Ok(p) => p,
            Err(e) => return Err(host_denied(format!("Failed to parse URL. URL [{url}]"), Some(e))),
        };
        if parsed.get_protocol() != "http" && parsed.get_protocol() != "https" {
            return Err(host_denied(format!("URL does not use a supported protocol. URL [{url}]"), None));
        }
        let hostname = match self.net.get_host_from_url(url) {
            Ok(h) => h,
            Err(e) => return Err(host_denied(format!("Failed to get hostname from URL. URL [{url}]"), Some(e))),
        };
        if self.allowlist_matcher.matches(url)? {
            return Ok(ResolvedUrl::new(url, hostname));
        }
        if self.denylist_matcher.matches(url)? {
            return Err(host_denied(format!("URL is part of the explicit denylist. URL [{url}]"), None));
        }
        let ip = match self.net.get_host_by_name(&hostname) {
            Ok(ip) => ip,
            Err(e) => return Err(host_denied(format!("Failed to resolve IP from hostname. Hostname [{hostname}]"), Some(e))),
        };
        let url_with_ip = match self.net.set_host(url, &ip.get_host_address()) {
            Ok(u) => u,
            Err(e) => {
                return Err(host_denied(format!("Failed to create new URL with IP. IP [{}] URL [{url}]", ip.get_host_address()), Some(e)));
            }
        };
        if self.allowlist_matcher.matches(&url_with_ip)? {
            return Ok(ResolvedUrl::new(url_with_ip, hostname));
        }
        if self.is_restricted_ip_range(&ip) {
            return Err(host_denied(format!("IP is restricted. URL [{url_with_ip}]"), None));
        }
        if self.denylist_matcher.matches(&url_with_ip)? {
            return Err(host_denied(format!("IP is part of the explicit denylist. URL [{url_with_ip}]"), None));
        }
        Ok(ResolvedUrl::new(url_with_ip, hostname))
    }

    /// `isRestrictedIpRange(ip)`
    pub fn is_restricted_ip_range(&self, ip: &InetAddress) -> bool {
        if let Some(f) = &self.restricted_ip_range {
            return f(ip);
        }
        ip.is_link_local_address()
            || ip.is_site_local_address()
            || ip.is_loopback_address()
            || ip.is_any_local_address()
            || self.net.is_unique_local_address(ip)
            || self.net.is_nat64_address(ip)
    }
}
