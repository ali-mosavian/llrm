//! The recursive-descent parser against the lalrpop one: same verdict, same tree.
//!
//!     rdcheck FILE...        (prints the first disagreements)
use qbfront::Dialect;

fn diff(a: &lalrspike::qb_tree::Tree, b: &lalrspike::qb_tree::Tree, path: &mut Vec<String>) -> Option<String> {
    use lalrspike::qb_tree::Tree::*;
    match (a, b) {
        (Node(x, xs), Node(y, ys)) if x == y && xs.len() == ys.len() => {
            for (i, (p, q)) in xs.iter().zip(ys).enumerate() {
                path.push(format!("{x}[{i}]"));
                if let Some(found) = diff(p, q, path) {
                    return Some(found);
                }
                path.pop();
            }
            None
        }
        (List(xs), List(ys)) if xs.len() == ys.len() => {
            for (i, (p, q)) in xs.iter().zip(ys).enumerate() {
                path.push(format!("#{i}"));
                if let Some(found) = diff(p, q, path) {
                    return Some(found);
                }
                path.pop();
            }
            None
        }
        _ if a == b => None,
        _ => Some(format!("{} : lalrpop {:?} vs rd {:?}", path.join("."), brief(a), brief(b))),
    }
}

fn brief(tree: &lalrspike::qb_tree::Tree) -> String {
    let text = format!("{tree:?}");
    text.chars().take(120).collect()
}

fn main() {
    let (mut same, mut both_refuse, mut only_lalr, mut only_rd, mut different) = (0, 0, 0, 0, 0);
    let mut shown = 0;
    for path in std::env::args().skip(1) {
        let source = lalrspike::dos_source(&std::fs::read(&path).unwrap());
        for dialect in [Dialect::QuickBasic45, Dialect::Pds71, Dialect::VbDos] {
            let mut spans = Vec::new();
            let lalr = lalrspike::parse_qb_stream(&source, dialect, &mut spans);
            let Some(Ok((toks, spans))) = lalrspike::qb_lexer::prelex(&source, dialect) else { continue };
            let rd = lalrspike::qb_rd::parse(&toks);
            let say = |what: &str, extra: String| println!("{path} {dialect:?}: {what} {extra}");
            match (&lalr, &rd) {
                (Ok(a), Ok(b)) if a == b => same += 1,
                (Ok(a), Ok(b)) => {
                    different += 1;
                    if shown < 25 {
                        shown += 1;
                        say("tree differs", diff(a, b, &mut Vec::new()).unwrap_or_default());
                    }
                }
                (Err(_), Err(_)) => both_refuse += 1,
                (Ok(_), Err(at)) => {
                    only_lalr += 1;
                    if shown < 25 {
                        shown += 1;
                        let s = spans.get(*at);
                        say("rd refuses", format!("at {:?} {:?}", s.map(|s| (s.line, s.start)), toks.get(*at)));
                    }
                }
                (Err(e), Ok(_)) => {
                    only_rd += 1;
                    if shown < 25 {
                        shown += 1;
                        say("rd accepts, lalrpop refuses", e.chars().take(100).collect());
                    }
                }
            }
        }
    }
    println!("same {same}, both refuse {both_refuse}, only lalrpop {only_lalr}, only rd {only_rd}, trees differ {different}");
}
