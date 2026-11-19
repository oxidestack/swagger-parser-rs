# swagger-parser-rs

<!-- languages:begin -->
[English](README.md) · **Русский** · [中文](README.zh-CN.md) · [Español](README.es.md)
<!-- languages:end -->
<!-- translation of README.md: d7514f499878 -->

Неофициальный порт [swagger-parser](https://github.com/swagger-api/swagger-parser) 2.1.48 (Apache-2.0) на Rust:
разбор OpenAPI 3.x / Swagger 2.0, разрешение ссылок и конвертация, с моделью swagger-core и её JSON-выводом,
воспроизведёнными байт в байт. SWAGGER — товарный знак SmartBear Software, OpenAPI — товарный знак Linux Foundation;
этот проект не связан ни с одной из них и не одобрен ими. Имя пакета `swagger-parser-rs` означает лишь
«swagger-parser для Rust». [`NOTICE`](NOTICE) называет, что портировано и под какой лицензией.

Он написан для [mock-server](https://github.com/oxidestack/mock-server) — MockServer на Rust, который должен отвечать
в точности как оригинал на Java, — и полезен всем, кому нужны в точности те же результаты, что у библиотеки на Java.

Статус: до первого выпуска. Крейт ещё не опубликован на crates.io; пока этого не произошло, его используют из
локальной копии этого репозитория.

## Установка

Крейт называется `swagger-parser-rs`, а в коде используется как `swagger_parser`. На crates.io он ещё не
опубликован; после публикации:

```toml
[dependencies]
swagger-parser-rs = "0.1"           # not published yet
```

До тех пор зависьте от локальной копии: `swagger-parser-rs = { path = "../swagger-parser-rs" }`, а рядом положите
копию [java-compat-rs](https://github.com/oxidestack/java-compat-rs).

Репозиторий — рабочее пространство Cargo. Кроме корневого крейта в нём четыре участника, каждый из которых можно
использовать сам по себе: `jackson-rs` (те части Jackson, на которые опирается swagger-parser), `snakeyaml-rs`
(SnakeYAML 2.6), `java-rt-rs` (исключения Java как значения, `java.net.URI`/`URL`) и
`swagger-parser-safe-url-resolver-rs`.

`upstream/` — исходный код оригинала, git-подмодуль, закреплённый на портированном выпуске, только для чтения.

## Быстрый старт

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

Справочник API появится на docs.rs после публикации крейта; из локальной копии его собирает `cargo doc --open`.

## Руководства

[Обновление](docs/i18n/ru/guide/upgrading.md): что каждый выпуск требует от кода, использующего крейт. Этот README и
руководства доступны также в виде сайта, `https://oxidestack.github.io/swagger-parser-rs/`, после того как репозиторий
станет публичным.

## Сборка и тесты

```sh
git submodule update --init upstream   # the pinned Java sources some tests read
cargo test --workspace
make lint           # clippy, warnings are errors
make deny           # licences, advisories and sources (cargo install cargo-deny --locked)
make doc            # the API documentation, rustdoc's warnings are errors
make public-check   # the publication gate
make site           # the documentation site (site/README.md), in a pinned node container
```

Крейт зависит от [java-compat-rs](https://github.com/oxidestack/java-compat-rs), порта поведения JDK. Как внести
вклад, описано в [`CONTRIBUTING.md`](CONTRIBUTING.md).

## Точки входа

`OpenAPIParser.buildOpenAPI` из MockServer, например, соответствует такому коду:

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

- `parser_extensions()` возвращает `OpenAPIV3Parser`, а затем `SwaggerConverter` (Swagger 2.0 → 3.0, с парсером
  совместимости Swagger 1.x за ним). `parser::OpenAPIV3Parser::get_extensions()` — только модуль v3.
- `SwaggerParserExtension::try_read_location` / `try_read_contents` возвращают `Err` там, где метод Java выпускает
  исключение наружу (обычные методы превращают его в результат с одним сообщением).
- Доступны также `OpenAPIParser` (класс модуля swagger-parser) и бинарник CLI `swagger-parser`.

## Не портировано / известные отличия

- Ничего из того, до чего доходит MockServer, не отсутствует. Тесты upstream портированы; несколько тестов, чьи
  ожидания проверяют поведение, специфичное для JDK, адаптированы в тесте на Rust.
- Значение `SwaggerConverter` не хранит состояния между чтениями (повторно используемый экземпляр в Java переносит
  глобальные consumes/produces, components и параметры v2 в следующее чтение).
- CLI печатает ошибки использования в стиле argparse4j, но не воспроизводит выровненный перенос текста argparse4j.
- Scope OAuth2 с описанием `null` (Swagger 2.0 → 3.0) отбрасывается. Map `Scopes` в v3 хранит строки, а Jackson всё
  равно опускает null-значения map при записи.

## Помощь

Вопрос о том, как работает крейт, можно сначала задать [DeepWiki](https://deepwiki.com/oxidestack/swagger-parser-rs):
она отвечает по коду и документации этого репозитория, а там, где она расходится с документацией, права
документация. Вопросы, которые она не решает, — в
[Discussions](https://github.com/oxidestack/swagger-parser-rs/discussions), ошибки — в
[issues](https://github.com/oxidestack/swagger-parser-rs/issues). <!-- these three links need the public repository; it is not published yet -->

## Лицензия

Код распространяется по лицензии [Apache License, Version 2.0](LICENSE) (`Apache-2.0`); то, что он портирует,
указано в [`NOTICE`](NOTICE). Документация — этот README и руководства в `docs/` — распространяется по лицензии
[Creative Commons Attribution 4.0 International](https://creativecommons.org/licenses/by/4.0/) (`CC-BY-4.0`); её
юридический текст — в [`LICENSE-docs`](LICENSE-docs).
