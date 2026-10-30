//! The whole front end's verdict on each file under a dialect: parse, then the
//! semantic pass.
//!
//!     verdict DIALECT FILE...        (qb45 | pds71 | vbdos; PARSER=old for the
//! table-driven parser)
//!
//! One line per file: `path<TAB>class<TAB>message`, the class one of ok,
//! syntax, semantic, unsupported.
use std::path::{Path, PathBuf};

use qbfront::Dialect;
use qbfront::semantic::{Options, compile_with_options};

fn main() {
    let mut args = std::env::args().skip(1);
    let dialect = match args.next().as_deref() {
        Some("qb45") => Dialect::QuickBasic45,
        Some("pds71") => Dialect::Pds71,
        Some("vbdos") => Dialect::VbDos,
        other => panic!("dialect {other:?}"),
    };
    let old = std::env::var("PARSER").is_ok_and(|v| v == "old");
    for path in args {
        let path = PathBuf::from(path);
        let dirs: Vec<PathBuf> = vec![path.parent().map(Path::to_path_buf).unwrap_or_default()];
        let (class, message) = match qbfront::source::load(&path, &dirs) {
            Err(error) => ("syntax", format!("include: {error}")),
            Ok(text) => {
                let text = lalrspike::dos_source(text.as_bytes());
                let parsed = if old {
                    qbfront::front::parse_coded(&text, dialect)
                        .map_err(|e| format!("{}:{} {}", e.span.line, e.span.start, e.message))
                } else {
                    lalrspike::qb_ast::parse(&text, dialect)
                        .map_err(|e| format!("{}:{} {}", e.span.line, e.span.start, e.message))
                };
                match parsed {
                    Err(message) => ("syntax", message),
                    Ok(module) => match compile_with_options(&module, "t", dialect, "llrm", &Options::default()) {
                        Ok(_) => ("ok", String::new()),
                        Err(error)
                            if error.message.contains("cannot generate code")
                                || error.message.contains("unsupported") =>
                        {
                            ("unsupported", error.message)
                        }
                        Err(error) => ("semantic", error.message),
                    },
                }
            }
        };
        println!("{}\t{class}\t{}", path.display(), message.replace('\n', " "));
    }
}
