//! `java.net.InetAddress` as JDK 25 implements it: literal parsing (`Inet4Address` decimal forms including the
//! legacy `a`, `a.b`, `a.b.c` shapes, the ambiguity check against BSD octal/hex forms, IPv6 literals with
//! IPv4-mapped addresses turned into `Inet4Address`), system name resolution with IPv4 addresses first
//! (`java.net.preferIPv6Addresses=false`), `getHostAddress()` text and the `is*Address()` predicates.

use crate::error::{JResult, JavaException};

/// `java.net.InetAddress`: an IPv4 or IPv6 address with the host name it was resolved from, if any.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum InetAddress {
    /// `Inet4Address`
    V4 {
        /// The four address bytes in network order.
        addr: [u8; 4],
        /// The host name it was created or resolved from, `None` for a bare literal.
        host_name: Option<String>,
    },
    /// `Inet6Address` (numeric scope id or interface name)
    V6 {
        /// The sixteen address bytes in network order.
        addr: [u8; 16],
        /// The `%scope` suffix: a numeric scope id or an interface name, `None` when absent.
        scope: Option<String>,
        /// The host name it was created or resolved from, `None` for a bare literal.
        host_name: Option<String>,
    },
}

fn unknown_host(msg: impl Into<String>) -> JavaException {
    JavaException::new("java.net.UnknownHostException", msg)
}

fn invalid_ipv6_literal(host: &str, brackets: bool) -> JavaException {
    let h = if brackets { format!("[{host}]") } else { host.to_string() };
    unknown_host(format!("{h}: invalid IPv6 address literal"))
}

/// `IPAddressUtil.parseAsciiDigit` / `parseAsciiHexDigit`
fn ascii_digit(c: char, radix: u32) -> Option<u32> {
    if radix == 16 {
        let l = c.to_ascii_lowercase();
        if ('a'..='f').contains(&l) {
            return Some(l as u32 - 'a' as u32 + 10);
        }
        return ascii_digit(c, 10);
    }
    let v = (c as i64) - ('0' as i64);
    if v < 0 || v >= radix as i64 { None } else { Some(v as u32) }
}

/// `IPAddressUtil.textToNumericFormatV4`
pub fn text_to_numeric_format_v4(src: &str) -> Option<[u8; 4]> {
    let chars: Vec<char> = src.chars().collect();
    let len = src.encode_utf16().count();
    if len == 0 || len > 15 {
        return None;
    }
    let mut res = [0u8; 4];
    let mut tmp: i64 = 0;
    let mut curr = 0usize;
    let mut new_octet = true;
    for &c in &chars {
        if c == '.' {
            if new_octet || !(0..=0xff).contains(&tmp) || curr == 3 {
                return None;
            }
            res[curr] = (tmp & 0xff) as u8;
            curr += 1;
            tmp = 0;
            new_octet = true;
        } else {
            let d = ascii_digit(c, 10)?;
            tmp = tmp * 10 + d as i64;
            new_octet = false;
        }
    }
    if new_octet || tmp < 0 || tmp >= (1i64 << ((4 - curr) * 8)) {
        return None;
    }
    let v = tmp as u32;
    if curr == 0 {
        res[0] = (v >> 24) as u8;
    }
    if curr <= 1 {
        res[1] = (v >> 16) as u8;
    }
    if curr <= 2 {
        res[2] = (v >> 8) as u8;
    }
    res[3] = v as u8;
    Some(res)
}

const CANT_PARSE: i64 = -1;
const TERMINAL: i64 = -2;

/// `IPAddressUtil.parseV4FieldBsd` over `chars[*pos..]`
fn parse_v4_field_bsd(radix: u32, chars: &[char], pos: &mut usize, field: usize) -> i64 {
    let initial = *pos;
    let mut val: i64 = 0;
    let mut digits = 0;
    let remaining = |p: usize| chars.len() - p;
    let prefix_ok = match radix {
        8 => {
            if remaining(*pos) < 2 {
                false
            } else {
                let (first, second) = (chars[*pos], chars[*pos + 1]);
                let ok = first == '0' && second != '.';
                if ok {
                    *pos += 1;
                }
                ok
            }
        }
        10 => remaining(*pos) > 0,
        _ => {
            if remaining(*pos) < 2 {
                false
            } else {
                let (first, second) = (chars[*pos], chars[*pos + 1]);
                *pos += 2;
                first == '0' && (second == 'x' || second == 'X')
            }
        }
    };
    if !prefix_ok {
        val = CANT_PARSE;
    }
    let mut dot_seen = false;
    while *pos < chars.len() && val != CANT_PARSE && !dot_seen {
        let c = chars[*pos];
        *pos += 1;
        if c == '.' {
            dot_seen = true;
            if field == 3 || digits == 0 || val > 255 {
                return TERMINAL;
            }
        } else if let Some(d) = ascii_digit(c, radix) {
            digits += 1;
            val = val.saturating_mul(radix as i64).saturating_add(d as i64);
        } else {
            return TERMINAL;
        }
    }
    if val == CANT_PARSE {
        *pos = initial;
    } else if !dot_seen {
        let max = (1i64 << ((4 - field) * 8)) - 1;
        if val > max {
            return TERMINAL;
        }
    }
    val
}

/// `IPAddressUtil.parseBsdLiteralV4`
pub fn parse_bsd_literal_v4(input: &str) -> Option<[u8; 4]> {
    let chars: Vec<char> = input.chars().collect();
    let mut res = [0u8; 4];
    let first = *chars.first()?;
    ascii_digit(first, 10)?;
    let last = *chars.last()?;
    if last == '.' || ascii_digit(last, 16).is_none() {
        return None;
    }
    let mut pos = 0;
    let mut field = 0usize;
    let mut value: i64 = -1;
    while pos < chars.len() {
        value = -1;
        for radix in [16, 8, 10] {
            value = parse_v4_field_bsd(radix, &chars, &mut pos, field);
            if value >= 0 {
                if value < 256 && field < 4 {
                    res[field] = value as u8;
                }
                field += 1;
                break;
            } else if value == TERMINAL {
                return None;
            }
        }
        if value < 0 {
            return None;
        }
    }
    if value < 0 {
        return None;
    }
    if field < 4 {
        let mut v = value;
        let mut i = 3i64;
        while i >= field as i64 - 1 {
            res[i as usize] = (v & 255) as u8;
            v >>= 8;
            i -= 1;
        }
    }
    Some(res)
}

/// `IPAddressUtil.textToNumericFormatV6`: 16 bytes, or 4 for an IPv4-mapped address.
pub fn text_to_numeric_format_v6(src: &str) -> Option<Vec<u8>> {
    let srcb: Vec<char> = src.chars().collect();
    if srcb.len() < 2 {
        return None;
    }
    let mut dst = [0u8; 16];
    let mut srcb_length = srcb.len();
    let pc = srcb.iter().position(|&c| c == '%');
    if pc == Some(srcb_length - 1) {
        return None;
    }
    if let Some(p) = pc {
        srcb_length = p;
    }
    let mut colonp: i64 = -1;
    let mut i = 0usize;
    let mut j = 0usize;
    if srcb[i] == ':' {
        i += 1;
        if srcb[i] != ':' {
            return None;
        }
    }
    let mut curtok = i;
    let mut saw_xdigit = false;
    let mut val: u32 = 0;
    while i < srcb_length {
        let ch = srcb[i];
        i += 1;
        if let Some(d) = ascii_digit(ch, 16) {
            val <<= 4;
            val |= d;
            if val > 0xffff {
                return None;
            }
            saw_xdigit = true;
            continue;
        }
        if ch == ':' {
            curtok = i;
            if !saw_xdigit {
                if colonp != -1 {
                    return None;
                }
                colonp = j as i64;
                continue;
            } else if i == srcb_length {
                return None;
            }
            if j + 2 > 16 {
                return None;
            }
            dst[j] = (val >> 8) as u8;
            dst[j + 1] = val as u8;
            j += 2;
            saw_xdigit = false;
            val = 0;
            continue;
        }
        if ch == '.' && j + 4 <= 16 {
            let ia4: String = srcb[curtok..srcb_length].iter().collect();
            if ia4.matches('.').count() != 3 {
                return None;
            }
            let v4 = text_to_numeric_format_v4(&ia4)?;
            dst[j..j + 4].copy_from_slice(&v4);
            j += 4;
            saw_xdigit = false;
            break;
        }
        return None;
    }
    if saw_xdigit {
        if j + 2 > 16 {
            return None;
        }
        dst[j] = (val >> 8) as u8;
        dst[j + 1] = val as u8;
        j += 2;
    }
    if colonp != -1 {
        let colonp = colonp as usize;
        let n = j - colonp;
        if j == 16 {
            return None;
        }
        for k in 1..=n {
            dst[16 - k] = dst[colonp + n - k];
            dst[colonp + n - k] = 0;
        }
        j = 16;
    }
    if j != 16 {
        return None;
    }
    if dst[..10].iter().all(|&b| b == 0) && dst[10] == 0xff && dst[11] == 0xff {
        return Some(dst[12..].to_vec());
    }
    Some(dst.to_vec())
}

/// `Inet6Address.parseAddressString(literal, false)`
fn parse_v6_literal(literal: &str) -> JResult<Option<InetAddress>> {
    if literal.is_empty() {
        return Ok(None);
    }
    let scope = literal.find('%').map(|p| literal[p + 1..].to_string());
    let Some(bytes) = text_to_numeric_format_v6(literal) else { return Ok(None) };
    if bytes.len() == 4 {
        if scope.is_some() {
            return Err(unknown_host(format!("{literal}: invalid IPv4-mapped address")));
        }
        return Ok(Some(InetAddress::V4 { addr: bytes.try_into().unwrap(), host_name: None }));
    }
    Ok(Some(InetAddress::V6 { addr: bytes.try_into().unwrap(), scope, host_name: None }))
}

impl InetAddress {
    /// `InetAddress.getByName(host)`
    pub fn get_by_name(host: &str) -> JResult<InetAddress> {
        Ok(InetAddress::get_all_by_name(host)?.remove(0))
    }

    /// `InetAddress.getAllByName(host)`
    pub fn get_all_by_name(host: &str) -> JResult<Vec<InetAddress>> {
        if host.is_empty() {
            return Ok(vec![InetAddress::loopback()]);
        }
        if host.contains('\0') {
            return Err(unknown_host("NUL character not allowed in hostname"));
        }
        let mut host = host;
        let mut ipv6_expected = false;
        if host.starts_with('[') {
            if host.chars().count() > 2 && host.ends_with(']') {
                host = &host[1..host.len() - 1];
                ipv6_expected = true;
            } else {
                return Err(invalid_ipv6_literal(host, false));
            }
        }
        let first = host.chars().next().unwrap_or('\0');
        if ascii_digit(first, 16).is_some() || first == ':' {
            let mut addr = None;
            if !ipv6_expected {
                match text_to_numeric_format_v4(host) {
                    Some(b) => addr = Some(InetAddress::V4 { addr: b, host_name: None }),
                    None => {
                        if parse_bsd_literal_v4(host).is_some() {
                            return Err(unknown_host(host.to_string()));
                        }
                    }
                }
            }
            if addr.is_none() {
                addr = parse_v6_literal(host)?;
                if addr.is_none() && (host.contains(':') || ipv6_expected) {
                    return Err(invalid_ipv6_literal(host, ipv6_expected));
                }
            }
            if let Some(a) = addr {
                return Ok(vec![a]);
            }
        } else if ipv6_expected {
            return Err(invalid_ipv6_literal(host, true));
        }
        resolve(host)
    }

    /// `InetAddress.getLoopbackAddress()` (IPv4 stack preferred)
    pub fn loopback() -> InetAddress {
        InetAddress::V4 { addr: [127, 0, 0, 1], host_name: Some("localhost".into()) }
    }

    /// `instanceof Inet4Address`.
    pub fn is_ipv4(&self) -> bool {
        matches!(self, InetAddress::V4 { .. })
    }

    /// `instanceof Inet6Address`.
    pub fn is_ipv6(&self) -> bool {
        matches!(self, InetAddress::V6 { .. })
    }

    /// `getAddress()`
    pub fn get_address(&self) -> Vec<u8> {
        match self {
            InetAddress::V4 { addr, .. } => addr.to_vec(),
            InetAddress::V6 { addr, .. } => addr.to_vec(),
        }
    }

    /// `getHostAddress()`: dotted quad, or eight lowercase hex groups without compression (`%scope` appended).
    pub fn get_host_address(&self) -> String {
        match self {
            InetAddress::V4 { addr, .. } => format!("{}.{}.{}.{}", addr[0], addr[1], addr[2], addr[3]),
            InetAddress::V6 { addr, scope, .. } => {
                let groups: Vec<String> = (0..8).map(|i| format!("{:x}", ((addr[2 * i] as u16) << 8) | addr[2 * i + 1] as u16)).collect();
                let mut s = groups.join(":");
                if let Some(sc) = scope {
                    s.push('%');
                    s.push_str(sc);
                }
                s
            }
        }
    }

    /// `isAnyLocalAddress()`: `true` for the wildcard address (all bytes zero).
    pub fn is_any_local_address(&self) -> bool {
        self.get_address().iter().all(|&b| b == 0)
    }

    /// `isLoopbackAddress()`: `127.0.0.0/8` for IPv4, `::1` for IPv6.
    pub fn is_loopback_address(&self) -> bool {
        match self {
            InetAddress::V4 { addr, .. } => addr[0] == 127,
            InetAddress::V6 { addr, .. } => addr[..15].iter().all(|&b| b == 0) && addr[15] == 1,
        }
    }

    /// `isLinkLocalAddress()`: `169.254.0.0/16` for IPv4, `fe80::/10` for IPv6.
    pub fn is_link_local_address(&self) -> bool {
        match self {
            InetAddress::V4 { addr, .. } => addr[0] == 169 && addr[1] == 254,
            InetAddress::V6 { addr, .. } => addr[0] == 0xfe && (addr[1] & 0xc0) == 0x80,
        }
    }

    /// `isSiteLocalAddress()`: the RFC 1918 ranges `10/8`, `172.16/12`, `192.168/16` for IPv4, `fec0::/10` for IPv6.
    pub fn is_site_local_address(&self) -> bool {
        match self {
            InetAddress::V4 { addr, .. } => addr[0] == 10 || (addr[0] == 172 && (addr[1] & 0xf0) == 16) || (addr[0] == 192 && addr[1] == 168),
            InetAddress::V6 { addr, .. } => addr[0] == 0xfe && (addr[1] & 0xc0) == 0xc0,
        }
    }

    /// `toString()`: `hostName/hostAddress`
    pub fn to_java_string(&self) -> String {
        let h = match self {
            InetAddress::V4 { host_name, .. } | InetAddress::V6 { host_name, .. } => host_name.clone().unwrap_or_default(),
        };
        format!("{h}/{}", self.get_host_address())
    }
}

/// The name service call itself (`getaddrinfo`) has no timeout of its own, unlike a socket read or connect: a
/// resolver that is configured but unreachable (rather than promptly refusing) can block the calling thread far
/// longer than any bounded step downstream of it (`RemoteUrl`'s own connect/read timeouts). Bound it on a
/// detached thread so an unresponsive resolver still surfaces as a (deterministic) lookup failure instead of an
/// unbounded hang; a real resolver answers in milliseconds, so this is not reachable in ordinary use.
const RESOLVE_TIMEOUT_MS: u64 = 5_000;

/// Name service lookup (IPv4 addresses first, as with `preferIPv6Addresses=false`).
fn resolve(host: &str) -> JResult<Vec<InetAddress>> {
    use std::net::IpAddr;
    let addrs: Vec<IpAddr> = match resolve_bounded(host) {
        Ok(it) => it,
        Err(e) => return Err(unknown_host(format!("{host}: {}", lookup_message(&e)))),
    };
    if addrs.is_empty() {
        return Err(unknown_host(format!("{host}: nodename nor servname provided, or not known")));
    }
    let mut out: Vec<InetAddress> = Vec::new();
    for want_v4 in [true, false] {
        for a in &addrs {
            let ia = match a {
                IpAddr::V4(v4) if want_v4 => InetAddress::V4 { addr: v4.octets(), host_name: Some(host.to_string()) },
                IpAddr::V6(v6) if !want_v4 => InetAddress::V6 { addr: v6.octets(), scope: None, host_name: Some(host.to_string()) },
                _ => continue,
            };
            if !out.contains(&ia) {
                out.push(ia);
            }
        }
    }
    Ok(out)
}

/// `(host, 0).to_socket_addrs()` on a detached thread, bounded by [`RESOLVE_TIMEOUT_MS`]. The lookup thread is
/// not cancelled on timeout (there is no portable way to abort a blocked `getaddrinfo`); it keeps running
/// detached and its result, if any, is simply dropped when the channel's other end is already gone.
fn resolve_bounded(host: &str) -> std::io::Result<Vec<std::net::IpAddr>> {
    use std::net::ToSocketAddrs;
    use std::sync::mpsc;
    let (tx, rx) = mpsc::channel();
    let owned = host.to_string();
    std::thread::spawn(move || {
        let result = (owned.as_str(), 0u16).to_socket_addrs().map(|it| it.map(|a| a.ip()).collect::<Vec<_>>());
        let _ = tx.send(result);
    });
    match rx.recv_timeout(std::time::Duration::from_millis(RESOLVE_TIMEOUT_MS)) {
        Ok(result) => result,
        Err(_) => Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "name lookup timed out")),
    }
}

fn lookup_message(e: &std::io::Error) -> String {
    let s = e.to_string();
    // std prefixes "failed to lookup address information: "
    s.strip_prefix("failed to lookup address information: ").unwrap_or(&s).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lit(s: &str) -> String {
        InetAddress::get_by_name(s).unwrap().get_host_address()
    }

    #[test]
    fn literals() {
        assert_eq!(lit("3232235778"), "192.168.1.2");
        assert_eq!(lit("127.3"), "127.0.0.3");
        assert_eq!(lit("010.1.1.1"), "10.1.1.1");
        assert_eq!(lit("::1"), "0:0:0:0:0:0:0:1");
        assert_eq!(lit("[fd00:ec2::254]"), "fd00:ec2:0:0:0:0:0:254");
        assert_eq!(lit("0:0:0:0:0:ffff:a01:219"), "10.1.2.25");
        assert_eq!(lit("0:0:0:0:0:ffff:1.2.3.4"), "1.2.3.4");
        assert!(InetAddress::get_by_name("0x7f.1").is_err());
        assert_eq!(InetAddress::get_by_name("[1.2.3.4]").unwrap_err().to_java_string(), "java.net.UnknownHostException: [1.2.3.4]: invalid IPv6 address literal");
        assert_eq!(InetAddress::get_by_name("0x7f.1").unwrap_err().to_java_string(), "java.net.UnknownHostException: 0x7f.1");
        assert_eq!(InetAddress::get_by_name("::ffff:1.2.3.4%1").unwrap_err().to_java_string(), "java.net.UnknownHostException: ::ffff:1.2.3.4%1: invalid IPv4-mapped address");
        assert_eq!(InetAddress::get_by_name("1:2:3:4:5:6:7:8:9").unwrap_err().to_java_string(), "java.net.UnknownHostException: 1:2:3:4:5:6:7:8:9: invalid IPv6 address literal");
        assert_eq!(lit("fe80::1%lo0"), "fe80:0:0:0:0:0:0:1%lo0");
        assert_eq!(lit("12345678901"), "223.220.28.53");
        assert_eq!(lit("0"), "0.0.0.0");
        assert_eq!(lit("::"), "0:0:0:0:0:0:0:0");
        assert_eq!(lit("1.2.3"), "1.2.0.3");
    }
}
