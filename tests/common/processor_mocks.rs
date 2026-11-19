//! `@Mocked XxxProcessor` for the processor tests: every method of a mocked class is replaced (returning the
//! mock default, or a recorded result) and every invocation and construction is recorded, so the tests can
//! check `times = N` and (`Full`)`Verifications`.

use std::cell::RefCell;
use std::rc::Rc;
use swagger_parser::java::JList;
use swagger_parser::models::*;
use swagger_parser::parser::processors::mocks::{self, MocksGuard, ProcessorMocks};
use swagger_parser::parser::ref_utils::RefFormat;

#[derive(Clone, Debug)]
pub enum ProcCall {
    Construct(&'static str),
    Schema(Schema),
    Response(ApiResponse),
    RequestBody(RequestBody),
    Parameter(Parameter),
    Parameters(Option<JList<Parameter>>),
    Header(Header),
    Example(Example),
    Link(Link),
    Callback(Callback),
    SecurityScheme(SecurityScheme),
    /// `ExternalRefProcessor.processRefToExternal<kind>(ref, format)`
    External(String, String, RefFormat),
}

pub type Calls = Rc<RefCell<Vec<ProcCall>>>;

pub struct Mocked {
    pub calls: Calls,
    _guard: MocksGuard,
}

/// Recorded `result`s of mocked methods
#[derive(Default, Clone)]
pub struct Results {
    /// `processRefToExternal<kind>(ref, format)`: (kind, ref, format, result)
    pub external: Vec<(String, String, RefFormat, Option<String>)>,
    /// `processParameters(list)`: result
    pub parameters: Option<Option<JList<Parameter>>>,
}

impl Mocked {
    /// Mocks the named classes (`"SchemaProcessor"`, `"ExternalRefProcessor"`, ...); constructions of every
    /// processor are recorded.
    pub fn new(classes: &[&str], results: Results) -> Mocked {
        let calls: Calls = Rc::new(RefCell::new(Vec::new()));
        let has = |c: &str| classes.contains(&c);
        let mut m = ProcessorMocks::default();
        // constructors of mocked classes do nothing: constructions nested in one are not recorded
        let mocked_classes: Vec<String> = classes.iter().map(|s| s.to_string()).collect();
        let stack: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));
        let (c, st) = (calls.clone(), stack.clone());
        m.on_construct = Some(Rc::new(move |name| {
            let inside_mock = st.borrow().iter().any(|n| mocked_classes.iter().any(|m| m == n));
            if !inside_mock {
                c.borrow_mut().push(ProcCall::Construct(name));
            }
            st.borrow_mut().push(name);
        }));
        let st = stack.clone();
        m.on_construct_end = Some(Rc::new(move |_| {
            st.borrow_mut().pop();
        }));
        macro_rules! handler {
            ($class:expr, $field:ident, $variant:ident) => {
                if has($class) {
                    let c = calls.clone();
                    m.$field = Some(Rc::new(move |x| {
                        c.borrow_mut().push(ProcCall::$variant(x.clone()));
                        Some(Ok(()))
                    }));
                }
            };
        }
        handler!("SchemaProcessor", process_schema, Schema);
        handler!("ResponseProcessor", process_response, Response);
        handler!("RequestBodyProcessor", process_request_body, RequestBody);
        handler!("ParameterProcessor", process_parameter, Parameter);
        handler!("HeaderProcessor", process_header, Header);
        handler!("ExampleProcessor", process_example, Example);
        handler!("LinkProcessor", process_link, Link);
        handler!("CallbackProcessor", process_callback, Callback);
        if has("ParameterProcessor") {
            let c = calls.clone();
            let r = results.parameters.clone();
            m.process_parameters = Some(Rc::new(move |l| {
                c.borrow_mut().push(ProcCall::Parameters(l.cloned()));
                Some(Ok(r.clone().flatten()))
            }));
        }
        if has("SecuritySchemeProcessor") {
            let c = calls.clone();
            m.process_security_scheme = Some(Rc::new(move |s| {
                c.borrow_mut().push(ProcCall::SecurityScheme(s.clone()));
                // a mocked method returning an object type: a cascaded instance
                Some(Ok(SecurityScheme::new()))
            }));
        }
        if has("ExternalRefProcessor") {
            let c = calls.clone();
            let ext = results.external.clone();
            m.external_ref = Some(Rc::new(move |kind, r, f| {
                c.borrow_mut().push(ProcCall::External(kind.to_string(), r.to_string(), f));
                let res = ext.iter().find(|(k, rr, ff, _)| k == kind && rr == r && *ff == f).and_then(|x| x.3.clone());
                Some(Ok(res))
            }));
        }
        Mocked { calls, _guard: mocks::install(m) }
    }

    pub fn calls(&self) -> Vec<ProcCall> {
        self.calls.borrow().clone()
    }

    pub fn clear(&self) {
        self.calls.borrow_mut().clear();
    }

    /// Number of `new <class>(...)`
    pub fn constructed(&self, class: &str) -> usize {
        self.calls().iter().filter(|c| matches!(c, ProcCall::Construct(n) if *n == class)).count()
    }

    /// Invocations other than constructions
    pub fn invocations(&self) -> Vec<ProcCall> {
        self.calls().into_iter().filter(|c| !matches!(c, ProcCall::Construct(_))).collect()
    }

    /// `processSchema(s)` invocations with this very instance
    pub fn schema_count(&self, s: &Schema) -> usize {
        self.calls().iter().filter(|c| matches!(c, ProcCall::Schema(x) if x.ptr_eq(s))).count()
    }

    pub fn external_count(&self, kind: &str, r: &str, f: RefFormat) -> usize {
        self.calls().iter().filter(|c| matches!(c, ProcCall::External(k, rr, ff) if k == kind && rr == r && *ff == f)).count()
    }
}
