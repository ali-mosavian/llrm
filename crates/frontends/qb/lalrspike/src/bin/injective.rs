//! Statements BC takes whose different spellings give the same tree: a tree
//! that cannot tell them apart has lost something the later passes would need.
//!
//!     injective DIALECT CATALOG.tsv [bc_results.json family]
use std::collections::BTreeMap;

use qbfront::Dialect;

fn strip_spans(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("Span { line: ") {
        out.push_str(&rest[..at]);
        let close = rest[at..].find('}').map_or(rest.len(), |end| at + end + 1);
        out.push('_');
        rest = &rest[close..];
    }
    out.push_str(rest);
    out
}

/// The statement without its blanks and case, outside strings.
fn spelling(text: &str) -> String {
    let mut out = String::new();
    let mut quoted = false;
    for c in text.chars() {
        if c == '"' {
            quoted = !quoted;
        }
        if quoted || !c.is_whitespace() {
            out.push(if quoted { c } else { c.to_ascii_uppercase() });
        }
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dialect = match args[0].as_str() {
        "qb45" => Dialect::QuickBasic45,
        "pds71" => Dialect::Pds71,
        _ => Dialect::VbDos,
    };
    let catalog = std::fs::read_to_string(&args[1]).unwrap();
    let accepted: Option<std::collections::BTreeSet<String>> = args
        .get(2)
        .map(
            |path| {
                let family = args.get(3).map_or("qb45", String::as_str);
                let text = std::fs::read_to_string(path).unwrap();
                let json: serde_json_lite::Map = serde_json_lite::parse(&text);
                json.accepted(family)
            },
        );
    let mut groups: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for line in catalog.lines() {
        let (id, text) = line.split_once('\t').unwrap();
        if let Some(ok) = &accepted {
            if !ok.contains(id) {
                continue;
            }
        }
        let source = format!("{text}\r\n");
        let Ok(module) = lalrspike::qb_ast::parse(&source, dialect) else { continue };
        let key = strip_spans(&format!("{module:?}"));
        groups.entry(key).or_default().push((spelling(text), text.to_owned()));
    }
    let mut collisions = 0;
    for (_, items) in groups {
        let mut distinct: Vec<&(String, String)> = Vec::new();
        for item in &items {
            if !distinct.iter().any(|seen| seen.0 == item.0) {
                distinct.push(item);
            }
        }
        if distinct.len() > 1 {
            collisions += 1;
            println!("{}", distinct.iter().map(|(_, text)| text.as_str()).collect::<Vec<_>>().join("   ||   "));
        }
    }
    eprintln!("{collisions} trees with more than one spelling");
}

/// Just enough JSON to read `{"id": {"qb45": null | [..], ...}}`.
mod serde_json_lite {
    pub struct Map(Vec<(String, Vec<(String, bool)>)>);
    pub fn parse(text: &str) -> Map {
        let value: Vec<(String, Vec<(String, bool)>)> = text
            .split("}, \"")
            .map(|chunk| {
                let id: String =
                    chunk.trim_start_matches(['{', '"']).chars().take_while(|c| c.is_ascii_digit()).collect();
                let mut families = Vec::new();
                for family in ["qb45", "pds71", "vbdos"] {
                    if let Some(at) = chunk.find(&format!("\"{family}\": ")) {
                        let rest = &chunk[at + family.len() + 4..];
                        families.push((family.to_owned(), rest.starts_with("null")));
                    }
                }
                (id, families)
            })
            .collect();
        Map(value)
    }
    impl Map {
        pub fn accepted(
            &self,
            family: &str,
        ) -> std::collections::BTreeSet<String> {
            self.0
                .iter()
                .filter(|(_, f)| f.iter().any(|(name, ok)| name == family && *ok))
                .map(|(id, _)| id.clone())
                .collect()
        }
    }
}
