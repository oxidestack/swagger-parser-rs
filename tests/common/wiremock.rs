//! A small WireMock stand-in for the ported tests: stubs matched on method + URL (`urlEqualTo`,
//! `urlPathEqualTo`, `urlPathMatching`, `urlMatching`) and optional request headers, plain HTTP or HTTPS
//! (self-signed), with request logging for `verify(...)`.

use parking_lot::Mutex;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;

#[derive(Clone, Debug)]
pub enum UrlPattern {
    /// `urlEqualTo(url)`: path + query
    Equal(String),
    /// `urlPathEqualTo(path)`
    PathEqual(String),
    /// `urlPathMatching(regex)`
    PathMatching(String),
    /// `urlMatching(regex)`: path + query
    Matching(String),
}

impl UrlPattern {
    fn matches(&self, url: &str) -> bool {
        let path = url.split('?').next().unwrap_or("");
        let whole = |re: &str, s: &str| regex::Regex::new(&format!("^(?:{re})$")).map(|r| r.is_match(s)).unwrap_or(false);
        match self {
            UrlPattern::Equal(u) => u == url,
            UrlPattern::PathEqual(p) => p == path,
            UrlPattern::PathMatching(re) => whole(re, path),
            UrlPattern::Matching(re) => whole(re, url),
        }
    }
}

pub fn url_equal_to(s: &str) -> UrlPattern {
    UrlPattern::Equal(s.into())
}
pub fn url_path_equal_to(s: &str) -> UrlPattern {
    UrlPattern::PathEqual(s.into())
}
pub fn url_path_matching(s: &str) -> UrlPattern {
    UrlPattern::PathMatching(s.into())
}
pub fn url_matching(s: &str) -> UrlPattern {
    UrlPattern::Matching(s.into())
}

#[derive(Clone, Debug)]
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// `aResponse()`
pub fn a_response() -> Response {
    Response { status: 200, headers: Vec::new(), body: Vec::new() }
}

impl Response {
    pub fn with_status(mut self, s: u16) -> Self {
        self.status = s;
        self
    }
    pub fn with_header(mut self, k: &str, v: &str) -> Self {
        self.headers.push((k.into(), v.into()));
        self
    }
    pub fn with_body(mut self, b: impl Into<Vec<u8>>) -> Self {
        self.body = b.into();
        self
    }
}

#[derive(Clone, Debug)]
pub struct Stub {
    pub method: String,
    pub url: UrlPattern,
    pub headers: Vec<(String, String)>,
    pub response: Response,
}

/// `get(urlPattern)`
pub fn get(url: UrlPattern) -> Stub {
    Stub { method: "GET".into(), url, headers: Vec::new(), response: a_response() }
}

impl Stub {
    /// `.withHeader(name, equalTo(value))`
    pub fn with_header(mut self, k: &str, v: &str) -> Self {
        self.headers.push((k.into(), v.into()));
        self
    }
    /// `.willReturn(response)`
    pub fn will_return(mut self, r: Response) -> Self {
        self.response = r;
        self
    }
}

#[derive(Clone, Debug)]
pub struct LoggedRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
}

impl LoggedRequest {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
    pub fn contains_header(&self, name: &str) -> bool {
        self.header(name).is_some()
    }
}

#[derive(Default)]
struct State {
    stubs: Vec<Stub>,
    requests: Vec<LoggedRequest>,
}

pub struct WireMockServer {
    port: u16,
    https: bool,
    state: Arc<Mutex<State>>,
    stop: Arc<std::sync::atomic::AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Drop for WireMockServer {
    /// `stop()`
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        let _ = TcpStream::connect(("127.0.0.1", self.port));
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn tls_server_config() -> Arc<rustls::ServerConfig> {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string(), "127.0.0.1".to_string()]).unwrap();
    let cert_der = rustls::pki_types::CertificateDer::from(cert.cert.der().to_vec());
    let key = rustls::pki_types::PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der()));
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let cfg = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key)
        .unwrap();
    Arc::new(cfg)
}

impl WireMockServer {
    /// A server on a free port (`dynamicPort()`); `port` fixes it (0 = any).
    pub fn start(port: u16, https: bool) -> WireMockServer {
        let listener = TcpListener::bind(("127.0.0.1", port)).expect("bind");
        let port = listener.local_addr().unwrap().port();
        let state: Arc<Mutex<State>> = Arc::new(Mutex::new(State::default()));
        let st = state.clone();
        let tls = if https { Some(tls_server_config()) } else { None };
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop2 = stop.clone();
        let handle = thread::spawn(move || {
            for conn in listener.incoming() {
                if stop2.load(std::sync::atomic::Ordering::SeqCst) {
                    break;
                }
                let Ok(conn) = conn else { continue };
                let st = st.clone();
                let tls = tls.clone();
                thread::spawn(move || {
                    let _ = match tls {
                        Some(cfg) => match rustls::ServerConnection::new(cfg) {
                            Ok(c) => {
                                let mut s = rustls::StreamOwned::new(c, conn);
                                serve(&mut s, &st)
                            }
                            Err(_) => Ok(()),
                        },
                        None => {
                            let mut c = conn;
                            serve(&mut c, &st)
                        }
                    };
                });
            }
        });
        WireMockServer { port, https, state, stop, handle: Some(handle) }
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// `baseUrl()`
    pub fn base_url(&self) -> String {
        format!("{}://localhost:{}", if self.https { "https" } else { "http" }, self.port)
    }

    /// `stubFor(stub)`
    pub fn stub_for(&self, s: Stub) {
        self.state.lock().stubs.push(s);
    }

    /// `findAll(getRequestedFor(url))`
    pub fn find_all(&self, url: &UrlPattern) -> Vec<LoggedRequest> {
        self.state.lock().requests.iter().filter(|r| url.matches(&r.url)).cloned().collect()
    }

    pub fn reset(&self) {
        let mut s = self.state.lock();
        s.stubs.clear();
        s.requests.clear();
    }
}

fn serve<S: Read + Write>(s: &mut S, state: &Arc<Mutex<State>>) -> std::io::Result<()> {
    let mut reader = BufReader::new(&mut *s);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let url = parts.next().unwrap_or("").to_string();
    let mut headers = Vec::new();
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h)? == 0 {
            break;
        }
        let h = h.trim_end_matches(['\r', '\n']);
        if h.is_empty() {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            headers.push((k.trim().to_string(), v.trim().to_string()));
        }
    }
    drop(reader);
    let req = LoggedRequest { method: method.clone(), url: url.clone(), headers };
    let resp = {
        let mut st = state.lock();
        st.requests.push(req.clone());
        st.stubs
            .iter()
            .rev()
            .find(|stub| {
                stub.method == method && stub.url.matches(&url) && stub.headers.iter().all(|(k, v)| req.header(k) == Some(v.as_str()))
            })
            .map(|stub| stub.response.clone())
    };
    let resp = resp.unwrap_or(Response { status: 404, headers: vec![], body: b"No response could be served".to_vec() });
    let reason = match resp.status {
        200 => "OK",
        301 => "Moved Permanently",
        302 => "Found",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Status",
    };
    let mut out = format!("HTTP/1.1 {} {reason}\r\n", resp.status);
    for (k, v) in &resp.headers {
        out.push_str(&format!("{k}: {v}\r\n"));
    }
    out.push_str(&format!("Content-Length: {}\r\nConnection: close\r\n\r\n", resp.body.len()));
    s.write_all(out.as_bytes())?;
    s.write_all(&resp.body)?;
    s.flush()
}

#[allow(dead_code)]
fn _unused(_: TcpStream) {}

/// The `@BeforeClass` setup many upstream tests share: for each (file, url path regex, content type), read the
/// file, replace `${dynamicPort}` with the server's port and stub `get(urlPathMatching(url))` with status 200,
/// that `Content-type` and the body.
pub fn stub_files(server: &WireMockServer, stubs: &[(&str, &str, &str)]) {
    for (file, url, ct) in stubs {
        let body = super::read_file(file).replace("${dynamicPort}", &server.port().to_string());
        server.stub_for(get(url_path_matching(url)).will_return(a_response().with_status(200).with_header("Content-type", ct).with_body(body)));
    }
}
