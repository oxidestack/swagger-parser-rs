//! Port of `NetUtilsTest`.

use swagger_parser_safe_url_resolver::java_rt::inet::InetAddress;
use swagger_parser_safe_url_resolver::system as net;

fn ip(s: &str) -> InetAddress {
    InetAddress::get_by_name(s).unwrap()
}

#[test]
fn get_host_from_url_with_domain_name_should_return_hostname() {
    assert_eq!(net::get_host_from_url("https://example.com/hello?query=world").unwrap(), "example.com");
}

#[test]
fn get_host_from_url_with_ipv4_address_should_return_ip_address() {
    assert_eq!(net::get_host_from_url("https://1.2.3.4/hello?query=world").unwrap(), "1.2.3.4");
}

#[test]
fn get_host_from_url_with_ipv6_address_should_return_ip_address() {
    assert_eq!(net::get_host_from_url("https://[::1]/hello?query=world").unwrap(), "::1");
}

#[test]
fn set_host_should_set_ipv4_address_in_url() {
    assert_eq!(net::set_host("https://example.com/hello?query=world", "1.2.3.4").unwrap(), "https://1.2.3.4/hello?query=world");
}

#[test]
fn set_host_should_set_ipv6_address_in_url_with_brackets() {
    assert_eq!(net::set_host("https://example.com/hello?query=world", "::1").unwrap(), "https://[::1]/hello?query=world");
}

#[test]
fn is_ipv4_with_ipv4_address_should_return_true() {
    assert!(net::is_ipv4("1.2.3.4"));
}

#[test]
fn is_ipv4_with_ipv6_address_should_return_false() {
    assert!(!net::is_ipv4("::1"));
}

#[test]
fn is_ipv6_with_ipv6_address_should_return_true() {
    assert!(net::is_ipv6("::1"));
}

#[test]
fn is_ipv6_with_ipv4_address_should_return_false() {
    assert!(!net::is_ipv6("1.2.3.4"));
}

#[test]
fn is_ipv6_with_improper_address_should_return_false() {
    assert!(!net::is_ipv6("999.999.999.999"));
}

#[test]
fn is_unique_local_address_with_ula_should_return_true() {
    assert!(net::is_unique_local_address(&ip("fc00::1")));
    assert!(net::is_unique_local_address(&ip("fd00:ec2::254")));
}

#[test]
fn is_unique_local_address_with_non_ula_ipv6_address_should_return_false() {
    assert!(!net::is_unique_local_address(&ip("::1")));
}

#[test]
fn is_unique_local_address_with_ipv4_address_should_return_false() {
    assert!(!net::is_unique_local_address(&ip("1.2.3.4")));
}

#[test]
fn is_nat64_with_nat64_address_should_return_true() {
    assert!(net::is_nat64_address(&ip("64:ff9b::")));
}

#[test]
fn is_nat64_with_regular_ipv6_address_should_return_false() {
    assert!(!net::is_nat64_address(&ip("fc00::1")));
}

#[test]
fn is_nat64_with_ipv4_address_should_return_false() {
    assert!(!net::is_nat64_address(&ip("1.2.3.4")));
}
