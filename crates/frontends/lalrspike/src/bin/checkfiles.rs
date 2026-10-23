//! The front end's verdict on whole program files, for each family.
//!
//!     checkfiles DIALECT FILE...      prints `path<TAB>class<TAB>message` (ok, syntax or semantic)
use qbfront::Dialect;

fn main() {
    let mut args = std::env::args().skip(1);
    let dialect = match args.next().as_deref() {
        Some("qb45") => Dialect::QuickBasic45,
        Some("pds71") => Dialect::Pds71,
        _ => Dialect::VbDos,
    };
    for path in args {
        let source = lalrspike::dos_source(&std::fs::read(&path).unwrap());
        let (class, message) = match lalrspike::qb_ast::parse(&source, dialect) {
            Err(e) => ("syntax", format!("{}:{} {}", e.span.line, e.span.start, e.message)),
            Ok(module) => match lalrspike::qb_check::check(&module, dialect).first() {
                None => ("ok", String::new()),
                Some(d) => ("semantic", format!("{}:{} {}", d.span.line, d.span.start, d.message)),
            },
        };
        println!("{path}\t{class}\t{message}");
    }
}
