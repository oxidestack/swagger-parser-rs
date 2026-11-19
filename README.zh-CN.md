# swagger-parser-rs

<!-- languages:begin -->
[English](README.md) · [Русский](README.ru.md) · **中文** · [Español](README.es.md)
<!-- languages:end -->
<!-- translation of README.md: d7514f499878 -->

[swagger-parser](https://github.com/swagger-api/swagger-parser) 2.1.48（Apache-2.0）的非官方 Rust 移植：OpenAPI 3.x /
Swagger 2.0 的解析、引用解析与转换，逐字节重现 swagger-core 模型及其 JSON 输出。SWAGGER 是 SmartBear Software 的商标，
OpenAPI 是 Linux Foundation 的商标；本项目与二者均无关联，也未获其认可。包名 `swagger-parser-rs` 仅表示
"用于 Rust 的 swagger-parser"。[`NOTICE`](NOTICE) 列出了所移植的内容及其许可证。

它是为 [mock-server](https://github.com/oxidestack/mock-server) 而写的。mock-server 是用 Rust 实现的 MockServer，
必须与 Java 原版作出完全相同的应答；任何需要与该 Java 库完全一致结果的人都可以使用它。

状态：尚未发布。本 crate 还没有发布到 crates.io；在此之前，请从本仓库的本地检出使用它。

## 安装

crate 名为 `swagger-parser-rs`，在代码中以 `swagger_parser` 使用。它尚未发布到 crates.io；发布之后：

```toml
[dependencies]
swagger-parser-rs = "0.1"           # not published yet
```

在此之前，请依赖本地检出：`swagger-parser-rs = { path = "../swagger-parser-rs" }`，并在其旁边检出
[java-compat-rs](https://github.com/oxidestack/java-compat-rs)。

本仓库是一个 Cargo 工作区。除根 crate 外还有四个成员，每个都可单独使用：`jackson-rs`（swagger-parser 所依赖的
Jackson 部分）、`snakeyaml-rs`（SnakeYAML 2.6）、`java-rt-rs`（作为值的 Java 异常、`java.net.URI`/`URL`）以及
`swagger-parser-safe-url-resolver-rs`。

`upstream/` 是原始源码，一个固定在所移植版本上的只读 git 子模块。

## 快速开始

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

crate 发布后，API 参考将位于 docs.rs；在本地检出中可用 `cargo doc --open` 生成。

## 指南

[升级](docs/i18n/zh/guide/upgrading.md)：每个版本对使用本 crate 的代码有哪些要求。仓库公开后，本 README 和这些指南也会以
站点形式提供：`https://oxidestack.github.io/swagger-parser-rs/`。

## 构建与测试

```sh
git submodule update --init upstream   # the pinned Java sources some tests read
cargo test --workspace
make lint           # clippy, warnings are errors
make deny           # licences, advisories and sources (cargo install cargo-deny --locked)
make doc            # the API documentation, rustdoc's warnings are errors
make public-check   # the publication gate
make site           # the documentation site (site/README.md), in a pinned node container
```

本 crate 依赖 [java-compat-rs](https://github.com/oxidestack/java-compat-rs)，即 JDK 行为的移植。如何参与贡献，见
[`CONTRIBUTING.md`](CONTRIBUTING.md)。

## 入口

例如，MockServer 的 `OpenAPIParser.buildOpenAPI` 对应于：

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

- `parser_extensions()` 先返回 `OpenAPIV3Parser`，再返回 `SwaggerConverter`（Swagger 2.0 → 3.0，其后是 Swagger 1.x
  兼容解析器）。`parser::OpenAPIV3Parser::get_extensions()` 只包含 v3 模块。
- 在 Java 方法会让异常逸出的地方，`SwaggerParserExtension::try_read_location` / `try_read_contents` 返回 `Err`
  （普通方法则把它变成只含一条消息的结果）。
- 还提供 `OpenAPIParser`（swagger-parser 模块的类）和 `swagger-parser` 命令行程序。

## 未移植的部分 / 已知差异

- MockServer 用到的部分没有任何缺失。upstream 的测试均已移植；少数断言 JDK 特有行为的测试在 Rust 测试中做了调整。
- `SwaggerConverter` 值在两次读取之间不保留状态（Java 中被重用的实例会把全局 consumes/produces、components 和 v2
  参数带到下一次读取）。
- 命令行程序以 argparse4j 的风格打印用法错误，但不重现 argparse4j 的两端对齐换行。
- description 为 `null` 的 OAuth2 scope（Swagger 2.0 → 3.0）会被丢弃。v3 的 `Scopes` map 存放字符串，而且 Jackson
  写出时本来就会省略 map 中的 null 值。

## 获取帮助

关于本 crate 如何工作的问题，可以先问 [DeepWiki](https://deepwiki.com/oxidestack/swagger-parser-rs)：它根据本仓库的
代码和文档作答；若它与文档不一致，以文档为准。它解决不了的问题请发到
[Discussions](https://github.com/oxidestack/swagger-parser-rs/discussions)，缺陷请提交到
[issues](https://github.com/oxidestack/swagger-parser-rs/issues)。 <!-- these three links need the public repository; it is not published yet -->

## 许可证

代码采用 [Apache License, Version 2.0](LICENSE)（`Apache-2.0`）许可；所移植的内容在 [`NOTICE`](NOTICE) 中注明出处。
文档（本 README 及 `docs/` 下的指南）采用
[Creative Commons Attribution 4.0 International](https://creativecommons.org/licenses/by/4.0/)（`CC-BY-4.0`）许可；
其法律文本见 [`LICENSE-docs`](LICENSE-docs)。
