# swagger-parser-rs

<!-- languages:begin -->
**English** · [Русский](README.ru.md) · [中文](README.zh-CN.md) · [Español](README.es.md)
<!-- languages:end -->

An unofficial Rust port of [swagger-parser](https://github.com/swagger-api/swagger-parser) 2.1.48 (Apache-2.0):
OpenAPI 3.x / Swagger 2.0 parsing, reference resolution and conversion, with the swagger-core model and its JSON
output reproduced byte for byte. SWAGGER is a trademark of SmartBear Software and OpenAPI is a trademark of the Linux
Foundation; this project is not affiliated with or endorsed by either. The package name `swagger-parser-rs` only says
"swagger-parser, for Rust". [`NOTICE`](NOTICE) names what is ported and under which licence.

It was written for [mock-server](https://github.com/oxidestack/mock-server), a Rust MockServer that must answer
exactly as the Java original does, and is useful to anyone who needs the Java library's exact results.

Status: pre-release. The crate is not published on crates.io yet; until it is, it is used from a checkout of this
repository.

## Install

The crate is `swagger-parser-rs` and is used in code as `swagger_parser`. It is not published on crates.io yet; once
it is:

```toml
[dependencies]
swagger-parser-rs = "0.1"           # not published yet
```

Until then, depend on a checkout: `swagger-parser-rs = { path = "../swagger-parser-rs" }`, with
[java-compat-rs](https://github.com/oxidestack/java-compat-rs) checked out beside it.

The repository is a Cargo workspace. Besides the root crate it holds four members, each usable on its own:
`jackson-rs` (the parts of Jackson that swagger-parser relies on), `snakeyaml-rs` (SnakeYAML 2.6),
`java-rt-rs` (Java exceptions as values, `java.net.URI`/`URL`) and `swagger-parser-safe-url-resolver-rs`.

`upstream/` is the original source, a git submodule pinned to the ported release and read-only.

## Quick start

```rust
use swagger_parser::core::{ParseOptions, SwaggerParserExtension};
use swagger_parser::parser::OpenAPIV3Parser;

let spec = r#"
openapi: 3.0.1
info: {title: Pets, version: "1.0"}
paths:
  /pets:
    get:
      responses:
        "200": {description: ok}
"#;
let options = ParseOptions { resolve: true, ..Default::default() };
let result = OpenAPIV3Parser::new().read_contents(spec, None, Some(&options));
let openapi = result.openapi.expect("a valid document");
let json = swagger_parser::models::json_string(&openapi).expect("serialisable");
assert!(json.starts_with(r#"{"openapi":"3.0.1","info":{"title":"Pets","version":"1.0"}"#));
```

The API reference will be on docs.rs once the crate is published; `cargo doc --open` builds it from a checkout.

## Guides

[Upgrading](docs/guide/upgrading.md): what each release asks of code that uses the crate. This README and the guides
are also a site, `https://oxidestack.github.io/swagger-parser-rs/`, once the repository is public.

## Build and test

```sh
git submodule update --init upstream   # the pinned Java sources some tests read
cargo test --workspace
make lint           # clippy, warnings are errors
make deny           # licences, advisories and sources (cargo install cargo-deny --locked)
make doc            # the API documentation, rustdoc's warnings are errors
make public-check   # the publication gate
make site           # the documentation site (site/README.md), in a pinned node container
```

The crate depends on [java-compat-rs](https://github.com/oxidestack/java-compat-rs), the port of JDK behaviour. See
[`CONTRIBUTING.md`](CONTRIBUTING.md) to contribute.

## Entry points

MockServer's `OpenAPIParser.buildOpenAPI`, for example, maps to:

```rust
use swagger_parser::core::ParseOptions;
use swagger_parser::models::json_string; // Json.mapper().writeValueAsString

let options = ParseOptions {
    resolve: true, resolve_fully: true, resolve_combinators: true,
    skip_matches: true, allow_empty_strings: true, camel_case_flatten_naming: true,
    ..Default::default()
};
for ext in swagger_parser::parser_extensions() {      // OpenAPIV3Parser.getExtensions() with the full distribution
    let result = ext.read_location(spec_url_or_path, None, Some(&options)); // or read_contents(text, …)
    if let Some(openapi) = &result.openapi { /* … */ }
}
```

- `parser_extensions()` returns `OpenAPIV3Parser` then `SwaggerConverter` (Swagger 2.0 → 3.0, with the
  Swagger 1.x compat parser behind it). `parser::OpenAPIV3Parser::get_extensions()` is the v3 module alone.
- `SwaggerParserExtension::try_read_location` / `try_read_contents` return `Err` where the Java method lets an
  exception escape (the plain methods turn it into a one-message result).
- `OpenAPIParser` (the swagger-parser module class) and the `swagger-parser` CLI binary are also available.

## Not ported / known differences

- Nothing MockServer reaches is missing. The upstream tests are ported; the few whose expectation asserts JDK-specific
  behaviour were adapted in the Rust test.
- A `SwaggerConverter` value keeps no state between reads (a reused Java instance carries global
  consumes/produces, components and v2 parameters over to the next read).
- The CLI prints argparse4j-style usage errors but does not reproduce argparse4j's justified text wrapping.
- An OAuth2 scope with a `null` description (Swagger 2.0 → 3.0) is dropped. The v3 `Scopes` map holds strings,
  and Jackson omits null map values when writing anyway.

## Getting help

A question about how the crate works can go to [DeepWiki](https://deepwiki.com/oxidestack/swagger-parser-rs) first:
it answers from this repository's code and documentation, and where it disagrees with the documentation, the
documentation is right. Questions it does not settle go to
[Discussions](https://github.com/oxidestack/swagger-parser-rs/discussions), bugs to
[issues](https://github.com/oxidestack/swagger-parser-rs/issues). <!-- these three links need the public repository; it is not published yet -->

## Licence

The code is licensed under the [Apache License, Version 2.0](LICENSE) (`Apache-2.0`); what it ports is attributed in
[`NOTICE`](NOTICE). The documentation — this README and the guides under `docs/` — is licensed under the
[Creative Commons Attribution 4.0 International](https://creativecommons.org/licenses/by/4.0/) licence
(`CC-BY-4.0`); its legal text is in [`LICENSE-docs`](LICENSE-docs).
