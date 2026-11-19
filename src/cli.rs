//! swagger-parser-cli 2.1.48: `io.swagger.v3.parser.SwaggerParser`, a command line front end that parses a
//! location with `OpenAPIV3Parser` and writes the result (YAML or JSON) and the parse messages to files.
//! The binary is `src/bin/swagger-parser.rs`.

use crate::core::ParseOptions;
use crate::models::{json_pretty, yaml_pretty};
use crate::parser::OpenAPIV3Parser;
use std::collections::HashMap;

/// Destination of `-resolve`: resolve remote and local references.
pub const RESOLVE: &str = "resolve";
/// Destination of `-resolveFully`: resolve references fully.
pub const RESOLVEFULLY: &str = "resolvefully";
/// Destination of `-flatten`: move inline models into components.
pub const FLATTEN: &str = "flatten";
/// Destination of `-json`: write the output as JSON.
pub const JSON: &str = "json";
/// Destination of `-yaml`: write the output as YAML.
pub const YAML: &str = "yaml";
/// Destination of `-l`: the file the parse messages are written to.
pub const LOG_ERRORS: &str = "l";
/// Destination of `-o`: the file the parsed document is written to.
pub const OUTPUT_FILE: &str = "o";
/// The value a store-true flag holds when given.
pub const TRUE: &str = "true";
/// Destination of `-i`: the location to parse.
pub const INPUT_FILE: &str = "i";

/// argparse4j's `Namespace`: destination → value (`getString` is the value's `toString()`)
#[derive(Clone, Debug, Default)]
pub struct Namespace(pub HashMap<String, String>);

impl Namespace {
    /// A namespace holding the given (destination, value) pairs.
    pub fn new(args: &[(&str, &str)]) -> Namespace {
        Namespace(args.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect())
    }

    /// `getString(dest)`: the value of the destination, `None` when absent.
    pub fn get_string(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }
}

/// The options `main` declares: flag, destination, whether it stores `true` (otherwise it takes a value),
/// and its help text.
const ARGUMENTS: [(&str, &str, bool, &str); 8] = [
    ("-i", INPUT_FILE, false, "input file to be parsed"),
    ("-resolve", RESOLVE, true, "resolve remote or local references"),
    ("-resolveFully", RESOLVEFULLY, true, ""),
    ("-flatten", FLATTEN, true, ""),
    ("-o", OUTPUT_FILE, false, "output file parsed"),
    ("-l", LOG_ERRORS, false, "output error logs"),
    ("-json", JSON, true, "generate file as JSON"),
    ("-yaml", YAML, true, "generate file as YAML"),
];

const USAGE: &str = "usage: swagger-parser [-h] -i I [-resolve] [-resolveFully] [-flatten]\n                      [-o O] [-l L] [-json] [-yaml]";

/// Why argument parsing stopped: a usage error (exit status 1) or the help screen (exit status 0).
#[derive(Debug, PartialEq, Eq)]
pub enum ArgError {
    /// A usage error with its message (exit status 1).
    Usage(String),
    /// `-h`/`--help` was given (exit status 0).
    Help,
}

fn help_text() -> String {
    let mut s = format!("{USAGE}\n\nnamed arguments:\n  -h, --help             show this help message and exit\n");
    for (flag, dest, store_true, help) in ARGUMENTS {
        let name = if store_true { flag.to_string() } else { format!("{flag} {}", dest.to_uppercase()) };
        let help = if store_true { if help.is_empty() { "(default: false)".to_string() } else { format!("{help} (default: false)") } } else { help.to_string() };
        s.push_str(&format!("  {name:<21}  {help}\n"));
    }
    s
}

/// argparse4j `parseArgs` for the options above: `-o value`, `-o=value`, `-ovalue` for single-letter
/// options, and unambiguous prefixes of the long single-dash flags.
pub fn parse_args(args: &[String]) -> Result<Namespace, ArgError> {
    let mut ns: HashMap<String, String> = HashMap::new();
    for (_, dest, store_true, _) in ARGUMENTS {
        ns.insert(dest.to_string(), if store_true { "false".into() } else { "null".into() });
    }
    let mut seen_input = false;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "-h" || a == "--help" {
            return Err(ArgError::Help);
        }
        if !a.starts_with('-') || a == "-" {
            return Err(ArgError::Usage(format!("unrecognized arguments: '{a}'")));
        }
        let (name, inline) = match a.split_once('=') {
            Some((n, v)) => (n.to_string(), Some(v.to_string())),
            None => (a.clone(), None),
        };
        let exact = ARGUMENTS.iter().find(|(f, ..)| *f == name);
        let found = match exact {
            Some(x) => Some((*x, inline)),
            None => {
                let prefixed: Vec<_> = ARGUMENTS.iter().filter(|(f, ..)| f.len() > 2 && f.starts_with(name.as_str())).collect();
                if prefixed.len() > 1 {
                    let names: Vec<&str> = prefixed.iter().map(|(f, ..)| *f).collect();
                    return Err(ArgError::Usage(format!("ambiguous option: {name} could match {}", names.join(", "))));
                }
                match prefixed.first() {
                    Some(x) => Some((**x, inline)),
                    None => {
                        // `-ivalue`: a single-letter option with its value attached
                        let short = ARGUMENTS.iter().find(|(f, _, st, _)| f.len() == 2 && !st && a.starts_with(f));
                        short.map(|x| (*x, Some(a[2..].to_string())))
                    }
                }
            }
        };
        let Some(((flag, dest, store_true, _), inline)) = found else {
            return Err(ArgError::Usage(format!("unrecognized arguments: '{a}'")));
        };
        if store_true {
            if let Some(inline) = inline {
                return Err(ArgError::Usage(format!("argument {flag}: ignored explicit argument '{inline}'")));
            }
            ns.insert(dest.to_string(), TRUE.into());
        } else {
            let value = match inline {
                Some(v) => v,
                None => {
                    i += 1;
                    match args.get(i) {
                        Some(v) if !v.starts_with('-') => v.clone(),
                        _ => return Err(ArgError::Usage(format!("argument {flag}: expected one argument"))),
                    }
                }
            };
            if dest == INPUT_FILE {
                seen_input = true;
            }
            ns.insert(dest.to_string(), value);
        }
        i += 1;
    }
    if !seen_input {
        return Err(ArgError::Usage("argument -i is required".into()));
    }
    // unset value options are null in the namespace
    ns.retain(|_, v| v != "null");
    Ok(Namespace(ns))
}

/// `SwaggerParser.main(args)`; returns the process exit status (argparse4j errors print the usage and
/// exit with 1).
pub fn main(args: &[String]) -> i32 {
    if args.is_empty() {
        return 0;
    }
    match parse_args(args) {
        Ok(ns) => {
            read_from_location(&ns);
            0
        }
        Err(ArgError::Help) => {
            print!("{}", help_text());
            0
        }
        Err(ArgError::Usage(msg)) => {
            eprintln!("{USAGE}\nswagger-parser: error: {msg}");
            1
        }
    }
}

fn generate_messages_file(messages: Option<&Vec<String>>, args: &Namespace) {
    if let Some(m) = messages && !m.is_empty() && args.get_string(LOG_ERRORS).is_some() {
        generate_parsed_file(args, LOG_ERRORS, Some(&format!("[{}]", m.join(", "))));
    }
}

/// `readFromLocation(args)`: the parse messages (`None` where Java returns null: no model and no message
/// list).
pub fn read_from_location(args: &Namespace) -> Option<Vec<String>> {
    let mut messages: Option<Vec<String>> = Some(Vec::new());
    let options = set_options(args);
    let Some(input) = args.get_string(INPUT_FILE) else {
        // `readLocation(null, …)` fails inside the parser; the exception is printed and swallowed
        eprintln!("java.lang.NullPointerException");
        return messages;
    };
    let result = OpenAPIV3Parser::new().read_location(input, None, Some(&options));
    if args.get_string(OUTPUT_FILE).is_some() && let Some(openapi) = &result.openapi {
        let output = if args.get_string(JSON) == Some(TRUE) { json_pretty(openapi) } else { yaml_pretty(openapi) };
        generate_parsed_file(args, OUTPUT_FILE, output.as_deref());
    }
    match (&result.openapi, &result.messages) {
        (None, m) => {
            messages = m.clone();
            generate_messages_file(m.as_ref(), args);
        }
        (Some(_), Some(m)) if !m.is_empty() => {
            messages = Some(m.clone());
            generate_messages_file(Some(m), args);
        }
        (Some(_), None) => {
            // `result.getMessages().isEmpty()` on null: a NullPointerException, printed and swallowed
            eprintln!("java.lang.NullPointerException");
        }
        _ => {}
    }
    messages
}

fn generate_parsed_file(args: &Namespace, key: &str, result: Option<&str>) {
    let (Some(result), Some(path)) = (result, args.get_string(key)) else { return };
    if let Err(e) = std::fs::write(path, result.as_bytes()) {
        eprintln!("java.nio.file.NoSuchFileException: {path}: {e}");
    }
}

fn set_options(args: &Namespace) -> ParseOptions {
    let mut options = ParseOptions::default();
    if args.get_string(RESOLVE) == Some(TRUE) {
        options.resolve = true;
    }
    if args.get_string(RESOLVEFULLY) == Some(TRUE) {
        options.resolve_fully = true;
    }
    if args.get_string(FLATTEN) == Some(TRUE) {
        options.flatten = true;
    }
    options
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Namespace, ArgError> {
        parse_args(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    /// Expectations printed by argparse4j 0.9.0 for the same parser definition.
    #[test]
    fn argparse4j_behaviour() {
        let n = parse(&["-i=x.yaml", "-resolve", "-resolveFully", "-yaml", "-o=out.yaml"]).unwrap();
        assert_eq!((n.get_string("i"), n.get_string("resolve"), n.get_string("resolvefully"), n.get_string("yaml"), n.get_string("o")), (Some("x.yaml"), Some("true"), Some("true"), Some("true"), Some("out.yaml")));
        assert_eq!(n.get_string("flatten"), Some("false"));
        assert_eq!(n.get_string("l"), None);
        let n = parse(&["-i", "x.yaml", "-json"]).unwrap();
        assert_eq!((n.get_string("i"), n.get_string("json"), n.get_string("o")), (Some("x.yaml"), Some("true"), None));
        let n = parse(&["-ix.yaml", "-l", "l.txt"]).unwrap();
        assert_eq!((n.get_string("i"), n.get_string("l")), (Some("x.yaml"), Some("l.txt")));
        assert_eq!(parse(&["-i=x", "-res"]).unwrap_err(), ArgError::Usage("ambiguous option: -res could match -resolve, -resolveFully".into()));
        assert_eq!(parse(&["-o", "y"]).unwrap_err(), ArgError::Usage("argument -i is required".into()));
        assert_eq!(parse(&["-i=x", "-bogus"]).unwrap_err(), ArgError::Usage("unrecognized arguments: '-bogus'".into()));
        assert_eq!(parse(&["-h"]).unwrap_err(), ArgError::Help);
    }
}
