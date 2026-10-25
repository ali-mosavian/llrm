//! The front end's verdict on whole program files, for each family.
//!
//!     checkfiles DIALECT FILE...      prints `path<TAB>class<TAB>message` (ok,
//! syntax or semantic)
use qbfront::Dialect;

fn main() {
    let mut args = std::env::args().skip(1);
    let dialect = match args.next().as_deref() {
        Some("qb45") => Dialect::QuickBasic45,
        Some("pds71") => Dialect::Pds71,
        _ => Dialect::VbDos,
    };
    for path in args {
        // BC inlines `'$INCLUDE:` files, so the front end sees the expanded
        // text
        let source = match qbfront::source::load(std::path::Path::new(&path), &[]) {
            Ok(text) => lalrspike::dos_source(text.as_bytes()),
            // a missing include file is BC's error too
            Err(message) if message.contains("was not found") => {
                println!("{path}\tsyntax\t{message}");
                continue;
            }
            Err(_) => lalrspike::dos_source(&std::fs::read(&path).unwrap()),
        };
        let (class, message) = match lalrspike::qb_ast::parse(&source, dialect) {
            Err(e) => {
                let text = source.lines().nth(e.span.line.saturating_sub(1)).unwrap_or("").trim();
                (
                    "syntax",
                    format!(
                        "{}:{} {}{}",
                        e.span.line,
                        e.span.start,
                        e.message,
                        if std::env::var("SRC").is_ok() { format!("  | {text}") } else { String::new() }
                    ),
                )
            }
            Ok(module) => match lalrspike::qb_check::check(&module, dialect).first() {
                None => ("ok", String::new()),
                Some(d) => {
                    let text = source.lines().nth(d.span.line.saturating_sub(1)).unwrap_or("").trim();
                    (
                        "semantic",
                        format!(
                            "{}:{} {}{}",
                            d.span.line,
                            d.span.start,
                            d.message,
                            if std::env::var("SRC").is_ok() { format!("  | {text}") } else { String::new() }
                        ),
                    )
                }
            },
        };
        println!("{path}\t{class}\t{message}");
    }
}
