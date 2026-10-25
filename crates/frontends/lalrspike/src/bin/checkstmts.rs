//! The front end's verdict on each catalog statement, in the prologue's module.
//!
//!     checkstmts DIALECT PROLOGUE EPILOGUE CATALOG.tsv     (id<TAB>statement
//! per line)
//!
//! Prints `id<TAB>class<TAB>message`; class is ok, syntax, semantic or
//! unsupported.
use qbfront::Dialect;
use qbfront::semantic::{Options, compile_with_options};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dialect = match args[0].as_str() {
        "qb45" => Dialect::QuickBasic45,
        "pds71" => Dialect::Pds71,
        _ => Dialect::VbDos,
    };
    let legacy = std::env::var("LEGACY").is_ok();
    let old = std::env::var("PARSER").is_ok_and(|v| v == "old");
    let prologue = std::fs::read_to_string(&args[1]).unwrap();
    let epilogue = std::fs::read_to_string(&args[2]).unwrap();
    let catalog = std::fs::read_to_string(&args[3]).unwrap();
    for line in catalog.lines() {
        let (id, text) = line.split_once('\t').unwrap();
        let source = format!("{prologue}{text}\n{epilogue}").replace('\n', "\r\n");
        let parsed = if old {
            qbfront::front::parse_coded(&source, dialect)
                .map_err(|e| format!("{}:{} {}", e.span.line, e.span.start, e.message))
        } else {
            lalrspike::qb_ast::parse(&source, dialect)
                .map_err(|e| format!("{}:{} {}", e.span.line, e.span.start, e.message))
        };
        let (class, message) = match parsed {
            Err(message) => ("syntax", message),
            Ok(module) if !legacy => {
                let diagnostics = lalrspike::qb_check::check(&module, dialect);
                match diagnostics.first() {
                    None => ("ok", String::new()),
                    Some(first) => ("semantic", format!("{}:{} {}", first.span.line, first.span.start, first.message)),
                }
            }
            Ok(module) => match compile_with_options(&module, "t", dialect, "llrm", &Options::default()) {
                Ok(_) => ("ok", String::new()),
                Err(error)
                    if error.message.contains("cannot generate code")
                        || error.message.contains("no source signature")
                        || error.message.contains("audited") =>
                {
                    ("unsupported", error.message)
                }
                Err(error) => ("semantic", error.message),
            },
        };
        println!("{id}\t{class}\t{}", message.replace('\n', " "));
    }
}
