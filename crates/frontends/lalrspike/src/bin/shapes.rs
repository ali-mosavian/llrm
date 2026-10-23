//! For each single-statement line of the given programs: the generic tree's rule against
//! the statement the existing parser builds.
use std::collections::BTreeMap;

use lalrspike::qb_tree::Tree;
use qbfront::syntax::Statement;
use qbfront::Dialect;

fn rule(tree: &Tree) -> Option<String> {
    match tree {
        Tree::Node(name, _) => Some((*name).to_owned()),
        Tree::List(items) => {
            let first = items.iter().find(|t| !matches!(t, Tree::Empty))?;
            rule(first)
        }
        _ => None,
    }
}

fn describe(statement: &Statement) -> String {
    let text = format!("{statement:?}");
    let variant: String = text.chars().take_while(|c| c.is_alphanumeric()).collect();
    match statement {
        Statement::Runtime { name, arguments, .. } => format!("Runtime({name}, {} args)", arguments.len()),
        Statement::Call { name, arguments, explicit, .. } => format!("Call({name}, {} args, explicit {explicit})", arguments.len()),
        _ => variant,
    }
}

fn main() {
    let mut table: BTreeMap<(String, String), (usize, String)> = BTreeMap::new();
    for path in std::env::args().skip(1) {
        let source = lalrspike::dos_source(&std::fs::read(&path).unwrap());
        for line in source.lines() {
            let line = format!("{}\n", line.trim_end());
            for dialect in [Dialect::QuickBasic45] {
                let Ok(module) = qbfront::parse(&line, dialect) else { continue };
                if module.statements.len() != 1 || !module.procedures.is_empty() {
                    continue;
                }
                let Some(Ok((kinds, _))) = lalrspike::qb_lexer::prelex(&line, dialect) else { continue };
                let Ok(tree) = lalrspike::qb_rd::parse(&kinds) else { continue };
                let Some(rule) = rule(&tree) else { continue };
                let entry = table.entry((rule, describe(&module.statements[0]))).or_insert((0, line.trim().to_owned()));
                entry.0 += 1;
            }
        }
    }
    for ((rule, old), (count, example)) in table {
        println!("{count:>5} {rule:<14} -> {old:<40} e.g. {example}");
    }
}
