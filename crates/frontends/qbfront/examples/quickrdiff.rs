//! The typed trees the grammar-table parser and the checked front give for
//! QuickrBASIC sources, spans left out.
//!
//!     quickrdiff FILE...
use qbfront::{generated_parser, Dialect};

fn strip(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find("Span {") {
        out.push_str(&rest[..at]);
        let close = rest[at..].find('}').map_or(rest.len(), |end| at + end + 1);
        out.push('_');
        rest = &rest[close..];
    }
    out.push_str(rest);
    out
}

fn main() {
    let (mut same, mut different, mut skipped) = (0, 0, 0);
    for path in std::env::args().skip(1) {
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let Ok(old) = generated_parser::parse(&text, Dialect::Quickr) else {
            skipped += 1;
            continue;
        };
        match qbfront::front::ast::parse(&text, Dialect::Quickr) {
            Ok(new) => {
                let (a, b) = (strip(&format!("{old:#?}")), strip(&format!("{new:#?}")));
                if a == b {
                    same += 1;
                } else {
                    different += 1;
                    let at = a.lines().zip(b.lines()).position(|(x, y)| x != y).unwrap_or(0);
                    println!("{path}: trees differ at line {at}");
                    if std::env::var_os("SHOW").is_some() {
                        for (label, tree) in [("old", &a), ("new", &b)] {
                            let from = at.saturating_sub(3);
                            println!("  {label}: {}", tree.lines().skip(from).take(8).collect::<Vec<_>>().join(" | "));
                        }
                    }
                }
            }
            Err(error) => {
                different += 1;
                println!("{path}: new front refuses: {}:{} {}", error.span.line, error.span.start, error.code);
            }
        }
    }
    eprintln!("same {same}, different {different}, the old parser refuses {skipped}");
}
