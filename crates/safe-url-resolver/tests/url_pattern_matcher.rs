//! Port of `UrlPatternMatcherTest`.

use swagger_parser_safe_url_resolver::UrlPatternMatcher;

fn m(p: &str) -> UrlPatternMatcher {
    UrlPatternMatcher::new(&[p]).unwrap()
}

fn matches(m: &UrlPatternMatcher, url: &str) -> bool {
    m.matches(url).unwrap()
}

#[test]
fn returns_false_when_url_cannot_be_parsed() {
    let matcher = UrlPatternMatcher::new::<&str>(&[]).unwrap();
    assert!(!matches(&matcher, "not a url"));
}

#[test]
fn returns_false_when_url_is_not_http_or_https() {
    let matcher = UrlPatternMatcher::new::<&str>(&[]).unwrap();
    assert!(!matches(&matcher, "file://not a url"));
}

#[test]
fn domain_without_port_matches_any_port() {
    let matcher = m("example.com");
    assert!(matches(&matcher, "http://example.com"));
    assert!(matches(&matcher, "https://example.com"));
    assert!(matches(&matcher, "http://example.com:12345"));
    assert!(matches(&matcher, "https://example.com:12345"));
    assert!(!matches(&matcher, "https://not.example.com:12345"));
}

#[test]
fn domain_with_port_matches_only_that_port() {
    let matcher = m("example.com:443");
    assert!(!matches(&matcher, "http://example.com"));
    assert!(matches(&matcher, "https://example.com"));
    assert!(matches(&matcher, "http://example.com:443"));
    assert!(!matches(&matcher, "http://example.com:12345"));
    assert!(!matches(&matcher, "https://example.com:1234"));
    assert!(!matches(&matcher, "https://not.example.com:12345"));
}

#[test]
fn domain_supports_wildcards() {
    let matcher = m("*.example.com");
    assert!(!matches(&matcher, "http://example.com"));
    assert!(!matches(&matcher, "https://example.com"));
    assert!(!matches(&matcher, "https://fooexample.com"));
    assert!(matches(&matcher, "https://foo.example.com"));
    assert!(matches(&matcher, "https://foo.bar.example.com"));
}

#[test]
fn domain_in_url_is_case_insensitive() {
    let matcher = m("*.example.com");
    assert!(!matches(&matcher, "http://ExAmPlE.CoM"));
    assert!(matches(&matcher, "https://FoO.ExAmPlE.CoM"));
}

#[test]
fn domain_in_pattern_is_case_insensitive() {
    let matcher = m("*.EXamPLe.Com");
    assert!(!matches(&matcher, "http://ExAmPlE.CoM"));
    assert!(matches(&matcher, "https://FoO.ExAmPlE.CoM"));
}

#[test]
fn support_for_matching_internationalized_domain_names() {
    let matcher = m("*.😋.local");
    assert!(!matches(&matcher, "http://example.com"));
    assert!(matches(&matcher, "http://blah.😋.local"));
    assert!(matches(&matcher, "http://blah.xn--p28h.local"));
}

#[test]
fn domains_do_not_support_wildcards_at_the_end() {
    let matcher = m("example.co*");
    assert!(!matches(&matcher, "https://example.net"));
    // the upstream test keeps its other two assertions commented out
}

#[test]
fn ip_addresses_support_wildcards_at_the_end() {
    let matcher = m("10.100.*.*");
    assert!(matches(&matcher, "http://10.100.1.2"));
    assert!(!matches(&matcher, "http://10.101.1.2"));
}

#[test]
fn works_with_urls_with_auth_path_and_query_components() {
    let matcher = m("*.example.com");
    assert!(!matches(&matcher, "https://foo:bar@example.com/path?q=1"));
    assert!(matches(&matcher, "https://foo:bar@foo.example.com/path?q=1"));
}

#[test]
fn supports_ip_addresses_in_patterns() {
    let matcher = m("1.*.3.4");
    assert!(matches(&matcher, "https://foo:bar@1.2.3.4/path?q=1"));
    assert!(!matches(&matcher, "https://foo:bar@1.2.3.5/path?q=1"));
}
