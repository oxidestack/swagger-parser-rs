//! The swagger-parser-cli command line tool (`io.swagger.v3.parser.SwaggerParser.main`).

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(swagger_parser::cli::main(&args));
}
