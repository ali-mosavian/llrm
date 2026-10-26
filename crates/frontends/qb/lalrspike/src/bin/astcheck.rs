//! The typed parser against the existing one: the same `Module`, spans and all,
//! on every program the old one takes.
//!
//!     astcheck [-v] FILE...
use qbfront::Dialect;

fn strip_spans(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("Span { line: ") {
        out.push_str(&rest[..at]);
        let close = rest[at..].find('}').map_or(rest.len(), |end| at + end + 1);
        out.push_str("_");
        rest = &rest[close..];
    }
    out.push_str(rest);
    let out = out.replace("unchecked: true", "unchecked: false").replace("empty_list: true", "empty_list: false");
    // deliberate differences: CURRENCY is a type of its own here; a NEXT's
    // variable is recorded
    let mut out = out.replace("Some(Currency)", "Some(Named(\"CURRENCY\"))");
    while let Some(at) = out.find("next_variable: Some((") {
        let close = out[at..].find("))").map_or(out.len(), |end| at + end + 2);
        out.replace_range(at..close, "next_variable: None");
    }
    out
}

fn first_difference(
    a: &str,
    b: &str,
) -> String {
    let at = a.bytes().zip(b.bytes()).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
    let from = at.saturating_sub(90);
    let window = |s: &str| s.get(from..(at + 110).min(s.len())).unwrap_or("").replace('\n', " ");
    format!("old: …{}\n      new: …{}", window(a), window(b))
}

fn main() {
    let mut verbose = false;
    let mut compile = false;
    let (mut compiled_same, mut compiled_differ, mut compile_new_fails, mut compile_old_fails) = (0, 0, 0, 0);
    let (mut same, mut spans_differ, mut differ, mut new_refuses, mut old_refuses, mut both_refuse) =
        (0, 0, 0, 0, 0, 0);
    let mut shown = 0;
    let limit: usize = std::env::var("SHOW").ok().and_then(|v| v.parse().ok()).unwrap_or(12);
    for path in std::env::args().skip(1) {
        if path == "-v" {
            verbose = true;
            continue;
        }
        if path == "-c" {
            compile = true;
            continue;
        }
        let source = lalrspike::dos_source(&std::fs::read(&path).unwrap());
        for dialect in [Dialect::QuickBasic45, Dialect::Pds71, Dialect::VbDos] {
            let old = qbfront::parse(&source, dialect);
            let new = lalrspike::qb_ast::parse(&source, dialect);
            match (old, new) {
                (Ok(a), Ok(b)) => {
                    if compile {
                        let options = qbfront::semantic::Options::default();
                        let old = qbfront::semantic::compile_with_options(&a, "t", dialect, "llrm", &options);
                        let new = qbfront::semantic::compile_with_options(&b, "t", dialect, "llrm", &options);
                        match (old.as_ref(), new.as_ref()) {
                            (Ok(x), Ok(y)) if x == y => compiled_same += 1,
                            (Ok(_), Ok(_)) => {
                                compiled_differ += 1;
                                if shown < limit {
                                    shown += 1;
                                    println!("{path} {dialect:?}: compiled output differs");
                                }
                            }
                            (Ok(_), Err(e)) => {
                                compile_new_fails += 1;
                                if shown < limit {
                                    shown += 1;
                                    println!("{path} {dialect:?}: only the old module compiles: {e:?}");
                                }
                            }
                            (Err(_), Ok(_)) => compile_old_fails += 1,
                            (Err(_), Err(_)) => {}
                        }
                    }
                    if a == b {
                        same += 1;
                    } else if strip_spans(&format!("{a:?}")) == strip_spans(&format!("{b:?}")) {
                        spans_differ += 1;
                        if verbose && shown < limit {
                            shown += 1;
                            println!(
                                "{path} {dialect:?}: spans differ\n  {}",
                                first_difference(&format!("{a:?}"), &format!("{b:?}"))
                            );
                        }
                    } else {
                        differ += 1;
                        if shown < limit {
                            shown += 1;
                            println!(
                                "{path} {dialect:?}: differs\n  {}",
                                first_difference(&strip_spans(&format!("{a:?}")), &strip_spans(&format!("{b:?}")))
                            );
                        }
                    }
                }
                (Ok(_), Err(e)) => {
                    new_refuses += 1;
                    if shown < limit {
                        shown += 1;
                        println!("{path} {dialect:?}: new refuses at {:?}", e.span);
                    }
                }
                (Err(_), Ok(_)) => old_refuses += 1,
                (Err(_), Err(_)) => both_refuse += 1,
            }
        }
    }
    if compile {
        println!(
            "compiled: same output {compiled_same}, different output {compiled_differ}, only the old module compiles {compile_new_fails}, only the new module compiles {compile_old_fails}"
        );
    }
    println!(
        "same {same}, same but for spans {spans_differ}, differ {differ}, only the old takes {new_refuses}, only the new takes {old_refuses}, neither {both_refuse}"
    );
}
