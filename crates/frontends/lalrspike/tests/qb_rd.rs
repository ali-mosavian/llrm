//! The recursive-descent parser against the lalrpop one: the same verdict and the same
//! tree on every program the repository has, and on one-token mutations of its lines.

use std::path::{Path, PathBuf};
use std::process::Command;

use lalrspike::qb_kw::Kw;
use lalrspike::qb_tokens::Tok;
use qbfront::Dialect;

fn tracked() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let output = Command::new("git").arg("-C").arg(&root).args(["ls-files", "*.bas", "*.BAS", "*.bi", "*.BI"]).output().unwrap();
    String::from_utf8(output.stdout).unwrap().lines().map(|line| root.join(line)).collect()
}

#[test]
fn recursive_descent_builds_lalrpops_tree_on_every_program() {
    let mut parsed = 0;
    for path in tracked() {
        let source = lalrspike::dos_source(&std::fs::read(&path).unwrap());
        for dialect in [Dialect::QuickBasic45, Dialect::Pds71, Dialect::VbDos] {
            let Some(Ok((kinds, _))) = lalrspike::qb_lexer::prelex(&source, dialect) else { continue };
            let what = format!("{} under {dialect:?}", path.display());
            match (lalrspike::parse_qb_kinds(&kinds), lalrspike::qb_rd::parse(&kinds)) {
                (Ok(a), Ok(b)) => {
                    assert_eq!(a, b, "{what}");
                    parsed += 1;
                }
                (Err(_), Err(_)) => {}
                (a, b) => panic!("{what}: lalrpop {:?}, recursive descent {:?}", a.map(|_| ()), b.map(|_| ())),
            }
        }
    }
    assert!(parsed > 1500, "parsed {parsed}");
}

/// Delete, double or swap one token of each short line: the parsers must refuse and accept alike.
/// (`+ a - b, c(d)` is the one thing lalrpop takes that recursive descent does not: a statement
/// that begins with a sign, which the grammar's `Neg<NameRefApplied>` lets in.)
#[test]
fn recursive_descent_refuses_what_lalrpop_refuses() {
    let newline = Tok::Kw(Kw::NewLine);
    let (mut checked, mut disagreements) = (0, Vec::new());
    let mut seen = std::collections::HashSet::new();
    for path in tracked().into_iter().step_by(5) {
        let source = lalrspike::dos_source(&std::fs::read(&path).unwrap());
        let Some(Ok((kinds, _))) = lalrspike::qb_lexer::prelex(&source, Dialect::VbDos) else { continue };
        for line in kinds.split_inclusive(|token| *token == newline).filter(|line| line.len() <= 30) {
            let mut variants = vec![line.to_vec()];
            for j in 0..line.len() - 1 {
                let mut deleted = line.to_vec();
                deleted.remove(j);
                let mut doubled = line.to_vec();
                doubled.insert(j, line[j].clone());
                let mut swapped = line.to_vec();
                swapped.swap(j, j + 1);
                variants.extend([deleted, doubled, swapped]);
            }
            for variant in variants {
                if !seen.insert(variant.clone()) {
                    continue;
                }
                checked += 1;
                let agree = match (lalrspike::parse_qb_kinds(&variant), lalrspike::qb_rd::parse(&variant)) {
                    (Ok(a), Ok(b)) => a == b,
                    (Err(_), Err(_)) => true,
                    _ => false,
                };
                if !agree {
                    disagreements.push(variant);
                }
            }
        }
    }
    assert!(checked > 10_000, "checked {checked}");
    let signed = |variant: &Vec<Tok>| {
        let rest = if variant.first() == Some(&Tok::Int) { &variant[1..] } else { &variant[..] };
        matches!(rest.first(), Some(Tok::Kw(Kw::Add | Kw::Minus)))
    };
    disagreements.retain(|variant| !signed(variant));
    assert!(disagreements.is_empty(), "{} of {checked}: {:?}", disagreements.len(), disagreements.first());
}
