//! The wording of every refusal over a set of sources: `diagsurvey DIALECT
//! FILE...`. Groups by BC's name and prints a count and a few distinct messages
//! for each.
use std::collections::BTreeMap;

use llrm_qb::{Dialect, front, source};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dialect = Dialect::parse(&args[0]).expect("a dialect");
    let per_code: usize = std::env::var("PER").ok().and_then(|n| n.parse().ok()).unwrap_or(4);
    let mut groups: BTreeMap<String, (usize, Vec<String>)> = BTreeMap::new();
    for path in &args[1..] {
        let Ok(loaded) = source::load_with_map(std::path::Path::new(path), &[]) else { continue };
        let Err(error) = front::parse(&loaded.text, dialect) else { continue };
        let code = error.code.clone();
        let line = loaded.text.lines().nth(error.span.line.saturating_sub(1)).unwrap_or("").trim().to_owned();
        let shown = format!(
            "{}\n      | {line}\n      help: {}",
            error.message.split(" [BC:").next().unwrap_or(""),
            error.help.as_deref().unwrap_or("-").replace('\n', "\n            ")
        );
        let entry = groups.entry(code).or_default();
        entry.0 += 1;
        if entry.1.len() < per_code && !entry.1.contains(&shown) {
            entry.1.push(shown);
        }
    }
    for (code, (count, shown)) in groups {
        println!("== {code} ({count})");
        for one in shown {
            println!("   - {one}");
        }
    }
}
