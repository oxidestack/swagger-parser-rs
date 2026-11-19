//! Port of `io.swagger.v3.parser.test.ResolverCacheTest` (`@Mocked RefUtils`: the `readExternal*Ref` methods
//! the cache calls return the recorded contents, or null when unrecorded; `DeserializationUtils` is partially
//! mocked in two tests; `@Injectable` models are fresh objects).

mod common;

use jackson::json::{read_tree, ReadFeatures};
use jackson::JsonNode;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use swagger_parser::core::{AuthorizationValue, ParseOptions};
use swagger_parser::java::net::JavaUri;
use swagger_parser::java::path::JavaPath;
use swagger_parser::java::JSet;
use swagger_parser::models::*;
use swagger_parser::parser::hooks::{self, Hooks, HooksGuard};
use swagger_parser::parser::ref_utils::RefFormat;
use swagger_parser::parser::resolver_cache::{Cache, Component, ResolverCache};
use swagger_parser_safe_url_resolver::{NetUtils, PermittedUrlsChecker};

type Call = (&'static str, String, RefFormat, Option<String>);

/// `@Mocked RefUtils` (+ the partial `DeserializationUtils` mock of `testMock` / `testLoadExternalRef_NoDefinitionPath`)
struct RefUtilsMock {
    calls: Rc<RefCell<Vec<Call>>>,
    deserialize_calls: Rc<RefCell<usize>>,
    _guard: HooksGuard,
}

impl RefUtilsMock {
    fn count(&self, method: &str, file: &str, format: RefFormat, root: Option<&str>) -> usize {
        self.calls.borrow().iter().filter(|(m, f, fmt, r)| *m == method && f == file && *fmt == format && r.as_deref() == root).count()
    }
}

/// A recorded read: `(method, file, format, root, contents)`.
type Recorded = (&'static str, String, RefFormat, String, String);

/// Recorded `(method, file, format, root, contents)`
fn mock_ref_utils(recorded: &[(&'static str, &str, RefFormat, &str, &str)], deserialize: Option<(&str, &str, JsonNode)>) -> RefUtilsMock {
    let calls: Rc<RefCell<Vec<Call>>> = Default::default();
    let recorded: Rc<Vec<Recorded>> =
        Rc::new(recorded.iter().map(|(m, f, fmt, r, c)| (*m, f.to_string(), *fmt, r.to_string(), c.to_string())).collect());
    let hook = |method: &'static str| -> hooks::ReadExternalRefHook {
        let (calls, recorded) = (calls.clone(), recorded.clone());
        Rc::new(move |file: &str, format: RefFormat, auths: &[AuthorizationValue], root: Option<&str>, _checker: Option<&PermittedUrlsChecker>| {
            assert!(auths.is_empty());
            calls.borrow_mut().push((method, file.to_string(), format, root.map(str::to_string)));
            let hit = recorded.iter().find(|(m, f, fmt, r, _)| *m == method && f == file && *fmt == format && Some(r.as_str()) == root);
            Some(Ok(hit.map(|x| x.4.clone())))
        })
    };
    let deserialize_calls: Rc<RefCell<usize>> = Default::default();
    let deserialize_hook: Option<hooks::DeserializeIntoTreeHook> = deserialize.map(|(contents, uri, tree)| {
        let (contents, uri, dc) = (contents.to_string(), uri.to_string(), deserialize_calls.clone());
        let hook: hooks::DeserializeIntoTreeHook = Rc::new(move |c: &str, u: Option<&str>, _o: Option<&ParseOptions>, _r: &mut swagger_parser::core::SwaggerParseResult| {
            if c == contents && u == Some(uri.as_str()) {
                *dc.borrow_mut() += 1;
                Some(Ok(tree.clone()))
            } else {
                None
            }
        });
        hook
    });
    let guard = hooks::install(Hooks {
        read_external_ref: Some(hook("readExternalRef")),
        read_external_url_ref: Some(hook("readExternalUrlRef")),
        read_external_classpath_ref: Some(hook("readExternalClasspathRef")),
        deserialize_into_tree: deserialize_hook,
        ..Default::default()
    });
    RefUtilsMock { calls, deserialize_calls, _guard: guard }
}

fn no_recordings() -> RefUtilsMock {
    mock_ref_utils(&[], None)
}

const AUTHS: &[AuthorizationValue] = &[];

fn cache(open_api: &OpenAPI, parent: Option<&str>) -> ResolverCache {
    ResolverCache::new(Some(open_api), Some(AUTHS), parent).unwrap()
}

fn schema_of(c: &Option<Component>) -> Schema {
    match c {
        Some(Component::Schema(s)) => s.clone(),
        other => panic!("not a Schema: {other:?}"),
    }
}

fn string_schema_tree() -> JsonNode {
    read_tree("{\"type\":  \"string\"}", ReadFeatures::default()).unwrap()
}

fn validating_options() -> ParseOptions {
    let mut o = ParseOptions::new();
    o.resolve = true;
    o.validate_external_refs = true;
    o
}

#[test]
fn test_mock() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPI::new();
    let format = RefFormat::Url;
    let r = "http://my.company.com/path/to/file.json";
    let contents = "really good json";
    let parent = "http://my.company.com/path/parent.json";
    let m = mock_ref_utils(&[("readExternalUrlRef", r, format, parent, contents)], Some((contents, r, string_schema_tree())));
    let cache = ResolverCache::with_options(Some(&open_api), Some(AUTHS), Some(parent), Some(JSet::new_hash()), validating_options()).unwrap();

    let first: Option<Schema> = cache.load_ref(r, RefFormat::Url).unwrap();

    assert_eq!(m.count("readExternalUrlRef", r, format, Some(parent)), 1);
    assert_eq!(*m.deserialize_calls.borrow(), 1);
    assert_eq!(first.unwrap().get_type().as_deref(), Some("string"));
}

#[test]
fn test_load_external_ref_no_definition_path() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPI::new();
    let format = RefFormat::Url;
    let r = "http://my.company.com/path/to/file.json";
    let contents = "really good json";
    let parent = "http://my.company.com/path/parent.json";
    let m = mock_ref_utils(&[("readExternalUrlRef", r, format, parent, contents)], Some((contents, r, string_schema_tree())));
    let cache = ResolverCache::with_options(Some(&open_api), Some(AUTHS), Some(parent), Some(JSet::new_hash()), validating_options()).unwrap();

    let first: Option<Schema> = cache.load_ref(r, RefFormat::Url).unwrap();

    assert_eq!(cache.get_external_file_cache().get(r).cloned().flatten().as_deref(), Some(contents));
    assert_eq!(schema_of(&cache.get_resolution_cache().get(r).cloned().flatten()).get_type().as_deref(), Some("string"));
    assert_eq!(first.unwrap().get_type().as_deref(), Some("string"));

    // requesting the same ref a second time should not result in reading the external file again
    let second: Option<Schema> = cache.load_ref(r, format).unwrap();
    assert_eq!(second.unwrap().get_type().as_deref(), Some("string"));
    assert_eq!(m.count("readExternalUrlRef", r, format, Some(parent)), 1);
    assert_eq!(*m.deserialize_calls.borrow(), 1);
}

#[test]
fn test_load_external_ref_with_escaped_characters() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPI::new();
    let format = RefFormat::Url;
    let r = "http://my.company.com/path/to/main.yaml";
    let contents = "openAPI: \"2.0\"\n\ninfo:\n  version: 1.0.0\n  title: Path include test case child\n\npaths:\n  /foo~bar~1:\n    get:\n      responses:\n        200:\n          description: \"Request successful\"\n";
    let parent = "http://my.company.com/path/parent.json";
    let m = mock_ref_utils(&[("readExternalUrlRef", r, format, parent, contents)], None);
    let cache = cache(&open_api, Some(parent));

    let path: Option<PathItem> = cache.load_ref(&format!("{r}#/paths/~1foo~0bar~01"), RefFormat::Url).unwrap();
    assert!(path.is_some());
    assert_eq!(m.count("readExternalUrlRef", r, format, Some(parent)), 1);
}

#[test]
fn test_load_external_ref_response_with_no_content() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPI::new();
    let format = RefFormat::Url;
    let r = "http://my.company.com/path/to/main.yaml";
    let contents = "openapi: 3.0.0\n\ninfo:\n  version: 1.0.0\n  title: Response include test case child\n\ncomponents:\n  responses:\n    200:\n      description: Success\n";
    let parent = "http://my.company.com/path/parent.json";
    let m = mock_ref_utils(&[("readExternalUrlRef", r, format, parent, contents)], None);
    let cache = cache(&open_api, Some(parent));

    let response: Option<ApiResponse> = cache.load_ref(&format!("{r}#/components/responses/200"), RefFormat::Url).unwrap();
    let response = response.expect("response");
    assert_eq!(response.get_description().as_deref(), Some("Success"));
    assert!(response.get_content().is_none());
    assert_eq!(m.count("readExternalUrlRef", r, format, Some(parent)), 1);
}

#[test]
fn test_load_internal_parameter_ref() {
    let _g = common::setup(common::V3_MODULE);
    let _m = no_recordings();
    let open_api = OpenAPI::new();
    let mocked_parameter = Parameter::new();
    open_api.components(Components::new().add_parameters("foo", mocked_parameter.clone()));

    let cache = cache(&open_api, None);
    let actual: Option<Parameter> = cache.load_ref("#/components/parameters/foo", RefFormat::Internal).unwrap();
    assert!(actual.unwrap().ptr_eq(&mocked_parameter));

    assert!(cache.load_ref::<Parameter>("#/components/parameters/bar", RefFormat::Internal).unwrap().is_none());
    assert!(cache.load_ref::<Parameter>("#/params/foo", RefFormat::Internal).unwrap().is_none());
}

#[test]
fn test_load_internal_parameter_ref_with_spaces() {
    let _g = common::setup(common::V3_MODULE);
    let _m = no_recordings();
    let open_api = OpenAPI::new();
    let mocked_parameter = Parameter::new();
    open_api.components(Components::new().add_parameters("foo bar", mocked_parameter.clone()));

    let cache = cache(&open_api, None);
    let actual: Option<Parameter> = cache.load_ref("#/components/parameters/foo bar", RefFormat::Internal).unwrap();
    assert!(actual.unwrap().ptr_eq(&mocked_parameter));
}

#[test]
fn test_load_internal_definition_ref() {
    let _g = common::setup(common::V3_MODULE);
    let _m = no_recordings();
    let open_api = OpenAPI::new();
    let mocked_model = Schema::new();
    open_api.components(Components::new().add_schemas("foo", mocked_model.clone()));

    let cache = cache(&open_api, None);
    let actual: Option<Schema> = cache.load_ref("#/components/schemas/foo", RefFormat::Internal).unwrap();
    assert!(actual.unwrap().ptr_eq(&mocked_model));

    assert!(cache.load_ref::<Schema>("#/components/schemas/bar", RefFormat::Internal).unwrap().is_none());
    assert!(cache.load_ref::<Schema>("#/defs/bar", RefFormat::Internal).unwrap().is_none());
}

#[test]
fn test_load_internal_definition_ref_with_spaces() {
    let _g = common::setup(common::V3_MODULE);
    let _m = no_recordings();
    let open_api = OpenAPI::new();
    let mocked_model = Schema::new();
    open_api.components(Components::new().add_schemas("foo bar", mocked_model.clone()));

    let cache = cache(&open_api, None);
    let actual: Option<Schema> = cache.load_ref("#/components/schemas/foo bar", RefFormat::Internal).unwrap();
    assert!(actual.unwrap().ptr_eq(&mocked_model));
}

#[test]
fn test_load_internal_definition_ref_with_escaped_characters() {
    let _g = common::setup(common::V3_MODULE);
    let _m = no_recordings();
    let open_api = OpenAPI::new();
    let mocked_model = Schema::new();
    open_api.components(Components::new().add_schemas("foo~bar/baz~1", mocked_model.clone()));

    let cache = cache(&open_api, None);
    let actual: Option<Schema> = cache.load_ref("#/components/schemas/foo~0bar~1baz~01", RefFormat::Internal).unwrap();
    assert!(actual.unwrap().ptr_eq(&mocked_model));
}

#[test]
fn test_load_internal_response_ref() {
    let _g = common::setup(common::V3_MODULE);
    let _m = no_recordings();
    let open_api = OpenAPI::new();
    let mocked_response = ApiResponse::new();
    open_api.components(Components::new().add_responses("foo", mocked_response.clone()));

    let cache = cache(&open_api, None);
    let actual: Option<ApiResponse> = cache.load_ref("#/components/responses/foo", RefFormat::Internal).unwrap();
    assert!(actual.unwrap().ptr_eq(&mocked_response));

    assert!(cache.load_ref::<ApiResponse>("#/components/responses/bar", RefFormat::Internal).unwrap().is_none());
}

#[test]
fn test_load_internal_response_ref_with_spaces() {
    let _g = common::setup(common::V3_MODULE);
    let _m = no_recordings();
    let open_api = OpenAPI::new();
    let mocked_response = ApiResponse::new();
    open_api.components(Components::new().add_responses("foo bar", mocked_response.clone()));

    let cache = cache(&open_api, None);
    let actual: Option<ApiResponse> = cache.load_ref("#/components/responses/foo bar", RefFormat::Internal).unwrap();
    assert!(actual.unwrap().ptr_eq(&mocked_response));
}

fn root_document_uri(cache: &ResolverCache) -> Option<String> {
    cache.root_document_uri().map(|u| u.to_java_string())
}

fn uri(s: &str) -> JavaUri {
    JavaUri::parse(s).unwrap()
}

/// `Files.createTempFile(prefix, suffix)`
fn create_temp_file(prefix: &str, suffix: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir();
    let name = format!("{prefix}{}{}{suffix}", std::process::id(), N.fetch_add(1, Ordering::Relaxed));
    let p = dir.join(name);
    std::fs::write(&p, b"").unwrap();
    p.to_string_lossy().into_owned()
}

#[test]
fn test_root_document_uri_is_established_once_for_supported_roots() {
    let _g = common::setup(common::V3_MODULE);
    let _m = no_recordings();
    let open_api = OpenAPI::new();
    let root_file = create_temp_file("resolver-cache-root", ".yaml");
    let filesystem_cache = cache(&open_api, Some(&root_file));
    let filesystem_uri = filesystem_cache.root_document_uri().cloned();
    std::fs::remove_file(&root_file).unwrap();

    let expected = JavaPath::new(&root_file).to_absolute_path().normalize().to_uri();
    assert!(filesystem_uri.as_ref().unwrap().java_equals(&uri(&expected)), "{filesystem_uri:?} {expected}");
    assert!(filesystem_cache.root_document_uri().unwrap().java_equals(filesystem_uri.as_ref().unwrap()));

    let file_cache = cache(&open_api, Some("file:///tmp/schemas/../root.yaml?v=1#section"));
    assert!(file_cache.root_document_uri().unwrap().java_equals(&uri("file:/tmp/root.yaml?v=1")), "{:?}", root_document_uri(&file_cache));

    let http_cache = cache(&open_api, Some("https://example.com/api/../root.yaml?v=1#section"));
    assert!(http_cache.root_document_uri().unwrap().java_equals(&uri("https://example.com/root.yaml?v=1")), "{:?}", root_document_uri(&http_cache));
}

#[test]
fn test_unsupported_and_ambiguous_roots_disable_root_identity() {
    let _g = common::setup(common::V3_MODULE);
    let _m = no_recordings();
    let open_api = OpenAPI::new();
    let roots = ["classpath:/root.yaml", "jar:file:/tmp/specs.jar!/root.yaml", "urn:swagger:root", "http://example.com/%zz", "missing-relative-root.yaml"];
    for root in roots {
        assert!(cache(&open_api, Some(root)).root_document_uri().is_none(), "{root}");
    }
}

#[test]
fn test_classpath_root_keeps_existing_external_loading_path() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPI::new();
    let root_schema = Schema::new().type_("object");
    open_api.components(Components::new().add_schemas("Foo", root_schema.clone()));
    let root = "classpath:/root.yaml";
    let file = "external.yaml";
    let contents = "components:\n  schemas:\n    Foo:\n      type: string\n";
    let m = mock_ref_utils(&[("readExternalClasspathRef", file, RefFormat::Relative, root, contents)], None);

    let result: Schema = cache(&open_api, Some(root)).load_ref(&format!("{file}#/components/schemas/Foo"), RefFormat::Relative).unwrap().unwrap();

    assert_eq!(m.count("readExternalClasspathRef", file, RefFormat::Relative, Some(root)), 1);
    assert!(!result.ptr_eq(&root_schema));
    assert_eq!(result.get_type().as_deref(), Some("string"));
}

/// `NetUtils` resolving every host name to a public address (DNS without the network)
struct PublicDns;

impl NetUtils for PublicDns {
    fn get_host_by_name(&self, _hostname: &str) -> swagger_parser::java::JResult<java_rt::inet::InetAddress> {
        java_rt::inet::InetAddress::get_by_name("93.184.215.14")
    }
}

#[test]
fn test_dot_segment_http_reference_reuses_root_without_loading() {
    let _g = common::setup(common::V3_MODULE);
    let m = no_recordings();
    let open_api = OpenAPI::new();
    let root_schema = Schema::new().type_("object");
    open_api.components(Components::new().add_schemas("Foo", root_schema.clone()));
    let root = "https://example.com/api/root.yaml";
    let file = "https://example.com/api/nested/../root.yaml";
    let r = format!("{file}#/components/schemas/Foo");
    let root_auths = vec![AuthorizationValue::new("Authorization", "token", "header")];
    let mut options = ParseOptions::new();
    options.safely_resolve_url = true;

    let permission_checks = Rc::new(RefCell::new(0));
    let cache = ResolverCache::with_options(Some(&open_api), Some(&root_auths), Some(root), Some(JSet::new_hash()), options)
        .unwrap()
        .with_permitted_urls_checker(PermittedUrlsChecker::with_net_utils(None, None, Arc::new(PublicDns)).unwrap());
    // the overriding checkUrlIsPermitted counts and calls super
    let pc = permission_checks.clone();
    cache.set_check_url_is_permitted_observer(Rc::new(move |_| *pc.borrow_mut() += 1));

    let result: Schema = cache.load_ref(&r, RefFormat::Url).unwrap().unwrap();

    assert!(result.ptr_eq(&root_schema));
    assert_eq!(*permission_checks.borrow(), 1);
    assert!(!cache.get_external_file_cache().contains_key(file));
    assert!(schema_of(&cache.get_resolution_cache().get(&r).cloned().flatten()).ptr_eq(&root_schema));
    assert!(m.calls.borrow().is_empty());
}

#[test]
fn test_root_reference_uses_object_present_at_cache_construction() {
    let _g = common::setup(common::V3_MODULE);
    let _m = no_recordings();
    let open_api = OpenAPI::new();
    let original_root_schema = Schema::new().type_("object");
    open_api.components(Components::new().add_schemas("Foo", original_root_schema.clone()));
    let root = "https://example.com/api/root.yaml";
    let cache = cache(&open_api, Some(root));

    open_api.get_components().unwrap().get_schemas().unwrap().put("Foo", Schema::new().type_("string"));

    let result: Schema = cache.load_ref(&format!("{root}#/components/schemas/Foo"), RefFormat::Url).unwrap().unwrap();
    assert!(result.ptr_eq(&original_root_schema));
}

#[test]
fn test_same_filename_in_different_directory_does_not_reuse_root() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPI::new();
    let root_schema = Schema::new().type_("object");
    open_api.components(Components::new().add_schemas("Foo", root_schema.clone()));
    let root = "https://example.com/api/root.yaml";
    let file = "https://example.com/api/sub/root.yaml";
    let contents = "components:\n  schemas:\n    Foo:\n      type: string\n";
    let m = mock_ref_utils(&[("readExternalUrlRef", file, RefFormat::Url, root, contents)], None);

    let result: Schema = cache(&open_api, Some(root)).load_ref(&format!("{file}#/components/schemas/Foo"), RefFormat::Url).unwrap().unwrap();

    assert_eq!(m.count("readExternalUrlRef", file, RefFormat::Url, Some(root)), 1);
    assert!(!result.ptr_eq(&root_schema));
    assert_eq!(result.get_type().as_deref(), Some("string"));
}

#[test]
fn test_http_query_is_part_of_root_identity() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPI::new();
    let root_schema = Schema::new().type_("object");
    open_api.components(Components::new().add_schemas("Foo", root_schema.clone()));
    let root = "https://example.com/api/root.yaml?v=1";
    let file = "https://example.com/api/root.yaml?v=2";
    let contents = "components:\n  schemas:\n    Foo:\n      type: integer\n";
    let m = mock_ref_utils(&[("readExternalUrlRef", file, RefFormat::Url, root, contents)], None);

    let result: Schema = cache(&open_api, Some(root)).load_ref(&format!("{file}#/components/schemas/Foo"), RefFormat::Url).unwrap().unwrap();

    assert_eq!(m.count("readExternalUrlRef", file, RefFormat::Url, Some(root)), 1);
    assert!(!result.ptr_eq(&root_schema));
    assert_eq!(result.get_type().as_deref(), Some("integer"));
}

#[test]
fn test_unsupported_absent_and_wrong_type_pointers_use_external_document() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPI::new();
    let root_schema = Schema::new().type_("object");
    open_api.components(Components::new().add_schemas("Foo", root_schema));
    let root = "https://example.com/api/root.yaml";
    let contents = "components:\n  schemas:\n    External:\n      type: string\n    Foo:\n      name: id\n      in: query\n      properties:\n        value:\n          type: integer\n";
    let m = mock_ref_utils(&[("readExternalUrlRef", root, RefFormat::Url, root, contents)], None);

    let cache = cache(&open_api, Some(root));
    let absent: Schema = cache.load_ref(&format!("{root}#/components/schemas/External"), RefFormat::Url).unwrap().unwrap();
    let deeper: Schema = cache.load_ref(&format!("{root}#/components/schemas/Foo/properties/value"), RefFormat::Url).unwrap().unwrap();
    let wrong_type: Option<Parameter> = cache.load_ref(&format!("{root}#/components/schemas/Foo"), RefFormat::Url).unwrap();

    assert_eq!(m.count("readExternalUrlRef", root, RefFormat::Url, Some(root)), 1);
    assert_eq!(absent.get_type().as_deref(), Some("string"));
    assert_eq!(deeper.get_type().as_deref(), Some("integer"));
    let wrong_type = wrong_type.expect("wrongType");
    assert_eq!(wrong_type.get_name().as_deref(), Some("id"));
}

#[test]
fn test_root_reuse_does_not_replace_existing_canonical_entry() {
    let _g = common::setup(common::V3_MODULE);
    let _m = no_recordings();
    let open_api = OpenAPI::new();
    let root_header = Header::new().description("root");
    open_api.components(Components::new().add_headers("Root", root_header.clone()));
    let root = "https://example.com/api/root.yaml";
    let r = format!("{root}#/components/headers/Root");
    let cache = cache(&open_api, Some(root));
    let existing_canonical_object = Schema::new().type_("string");
    cache.put_canonical_resolution(&r, Some(Component::Schema(existing_canonical_object.clone())));

    let result: Header = cache.load_ref(&r, RefFormat::Url).unwrap().unwrap();

    assert!(result.ptr_eq(&root_header));
    assert!(matches!(cache.get_resolution_cache().get(&r), Some(Some(Component::Header(h))) if h.ptr_eq(&root_header)));
    assert!(schema_of(&cache.canonical_resolution_cache().get(&r).cloned().flatten()).ptr_eq(&existing_canonical_object));
}

#[test]
fn test_rename_cache() {
    let _g = common::setup(common::V3_MODULE);
    let _m = no_recordings();
    let cache = cache(&OpenAPI::new(), None);
    assert!(cache.get_renamed_ref(Some("foo")).is_none());
    cache.put_renamed_ref("foo", "bar");
    assert_eq!(cache.get_renamed_ref(Some("foo")).as_deref(), Some("bar"));
}

#[test]
fn test_equivalent_uri_refs_share_cache_identity() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPI::new();
    let ref_with_dot_segments = "http://my.company.com/schemas/../schemas/file.yaml#/components/schemas/Foo";
    let canonical_ref = "http://my.company.com/schemas/file.yaml#/components/schemas/Foo";
    let contents = "components:\n  schemas:\n    Foo:\n      type: string\n";
    let file = "http://my.company.com/schemas/../schemas/file.yaml";
    let root = "http://my.company.com/root.yaml";
    let m = mock_ref_utils(&[("readExternalUrlRef", file, RefFormat::Url, root, contents)], None);

    let cache = cache(&open_api, Some(root));
    let first: Schema = cache.load_ref(ref_with_dot_segments, RefFormat::Url).unwrap().unwrap();
    let second: Schema = cache.load_ref(canonical_ref, RefFormat::Url).unwrap().unwrap();

    assert_eq!(m.count("readExternalUrlRef", file, RefFormat::Url, Some(root)), 1);
    assert_eq!(m.calls.borrow().len(), 1);
    assert!(first.ptr_eq(&second));
    let files: HashMap<String, Option<String>> = cache.get_external_file_cache();
    assert_eq!(files.len(), 2);
    assert_eq!(files.get(file).cloned().flatten().as_deref(), Some(contents));
    assert_eq!(files.get("http://my.company.com/schemas/file.yaml").cloned().flatten().as_deref(), Some(contents));
    let resolutions = cache.get_resolution_cache();
    assert_eq!(resolutions.len(), 2);
    assert!(schema_of(&resolutions.get(ref_with_dot_segments).cloned().flatten()).ptr_eq(&first));
    assert!(schema_of(&resolutions.get(canonical_ref).cloned().flatten()).ptr_eq(&first));
    cache.put_renamed_ref(ref_with_dot_segments, "Foo");
    assert_eq!(cache.get_renamed_ref(Some(canonical_ref)).as_deref(), Some("Foo"));
    assert_eq!(cache.get_rename_cache().get(ref_with_dot_segments).map(String::as_str), Some("Foo"));
    assert!(!cache.get_rename_cache().contains_key(canonical_ref));
}

#[test]
fn test_equivalent_api_response_refs_share_cache_identity() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPI::new();
    let ref_with_dot_segments = "http://my.company.com/responses/../responses/common.yaml#/components/responses/Error";
    let canonical_ref = "http://my.company.com/responses/common.yaml#/components/responses/Error";
    let contents = "components:\n  responses:\n    Error:\n      description: Error response\n";
    let file = "http://my.company.com/responses/../responses/common.yaml";
    let root = "http://my.company.com/root.yaml";
    let m = mock_ref_utils(&[("readExternalUrlRef", file, RefFormat::Url, root, contents)], None);

    let cache = cache(&open_api, Some(root));
    let first: ApiResponse = cache.load_ref(ref_with_dot_segments, RefFormat::Url).unwrap().unwrap();
    let second: ApiResponse = cache.load_ref(canonical_ref, RefFormat::Url).unwrap().unwrap();

    assert_eq!(m.count("readExternalUrlRef", file, RefFormat::Url, Some(root)), 1);
    assert!(first.ptr_eq(&second));
    assert_eq!(first.get_description().as_deref(), Some("Error response"));
    assert_eq!(cache.get_external_file_cache().len(), 2);
    let resolutions = cache.get_resolution_cache();
    assert_eq!(resolutions.len(), 2);
    assert!(matches!(resolutions.get(ref_with_dot_segments), Some(Some(Component::ApiResponse(r))) if r.ptr_eq(&first)));
    assert!(matches!(resolutions.get(canonical_ref), Some(Some(Component::ApiResponse(r))) if r.ptr_eq(&first)));
}

#[test]
fn test_issue2016_equivalent_relative_refs_share_rename_cache_identity() {
    let _g = common::setup(common::V3_MODULE);
    let _m = no_recordings();
    let cache = cache(&OpenAPI::new(), None);
    let ref_with_redundant_dot_segment = "./../A.yaml#/components/schemas/A";
    let equivalent_ref = "../A.yaml#/components/schemas/A";

    cache.put_renamed_ref(ref_with_redundant_dot_segment, "A");

    assert_eq!(cache.get_renamed_ref(Some(equivalent_ref)).as_deref(), Some("A"));
    assert_eq!(cache.get_rename_cache().len(), 1);
    assert_eq!(cache.get_rename_cache().get(ref_with_redundant_dot_segment).map(String::as_str), Some("A"));
    assert!(!cache.get_rename_cache().contains_key(equivalent_ref));
}

#[test]
fn test_ref_equivalence_uses_normalized_cache_identity() {
    let _g = common::setup(common::V3_MODULE);
    let _m = no_recordings();
    let cache = cache(&OpenAPI::new(), None);
    assert!(cache.refs_are_equivalent(
        Some("./components/schemas/Thing.yaml#/components/schemas/Thing"),
        Some("components/foo/../schemas/Thing.yaml#/components/schemas/Thing")
    ));
    assert!(!cache.refs_are_equivalent(Some("inventory.yaml#/components/schemas/Pet"), Some("pets.yaml#/components/schemas/Pet")));
}

#[test]
fn test_canonical_rename_cache_keys_preserve_uri_parts() {
    let _g = common::setup(common::V3_MODULE);
    let _m = no_recordings();
    let cache = cache(&OpenAPI::new(), None);

    cache.put_renamed_ref("file:///tmp/schemas/../Foo.yaml#/components/schemas/Foo", "FileFoo");
    cache.put_renamed_ref("/tmp/schemas/../Foo.yaml#/components/schemas/Foo", "AbsoluteFoo");
    cache.put_renamed_ref("https://example.com/a/../Foo.yaml?version=1#/components/schemas/Foo", "HttpFoo");
    cache.put_renamed_ref("#/components/schemas/Foo", "InternalFoo");
    cache.put_renamed_ref("C:\\schemas\\Foo.yaml", "WindowsPath");
    cache.put_renamed_ref("my schemas/../Foo.yaml#/components/schemas/Foo", "UnencodedSpace");

    let rc = cache.get_rename_cache();
    let get = |k: &str| rc.get(k).map(String::as_str);
    assert_eq!(get("file:///tmp/schemas/../Foo.yaml#/components/schemas/Foo"), Some("FileFoo"));
    assert_eq!(get("/tmp/schemas/../Foo.yaml#/components/schemas/Foo"), Some("AbsoluteFoo"));
    assert_eq!(get("https://example.com/a/../Foo.yaml?version=1#/components/schemas/Foo"), Some("HttpFoo"));
    assert_eq!(get("#/components/schemas/Foo"), Some("InternalFoo"));
    assert_eq!(get("C:\\schemas\\Foo.yaml"), Some("WindowsPath"));
    assert_eq!(get("my schemas/../Foo.yaml#/components/schemas/Foo"), Some("UnencodedSpace"));
    assert_eq!(cache.get_renamed_ref(Some("file:/tmp/Foo.yaml#/components/schemas/Foo")).as_deref(), Some("FileFoo"));
    assert_eq!(cache.get_renamed_ref(Some("/tmp/Foo.yaml#/components/schemas/Foo")).as_deref(), Some("AbsoluteFoo"));
    assert_eq!(cache.get_renamed_ref(Some("https://example.com/Foo.yaml?version=1#/components/schemas/Foo")).as_deref(), Some("HttpFoo"));
}

