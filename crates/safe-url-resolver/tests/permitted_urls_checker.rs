//! Port of `PermittedUrlsCheckerTest`. The Java test mocks every static `NetUtils` method with JMockit
//! (`@Mocked NetUtils`): recorded expectations return their result and must be met the recorded number of
//! times, unrecorded calls return the type's default (`null` / `false`). [`MockNet`] does the same.

use parking_lot::Mutex;
use std::sync::Arc;
use swagger_parser_safe_url_resolver::java_rt::inet::InetAddress;
use swagger_parser_safe_url_resolver::java_rt::{JResult, JavaException};
use swagger_parser_safe_url_resolver::{HOST_DENIED_EXCEPTION, NetUtils, PermittedUrlsChecker, ResolvedUrl};

#[derive(Clone)]
enum Ret {
    Str(String),
    Ip(InetAddress),
    Bool(bool),
}

struct Expectation {
    method: &'static str,
    /// `None` for `withInstanceOf(InetAddress.class)`
    arg: Option<Vec<String>>,
    result: Ret,
    times: Option<usize>,
    calls: usize,
}

#[derive(Default)]
struct MockNet {
    expectations: Mutex<Vec<Expectation>>,
}

impl MockNet {
    fn expect(&self, method: &'static str, arg: &[&str], result: Ret, times: Option<usize>) {
        self.expectations.lock().push(Expectation { method, arg: Some(arg.iter().map(|s| s.to_string()).collect()), result, times, calls: 0 });
    }

    fn expect_any(&self, method: &'static str, result: Ret, times: Option<usize>) {
        self.expectations.lock().push(Expectation { method, arg: None, result, times, calls: 0 });
    }

    fn call(&self, method: &'static str, args: &[&str]) -> Option<Ret> {
        let mut ex = self.expectations.lock();
        for e in ex.iter_mut() {
            if e.method == method && e.arg.as_ref().is_none_or(|a| a.iter().map(String::as_str).eq(args.iter().copied())) {
                e.calls += 1;
                return Some(e.result.clone());
            }
        }
        None
    }

    fn verify(&self) {
        for e in self.expectations.lock().iter() {
            match e.times {
                Some(t) => assert_eq!(e.calls, t, "{} {:?}: expected {t} invocation(s), got {}", e.method, e.arg, e.calls),
                None => assert!(e.calls >= 1, "{} {:?}: missing invocation", e.method, e.arg),
            }
        }
    }
}

fn npe() -> JavaException {
    JavaException::without_message("java.lang.NullPointerException")
}

impl NetUtils for MockNet {
    fn get_host_by_name(&self, hostname: &str) -> JResult<InetAddress> {
        match self.call("getHostByName", &[hostname]) {
            Some(Ret::Ip(ip)) => Ok(ip),
            _ => Err(npe()),
        }
    }
    fn get_host_from_url(&self, url: &str) -> JResult<String> {
        match self.call("getHostFromUrl", &[url]) {
            Some(Ret::Str(s)) => Ok(s),
            // null
            _ => Ok(String::new()),
        }
    }
    fn set_host(&self, url: &str, host: &str) -> JResult<String> {
        match self.call("setHost", &[url, host]) {
            Some(Ret::Str(s)) => Ok(s),
            _ => Err(npe()),
        }
    }
    fn is_ipv4(&self, ip: &str) -> bool {
        matches!(self.call("isIPv4", &[ip]), Some(Ret::Bool(true)))
    }
    fn is_ipv6(&self, ip: &str) -> bool {
        matches!(self.call("isIPv6", &[ip]), Some(Ret::Bool(true)))
    }
    fn is_unique_local_address(&self, _ip: &InetAddress) -> bool {
        matches!(self.call("isUniqueLocalAddress", &[]), Some(Ret::Bool(true)))
    }
    fn is_nat64_address(&self, _ip: &InetAddress) -> bool {
        matches!(self.call("isNAT64Address", &[]), Some(Ret::Bool(true)))
    }
}

fn ip(s: &str) -> Ret {
    Ret::Ip(InetAddress::get_by_name(s).unwrap())
}

fn s(v: &str) -> Ret {
    Ret::Str(v.to_string())
}

fn checker(mock: &Arc<MockNet>, allow: &[&str], deny: &[&str]) -> PermittedUrlsChecker {
    let allow: Vec<String> = allow.iter().map(|x| x.to_string()).collect();
    let deny: Vec<String> = deny.iter().map(|x| x.to_string()).collect();
    PermittedUrlsChecker::with_net_utils(Some(&allow), Some(&deny), mock.clone()).unwrap()
}

/// `expectedExceptions = HostDeniedException.class, expectedExceptionsMessageRegExp = ".*<text>.*"`
fn expect_denied(r: JResult<ResolvedUrl>, text: &str) {
    let e = r.expect_err("expected HostDeniedException");
    assert_eq!(e.class, HOST_DENIED_EXCEPTION);
    let m = e.message.unwrap_or_default();
    assert!(m.contains(text), "message {m:?} does not match .*{text}.*");
}

/// The common `getHostFromUrl` → `getHostByName` → `setHost` recording.
fn record_resolution(mock: &MockNet, url: &str, host: &str, lookup: &str, resolved: Ret, ip_text: &str, url_with_ip: &str) {
    mock.expect("getHostFromUrl", &[url], s(host), Some(1));
    mock.expect("getHostByName", &[lookup], resolved, Some(1));
    mock.expect("setHost", &[url, ip_text], s(url_with_ip), Some(1));
}

#[test]
fn should_reject_private_siit_ipv4in6_host_references_in_abcd_format() {
    let mock = Arc::new(MockNet::default());
    let url = "https://[0:0:0:0:0:ffff:10.1.33.147]:8000/v1/operation?theThing=something";
    let expected_ip = "10.1.33.147";
    record_resolution(&mock, url, expected_ip, expected_ip, ip(expected_ip), expected_ip, url);
    expect_denied(checker(&mock, &[], &[]).verify(url), "IP is restricted");
    mock.verify();
}

#[test]
fn should_allow_public_siit_ipv4in6_host_references_in_abcd_format() {
    let mock = Arc::new(MockNet::default());
    let url = "https://[0:0:0:0:0:ffff:1.2.3.4]:8000/v1/operation?theThing=something";
    let expected_ip = "1.2.3.4";
    let expected_url = "https://1.2.3.4:8000/v1/operation?theThing=something";
    let expected_host_header = "0:0:0:0:0:ffff:1.2.3.4";
    record_resolution(&mock, url, expected_host_header, expected_host_header, ip(expected_ip), expected_ip, expected_url);
    let result = checker(&mock, &[], &[]).verify(url).unwrap();
    assert_eq!(result.get_url(), expected_url);
    assert_eq!(result.get_host_header(), expected_host_header);
    mock.verify();
}

#[test]
fn should_reject_private_siit_ipv4in6_host_references_in_ipv6_format() {
    let mock = Arc::new(MockNet::default());
    let url = "https://[0:0:0:0:0:ffff:a01:219]:8000/v1/operation?theThing=something";
    let expected_ip = "10.1.2.25";
    record_resolution(&mock, url, expected_ip, expected_ip, ip(expected_ip), expected_ip, url);
    expect_denied(checker(&mock, &[], &[]).verify(url), "IP is restricted");
    mock.verify();
}

#[test]
fn should_reject_nat64_host_references() {
    let mock = Arc::new(MockNet::default());
    let url = "https://[64:ff9b::]:8000/v1/operation?theThing=something";
    let expected_ip = "64:ff9b:0:0:0:0:0:0";
    record_resolution(&mock, url, expected_ip, expected_ip, ip(expected_ip), expected_ip, url);
    mock.expect_any("isNAT64Address", Ret::Bool(true), Some(1));
    expect_denied(checker(&mock, &[], &[]).verify(url), "IP is restricted");
    mock.verify();
}

#[test]
fn should_reject_decimal_ips_that_resolve_to_local_ips() {
    let mock = Arc::new(MockNet::default());
    let url = "https://3232235778:8000/api/v3/pet/findByStatus?status=available";
    let expected_ip = "192.168.1.2";
    record_resolution(&mock, url, expected_ip, expected_ip, ip(expected_ip), expected_ip, url);
    expect_denied(checker(&mock, &[], &[]).verify(url), "IP is restricted");
    mock.verify();
}

#[test]
fn should_prefer_allowlist_over_everything_else() {
    let mock = Arc::new(MockNet::default());
    let url = "https://localhost:3000/1";
    mock.expect("getHostFromUrl", &[url], s("localhost"), Some(1));
    let result = checker(&mock, &["localhost"], &[]).verify(url).unwrap();
    assert_eq!(result.get_url(), "https://localhost:3000/1");
    assert_eq!(result.get_host_header(), "localhost");
    mock.verify();
}

#[test]
fn should_allow_public_domains_by_default() {
    let mock = Arc::new(MockNet::default());
    let url = "https://smartbear.com:3000/1";
    let expected_url = "https://1.2.3.4:3000/1";
    let expected_host = "smartbear.com";
    let expected_ip = "1.2.3.4";
    record_resolution(&mock, url, expected_host, expected_host, ip(expected_ip), expected_ip, expected_url);
    let result = checker(&mock, &[], &[]).verify(url).unwrap();
    assert_eq!(result.get_url(), expected_url);
    assert_eq!(result.get_host_header(), expected_host);
    mock.verify();
}

#[test]
fn should_allow_public_ips_by_default() {
    let mock = Arc::new(MockNet::default());
    let url = "https://1.2.3.4:3000/1";
    let expected_host = "1.2.3.4";
    record_resolution(&mock, url, expected_host, expected_host, ip(expected_host), expected_host, url);
    let result = checker(&mock, &[], &[]).verify(url).unwrap();
    assert_eq!(result.get_url(), url);
    assert_eq!(result.get_host_header(), expected_host);
    mock.verify();
}

/// `@DataProvider shouldBlockRestrictedIPv4sByDefault`
#[test]
fn should_block_ipv4_localhost() {
    for (url, expected_ip) in [
        ("https://localhost:3000/1", "127.0.0.1"),
        ("https://127.0.0.1/", "127.0.0.1"),
        ("https://192.168.1.2/", "192.168.1.2"),
        ("https://127.3", "127.0.0.3"),
    ] {
        let mock = Arc::new(MockNet::default());
        record_resolution(&mock, url, expected_ip, expected_ip, ip(expected_ip), expected_ip, url);
        expect_denied(checker(&mock, &[], &[]).verify(url), "IP is restricted");
        mock.verify();
    }
}

/// `@DataProvider shouldBlockRestrictedIPv6sByDefault`
#[test]
fn should_block_ipv6_localhost() {
    for (url, expected_ip) in [("https://[fc00::1]/", "fc00:0:0:0:0:0:0:1"), ("https://[fd00:ec2::254]/", "fd00:ec2:0:0:0:0:0:254")] {
        let mock = Arc::new(MockNet::default());
        record_resolution(&mock, url, expected_ip, expected_ip, ip(expected_ip), expected_ip, url);
        mock.expect_any("isUniqueLocalAddress", Ret::Bool(true), None);
        expect_denied(checker(&mock, &[], &[]).verify(url), "IP is restricted");
        mock.verify();
    }
}

#[test]
fn should_block_domain_names_that_resolve_to_private_ips() {
    let mock = Arc::new(MockNet::default());
    let url = "https://evil.com";
    let expected_url = "https://192.168.1.1:3000/1";
    let expected_host = "evil.com";
    let expected_ip = "192.168.1.1";
    record_resolution(&mock, url, expected_host, expected_host, ip(expected_ip), expected_ip, expected_url);
    expect_denied(checker(&mock, &[], &[]).verify(url), "IP is restricted");
    mock.verify();
}

#[test]
fn should_block_specifically_denylisted_urls() {
    let mock = Arc::new(MockNet::default());
    let url = "https://smartbear.com";
    expect_denied(checker(&mock, &[], &["smartbear.com"]).verify(url), "URL is part of the explicit denylist");
    mock.verify();
}

#[test]
fn should_block_based_on_resolved_ip() {
    let mock = Arc::new(MockNet::default());
    let url = "https://smartbear.com";
    let expected_url = "https://1.2.3.4:3000/1";
    let expected_host = "smartbear.com";
    let expected_ip = "1.2.3.4";
    record_resolution(&mock, url, expected_host, expected_host, ip(expected_ip), expected_ip, expected_url);
    expect_denied(checker(&mock, &[], &["1.2.3.4"]).verify(url), "IP is part of the explicit denylist");
    mock.verify();
}

#[test]
fn should_block_url_matching_wildcard_pattern() {
    let mock = Arc::new(MockNet::default());
    let url = "https://foo.example.com";
    mock.expect("getHostFromUrl", &[url], s("foo.example.com"), Some(1));
    expect_denied(checker(&mock, &[], &["f*.example.com"]).verify(url), "URL is part of the explicit denylist");
    mock.verify();
}

#[test]
fn should_allow_url_matching_wildcard_pattern() {
    let mock = Arc::new(MockNet::default());
    let url = "https://foo.example.com";
    mock.expect("getHostFromUrl", &[url], s("foo.example.com"), Some(1));
    checker(&mock, &["f*.example.com"], &[]).verify(url).unwrap();
    mock.verify();
}
