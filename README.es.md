# swagger-parser-rs

<!-- languages:begin -->
[English](README.md) · [Русский](README.ru.md) · [中文](README.zh-CN.md) · **Español**
<!-- languages:end -->
<!-- translation of README.md: d7514f499878 -->

Un port no oficial a Rust de [swagger-parser](https://github.com/swagger-api/swagger-parser) 2.1.48 (Apache-2.0):
análisis de OpenAPI 3.x / Swagger 2.0, resolución de referencias y conversión, con el modelo de swagger-core y su
salida JSON reproducidos byte a byte. SWAGGER es una marca de SmartBear Software y OpenAPI es una marca de la Linux
Foundation; este proyecto no está afiliado a ninguna de ellas ni cuenta con su aval. El nombre del paquete
`swagger-parser-rs` solo dice «swagger-parser, para Rust». [`NOTICE`](NOTICE) indica qué se porta y bajo qué licencia.

Se escribió para [mock-server](https://github.com/oxidestack/mock-server), un MockServer en Rust que debe responder
exactamente como el original en Java, y es útil para cualquiera que necesite los resultados exactos de la biblioteca de
Java.

Estado: previo a la primera versión. El crate aún no está publicado en crates.io; hasta entonces se usa desde una copia
local de este repositorio.

## Instalación

El crate se llama `swagger-parser-rs` y en el código se usa como `swagger_parser`. Aún no está publicado en
crates.io; cuando lo esté:

```toml
[dependencies]
swagger-parser-rs = "0.1"           # not published yet
```

Hasta entonces, depende de una copia local: `swagger-parser-rs = { path = "../swagger-parser-rs" }`, con
[java-compat-rs](https://github.com/oxidestack/java-compat-rs) clonado a su lado.

El repositorio es un espacio de trabajo de Cargo. Además del crate raíz contiene cuatro miembros, cada uno utilizable
por separado: `jackson-rs` (las partes de Jackson en las que se apoya swagger-parser), `snakeyaml-rs` (SnakeYAML 2.6),
`java-rt-rs` (excepciones de Java como valores, `java.net.URI`/`URL`) y `swagger-parser-safe-url-resolver-rs`.

`upstream/` es el código fuente original, un submódulo de git fijado en la versión portada y de solo lectura.

## Inicio rápido

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

La referencia de la API estará en docs.rs cuando el crate se publique; desde una copia local la genera
`cargo doc --open`.

## Guías

[Actualización](docs/i18n/es/guide/upgrading.md): lo que cada versión pide al código que usa el crate. Este README y
las guías son también un sitio, `https://oxidestack.github.io/swagger-parser-rs/`, cuando el repositorio sea público.

## Compilar y probar

```sh
git submodule update --init upstream   # the pinned Java sources some tests read
cargo test --workspace
make lint           # clippy, warnings are errors
make deny           # licences, advisories and sources (cargo install cargo-deny --locked)
make doc            # the API documentation, rustdoc's warnings are errors
make public-check   # the publication gate
make site           # the documentation site (site/README.md), in a pinned node container
```

El crate depende de [java-compat-rs](https://github.com/oxidestack/java-compat-rs), el port del comportamiento del
JDK. Para contribuir, consulta [`CONTRIBUTING.md`](CONTRIBUTING.md).

## Puntos de entrada

`OpenAPIParser.buildOpenAPI` de MockServer, por ejemplo, corresponde a:

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

- `parser_extensions()` devuelve `OpenAPIV3Parser` y después `SwaggerConverter` (Swagger 2.0 → 3.0, con el parser
  de compatibilidad de Swagger 1.x detrás). `parser::OpenAPIV3Parser::get_extensions()` es solo el módulo v3.
- `SwaggerParserExtension::try_read_location` / `try_read_contents` devuelven `Err` donde el método de Java deja
  escapar una excepción (los métodos simples la convierten en un resultado con un único mensaje).
- También están disponibles `OpenAPIParser` (la clase del módulo swagger-parser) y el binario de línea de comandos
  `swagger-parser`.

## No portado / diferencias conocidas

- No falta nada de lo que alcanza MockServer. Las pruebas de upstream están portadas; las pocas cuya expectativa
  comprueba un comportamiento propio del JDK se adaptaron en la prueba de Rust.
- Un valor `SwaggerConverter` no guarda estado entre lecturas (una instancia de Java reutilizada arrastra los
  consumes/produces globales, los components y los parámetros v2 a la siguiente lectura).
- La línea de comandos imprime los errores de uso al estilo de argparse4j, pero no reproduce el ajuste de texto
  justificado de argparse4j.
- Un scope de OAuth2 con descripción `null` (Swagger 2.0 → 3.0) se descarta. El mapa `Scopes` de v3 contiene
  cadenas, y Jackson omite de todos modos los valores nulos de un mapa al escribirlo.

## Obtener ayuda

Una pregunta sobre cómo funciona el crate puede ir primero a
[DeepWiki](https://deepwiki.com/oxidestack/swagger-parser-rs): responde a partir del código y la documentación de este
repositorio, y donde no coincide con la documentación, la documentación tiene razón. Las preguntas que no resuelva van
a [Discussions](https://github.com/oxidestack/swagger-parser-rs/discussions), y los errores a
[issues](https://github.com/oxidestack/swagger-parser-rs/issues). <!-- these three links need the public repository; it is not published yet -->

## Licencia

El código se distribuye bajo la [Apache License, Version 2.0](LICENSE) (`Apache-2.0`); lo que porta se atribuye en
[`NOTICE`](NOTICE). La documentación —este README y las guías de `docs/`— se distribuye bajo la licencia
[Creative Commons Attribution 4.0 International](https://creativecommons.org/licenses/by/4.0/) (`CC-BY-4.0`); su texto
legal está en [`LICENSE-docs`](LICENSE-docs).
