//! Shared support for the ported upstream tests.
#![allow(dead_code)]

pub mod mock_cache;
pub mod processor_mocks;
pub mod wiremock;

use parking_lot::{Mutex, MutexGuard};
use std::sync::Once;

pub const V3_MODULE: &str = "upstream/modules/swagger-parser-v3";
pub const CONVERTER_MODULE: &str = "upstream/modules/swagger-parser-v2-converter";
pub const PARSER_MODULE: &str = "upstream/modules/swagger-parser";
pub const CLI_MODULE: &str = "upstream/modules/swagger-parser-cli";

static INIT: Once = Once::new();
static SERIAL: Mutex<()> = Mutex::new(());

/// Per-test setup, mirroring how Maven runs the module's tests: the working directory is the module
/// directory, `src/test/resources` (and `src/main/resources`) are on the class path, the tests'
/// `META-INF/services` registrations are active, and tests run one at a time (TestNG runs them
/// sequentially; some mutate process-wide options). Hold the returned guard for the whole test.
pub fn setup(module: &str) -> MutexGuard<'static, ()> {
    let guard = SERIAL.lock();
    INIT.call_once(|| {
        let root = env!("CARGO_MANIFEST_DIR");
        std::env::set_current_dir(format!("{root}/{module}")).expect("module directory");
        swagger_parser::java::classpath::set_classpath_roots(vec!["src/test/resources".into(), "src/main/resources".into()]);
    });
    if module == V3_MODULE {
        register_v3_test_services();
    }
    guard
}

/// io.swagger.v3.parser.test.CustomOpenAPIDereferencer (registered in the v3 test resources'
/// `META-INF/services`). The registration is per thread: threads a test spawns call this themselves.
pub fn register_v3_test_services() {
    swagger_parser::parser::reference::clear_http_readers();
    swagger_parser::parser::reference::add_http_reader(std::rc::Rc::new(|uri: &str, _| {
        uri.starts_with("http://example.com/custom").then(|| Ok(CUSTOM_DEREFERENCER_DOMAIN.to_string()))
    }));
}

pub const CUSTOM_DEREFERENCER_DOMAIN: &str = "openapi: 3.1.0\ninfo:\n  title: Domain\n  version: '1.0'\ncomponents:\n  pathItems:\n    ExternalRef:\n      get:\n        description: ExternalRef domain\n        operationId: ExternalRef PathItem\n        responses:\n          '200':\n            description: OK";

/// `FileUtils.readFileToString(new File(path))`
pub fn read_file(path: &str) -> String {
    String::from_utf8_lossy(&std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"))).into_owned()
}

/// `List.toString()` of messages
pub fn java_list(v: &[String]) -> String {
    format!("[{}]", v.join(", "))
}

/// A `NetUtils` whose host name resolution uses a fixed table instead of DNS (for tests that name real
/// internet hosts); every other method is the real one.
pub struct FixedNetUtils {
    pub hosts: Vec<(&'static str, &'static str)>,
}

impl Default for FixedNetUtils {
    fn default() -> Self {
        FixedNetUtils {
            hosts: vec![
                ("localhost", "127.0.0.1"),
                ("petstore.swagger.io", "3.162.103.37"),
                ("petstore3.swagger.io", "3.162.103.62"),
            ],
        }
    }
}

impl swagger_parser_safe_url_resolver::NetUtils for FixedNetUtils {
    fn get_host_by_name(&self, hostname: &str) -> swagger_parser::java::JResult<swagger_parser_safe_url_resolver::java_rt::inet::InetAddress> {
        use swagger_parser_safe_url_resolver::java_rt::inet::InetAddress;
        match self.hosts.iter().find(|(h, _)| h.eq_ignore_ascii_case(hostname)) {
            Some((_, ip)) => InetAddress::get_by_name(ip),
            None if InetAddress::get_by_name(hostname).is_ok() && !hostname.chars().any(|c| c.is_ascii_alphabetic()) => InetAddress::get_by_name(hostname),
            None => Err(swagger_parser::java::JavaException::new("java.net.UnknownHostException", format!("{hostname}: nodename nor servname provided, or not known"))),
        }
    }
}

/// One `RemoteUrl.urlToString` invocation seen by [`mock_remote_url`]: (url, auths as `(keyName, value, type)`)
pub type RemoteCall = (String, Vec<(Option<String>, Option<String>, Option<String>)>);

/// `@Mocked RemoteUrl` with `RemoteUrl.urlToString(url, ...); result = contents` recorded for each
/// `(url, contents)`. Every invocation is recorded; an unrecorded URL fails (the mock would return null).
pub struct RemoteUrlMock {
    pub calls: std::rc::Rc<std::cell::RefCell<Vec<RemoteCall>>>,
    _guard: swagger_parser::parser::hooks::HooksGuard,
}

impl RemoteUrlMock {
    pub fn count(&self, url: &str) -> usize {
        self.calls.borrow().iter().filter(|(u, _)| u == url).count()
    }

    pub fn calls(&self) -> Vec<RemoteCall> {
        self.calls.borrow().clone()
    }
}

pub fn mock_remote_url(expectations: &[(&str, &str)]) -> RemoteUrlMock {
    use swagger_parser::java::JavaException;
    let calls: std::rc::Rc<std::cell::RefCell<Vec<RemoteCall>>> = Default::default();
    let recorded: Vec<(String, String)> = expectations.iter().map(|(u, c)| (u.to_string(), c.to_string())).collect();
    let c = calls.clone();
    let hooks = swagger_parser::parser::hooks::Hooks {
        url_to_string: Some(std::rc::Rc::new(move |url: &str, auths: &[swagger_parser::core::AuthorizationValue]| {
            c.borrow_mut().push((url.to_string(), auths.iter().map(|a| (a.key_name.clone(), a.value.clone(), a.type_.clone())).collect()));
            Some(match recorded.iter().find(|(u, _)| u == url) {
                Some((_, contents)) => Ok(contents.clone()),
                None => Err(JavaException::new("java.lang.IllegalStateException", format!("unrecorded RemoteUrl.urlToString({url})"))),
            })
        })),
        ..Default::default()
    };
    RemoteUrlMock { calls, _guard: swagger_parser::parser::hooks::install(hooks) }
}
