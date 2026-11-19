//! Port of `io.swagger.v3.parser.test.OpenAPIDereferencer31ThreadSafetyTest`: concurrent parsing of OpenAPI 3.1
//! specs with resolve=true must not swap results between threads (swagger-parser issue 2293). The Java test
//! submits 200 parses to a pool of 8 threads; here 8 std threads take the same 200 parses from a shared queue,
//! each parse running on its own thread (the Rust parser state is per call and per thread).

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use swagger_parser::core::ParseOptions;
use swagger_parser::parser::OpenAPIV3Parser;

fn make_spec(title: &str) -> String {
    format!(
        "{{\n  \"openapi\": \"3.1.0\",\n  \"info\": {{ \"title\": \"{title}\", \"version\": \"1.0.0\" }},\n  \"paths\": {{}},\n  \"components\": {{\n    \"schemas\": {{\n      \"Item\": {{\n        \"type\": \"object\",\n        \"properties\": {{ \"id\": {{ \"type\": \"string\" }} }}\n      }},\n      \"ItemList\": {{\n        \"type\": \"object\",\n        \"properties\": {{\n          \"items\": {{\n            \"type\": \"array\",\n            \"items\": {{ \"$ref\": \"#/components/schemas/Item\" }}\n          }}\n        }}\n      }}\n    }}\n  }}\n}}"
    )
}

fn parse_and_get_title(spec: &str) -> Option<String> {
    let options = ParseOptions { resolve: true, resolve_fully: false, ..Default::default() };
    let result = OpenAPIV3Parser::new().read_contents(spec, None, Some(&options));
    result.openapi.unwrap().get_info().unwrap().get_title()
}

#[test]
fn concurrent_parsing_of_different_specs_should_not_corrupt_results() {
    let _g = common::setup(common::V3_MODULE);
    let spec_a = make_spec("SpecA");
    let spec_b = make_spec("SpecB");
    let thread_count = 8;
    let iterations = 200;
    let next = AtomicUsize::new(0);
    std::thread::scope(|s| {
        let handles: Vec<_> = (0..thread_count)
            .map(|_| {
                s.spawn(|| {
                    common::register_v3_test_services();
                    let mut done = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::SeqCst);
                        if i >= iterations {
                            break;
                        }
                        let (spec, expected_title) = if i.is_multiple_of(2) { (&spec_a, "SpecA") } else { (&spec_b, "SpecB") };
                        let actual = parse_and_get_title(spec);
                        assert_eq!(actual.as_deref(), Some(expected_title), "Parsed title should match input spec (concurrent race detected)");
                        done.push(actual);
                    }
                    done
                })
            })
            .collect();
        let total: usize = handles.into_iter().map(|h| h.join().expect("parse thread").len()).sum();
        assert_eq!(total, iterations);
    });
}
