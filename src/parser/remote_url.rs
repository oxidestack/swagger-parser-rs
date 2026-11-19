//! `io.swagger.v3.parser.util.RemoteUrl`: fetching remote documents over `HttpURLConnection` semantics
//! (same-protocol redirects followed internally, cross-protocol redirects by the caller loop, auth values as
//! headers or query parameters, ISO control characters other than `\n` dropped from the body).

use crate::core::AuthorizationValue;
use crate::java::net::JavaUrl;
use crate::java::{JResult, JavaException};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use swagger_parser_safe_url_resolver::PermittedUrlsChecker;

const ACCEPT_HEADER_VALUE: &str = "application/json, application/yaml, */*";
const USER_AGENT_HEADER_VALUE: &str = "Apache-HttpClient/Swagger";
const CONNECTION_TIMEOUT_MS: u64 = 30_000;
const READ_TIMEOUT_MS: u64 = 60_000;
const MAX_REDIRECTS: usize = 5;
/// `http.maxRedirects` default of `HttpURLConnection`
const JDK_MAX_REDIRECTS: usize = 20;

/// `cleanUrl`
pub fn clean_url(url: &str) -> String {
    url.replace('{', "%7B").replace('}', "%7D").replace(' ', "%20")
}

fn io_err(msg: impl Into<String>) -> JavaException {
    JavaException::new("java.io.IOException", msg)
}

struct Response {
    code: u16,
    location: Option<String>,
    body: Vec<u8>,
}

static TRUST_ALL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// `System.setProperty("io.swagger.v3.parser.util.RemoteUrl.trustAll", ...)`: accept any server certificate
/// and host name, and let `urlToString` handle every https redirect itself.
pub fn set_trust_all(v: bool) {
    TRUST_ALL.store(v, std::sync::atomic::Ordering::SeqCst);
}

/// Whether `RemoteUrl.trustAll` is on (see [`set_trust_all`]).
pub fn trust_all() -> bool {
    TRUST_ALL.load(std::sync::atomic::Ordering::SeqCst)
}

#[derive(Debug)]
struct AcceptAll(Arc<rustls::crypto::CryptoProvider>);

impl rustls::client::danger::ServerCertVerifier for AcceptAll {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.0.signature_verification_algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.0.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

fn trust_all_config() -> Arc<rustls::ClientConfig> {
    static CFG: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();
    CFG.get_or_init(|| {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let cfg = rustls::ClientConfig::builder_with_provider(provider.clone())
            .with_safe_default_protocol_versions()
            .expect("tls versions")
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(AcceptAll(provider)))
            .with_no_client_auth();
        Arc::new(cfg)
    })
    .clone()
}

fn tls_config() -> Arc<rustls::ClientConfig> {
    if trust_all() {
        return trust_all_config();
    }
    static CFG: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();
    CFG.get_or_init(|| {
        let mut roots = rustls::RootCertStore::empty();
        for cert in rustls_native_certs::load_native_certs().certs {
            let _ = roots.add(cert);
        }
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let cfg = rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .expect("tls versions")
            .with_root_certificates(roots)
            .with_no_client_auth();
        Arc::new(cfg)
    })
    .clone()
}

/// One HTTP GET (no redirect handling).
fn http_get(url: &JavaUrl, headers: &[(String, String)]) -> JResult<Response> {
    // `HttpURLConnection.plainConnect` asks the default `ProxySelector` where to connect before it connects.
    if let Some(select) = crate::parser::hooks::current().select_proxy {
        select(&url.to_external_form(), url.get_host())?;
    }
    let host = url.get_host().trim_start_matches('[').trim_end_matches(']').to_string();
    let port = if url.get_port() == -1 { url.get_default_port() } else { url.get_port() };
    if port <= 0 {
        return Err(io_err(format!("unknown protocol: {}", url.get_protocol())));
    }
    // `new InetSocketAddress(host, port)`: the first address of `InetAddress.getByName(host)`
    let sock = {
        let ip = java_rt::inet::InetAddress::get_by_name(&host)?;
        let ip: std::net::IpAddr = match &ip {
            java_rt::inet::InetAddress::V4 { addr, .. } => std::net::IpAddr::from(*addr),
            java_rt::inet::InetAddress::V6 { addr, .. } => std::net::IpAddr::from(*addr),
        };
        let a = std::net::SocketAddr::new(ip, port as u16);
        match TcpStream::connect_timeout(&a, Duration::from_millis(CONNECTION_TIMEOUT_MS)) {
            Ok(x) => x,
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => return Err(JavaException::new("java.net.SocketTimeoutException", "Connect timed out")),
            Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused || e.raw_os_error().is_none() => {
                return Err(JavaException::new("java.net.ConnectException", "Connection refused"));
            }
            Err(e) => return Err(JavaException::new("java.net.ConnectException", strip_os_error(&e.to_string()))),
        }
    };
    sock.set_read_timeout(Some(Duration::from_millis(READ_TIMEOUT_MS))).ok();
    let path = if url.get_file().is_empty() { "/".to_string() } else { url.get_file().to_string() };
    let host_header = if url.get_port() == -1 { host.clone() } else { format!("{}:{}", url.get_host(), url.get_port()) };
    let mut req = format!("GET {path} HTTP/1.1\r\n");
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str(&format!("Host: {host_header}\r\nConnection: close\r\n\r\n"));
    let raw: Vec<u8> = if url.get_protocol() == "https" {
        let server_name = rustls::pki_types::ServerName::try_from(host.clone()).map_err(|_| io_err(format!("invalid host {host}")))?;
        let conn = rustls::ClientConnection::new(tls_config(), server_name).map_err(|e| io_err(e.to_string()))?;
        let mut tls = rustls::StreamOwned::new(conn, sock);
        tls.write_all(req.as_bytes()).map_err(|e| JavaException::new("javax.net.ssl.SSLHandshakeException", e.to_string()))?;
        let mut buf = Vec::new();
        match tls.read_to_end(&mut buf) {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof && !buf.is_empty() => {}
            Err(e) => return Err(io_err(e.to_string())),
        }
        buf
    } else {
        let mut s = sock;
        s.write_all(req.as_bytes()).map_err(|e| io_err(e.to_string()))?;
        let mut buf = Vec::new();
        s.read_to_end(&mut buf).map_err(|e| io_err(e.to_string()))?;
        buf
    };
    parse_response(&raw)
}

fn parse_response(raw: &[u8]) -> JResult<Response> {
    let mut reader = BufReader::new(raw);
    let mut status = String::new();
    reader.read_line(&mut status).map_err(|e| io_err(e.to_string()))?;
    let code: u16 = status.split_whitespace().nth(1).and_then(|c| c.parse().ok()).ok_or_else(|| io_err("Invalid Http response"))?;
    let mut location = None;
    let mut content_length: Option<usize> = None;
    let mut chunked = false;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).map_err(|e| io_err(e.to_string()))? == 0 {
            break;
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            let v = v.trim();
            match k.trim().to_ascii_lowercase().as_str() {
                "location" => location = Some(v.to_string()),
                "content-length" => content_length = v.parse().ok(),
                "transfer-encoding" if v.eq_ignore_ascii_case("chunked") => chunked = true,
                _ => {}
            }
        }
    }
    let mut body = Vec::new();
    if chunked {
        loop {
            let mut size_line = String::new();
            if reader.read_line(&mut size_line).map_err(|e| io_err(e.to_string()))? == 0 {
                break;
            }
            let size = usize::from_str_radix(size_line.trim().split(';').next().unwrap_or("0"), 16).unwrap_or(0);
            if size == 0 {
                break;
            }
            let mut chunk = vec![0u8; size];
            reader.read_exact(&mut chunk).map_err(|e| io_err(e.to_string()))?;
            body.extend_from_slice(&chunk);
            let mut crlf = String::new();
            let _ = reader.read_line(&mut crlf);
        }
    } else {
        let mut rest = Vec::new();
        reader.read_to_end(&mut rest).map_err(|e| io_err(e.to_string()))?;
        if let Some(n) = content_length {
            rest.truncate(n);
        }
        body = rest;
    }
    Ok(Response { code, location, body })
}

/// std's io::Error text without the " (os error N)" suffix
fn strip_os_error(s: &str) -> String {
    match s.rfind(" (os error ") {
        Some(i) => s[..i].to_string(),
        None => s.to_string(),
    }
}

fn is_redirect(code: u16) -> bool {
    matches!(code, 301 | 302 | 303 | 307 | 308)
}

/// `URLEncoder.encode(s, UTF-8)`
pub fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        let c = b as char;
        if c.is_ascii_alphanumeric() || ".-*_".contains(c) {
            out.push(c);
        } else if c == ' ' {
            out.push('+');
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// `urlToString(url, auths, permittedUrlsChecker)`
pub fn url_to_string(url: &str, auths: &[AuthorizationValue], checker: Option<&PermittedUrlsChecker>) -> JResult<String> {
    if let Some(h) = crate::parser::hooks::current().url_to_string && let Some(r) = h(url, auths) {
        return r;
    }
    let body = url_to_bytes(url, auths, checker)?;
    let text = String::from_utf8_lossy(&body);
    Ok(text.chars().filter(|c| !is_iso_control(*c) || *c == '\n').collect())
}

/// The body [`url_to_string`] reads, as the bytes `URL.openStream` gives, before its decoding and control-character
/// filter.
pub fn url_to_bytes(url: &str, auths: &[AuthorizationValue], checker: Option<&PermittedUrlsChecker>) -> JResult<Vec<u8>> {
    let mut url = url.to_string();
    let mut redirect_count = 0;
    while redirect_count <= MAX_REDIRECTS {
        if redirect_count > 0 && let Some(c) = checker {
            c.verify(&url)?;
        }
        let in_url = JavaUrl::parse(&clean_url(&url))?;
        let mut query: Vec<&AuthorizationValue> = Vec::new();
        let mut header: Vec<&AuthorizationValue> = Vec::new();
        for auth in auths {
            if (auth.url_matcher)(&in_url) {
                match auth.type_.as_deref() {
                    Some("query") => query.push(auth),
                    Some("header") => header.push(auth),
                    _ => {}
                }
            }
        }
        let mut target = in_url.clone();
        if !query.is_empty() {
            let uri = in_url.to_uri()?;
            let mut new_query = uri.query.clone().unwrap_or_default();
            for item in &query {
                if !new_query.is_empty() {
                    new_query.push('&');
                }
                new_query.push_str(&url_encode(item.key_name.as_deref().unwrap_or("null")));
                new_query.push('=');
                new_query.push_str(&url_encode(item.value.as_deref().unwrap_or("null")));
            }
            let mut rebuilt = format!("{}://{}{}", uri.scheme.clone().unwrap_or_default(), uri.authority.clone().unwrap_or_default(), uri.path.clone().unwrap_or_default());
            rebuilt.push('?');
            rebuilt.push_str(&new_query);
            if let Some(f) = &uri.fragment {
                rebuilt.push('#');
                rebuilt.push_str(f);
            }
            target = JavaUrl::parse(&rebuilt)?;
        }
        let mut headers: Vec<(String, String)> = header.iter().map(|a| (a.key_name.clone().unwrap_or_default(), a.value.clone().unwrap_or_default())).collect();
        headers.push(("Accept".into(), ACCEPT_HEADER_VALUE.into()));
        headers.push(("User-Agent".into(), USER_AGENT_HEADER_VALUE.into()));
        // HttpURLConnection follows same-protocol redirects on its own
        let mut current = target;
        let mut resp = http_get(&current, &headers)?;
        let mut internal = 0;
        let follow_internally = !(trust_all() && current.get_protocol() == "https");
        while follow_internally && is_redirect(resp.code) && internal < JDK_MAX_REDIRECTS {
            let Some(loc) = resp.location.clone() else { break };
            let next = JavaUrl::parse_with_context(Some(&current), &loc)?;
            if next.get_protocol() != current.get_protocol() {
                break;
            }
            current = next;
            resp = http_get(&current, &headers)?;
            internal += 1;
        }
        if is_redirect(resp.code) {
            match resp.location {
                Some(l) => url = l,
                None => return Err(io_err("Redirect response missing 'Location' header")),
            }
            redirect_count += 1;
            continue;
        }
        if resp.code >= 400 {
            let u = current.to_external_form();
            return Err(if resp.code == 404 || resp.code == 410 {
                JavaException::new("java.io.FileNotFoundException", u)
            } else {
                io_err(format!("Server returned HTTP response code: {} for URL: {u}", resp.code))
            });
        }
        return Ok(resp.body);
    }
    Err(io_err(format!("Too many redirects (> {MAX_REDIRECTS})")))
}

fn is_iso_control(c: char) -> bool {
    (c as u32) <= 0x1f || (0x7f..=0x9f).contains(&(c as u32))
}
