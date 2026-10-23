//! Mutate each line of the given programs by a token and compare the two parsers' verdicts and trees.
//!
//!     rdfuzz FILE...
use lalrspike::qb_tokens::Tok;
use qbfront::Dialect;

fn main() {
    let (mut checked, mut accepted, mut bad) = (0_u64, 0_u64, 0_u64);
    let mut seen = std::collections::HashSet::new();
    for path in std::env::args().skip(1) {
        let source = lalrspike::dos_source(&std::fs::read(&path).unwrap());
        let Some(Ok((toks, _))) = lalrspike::qb_lexer::prelex(&source, Dialect::VbDos) else { continue };
        for line in toks.split_inclusive(|t| *t == Tok::Kw(lalrspike::qb_kw::Kw::NewLine)) {
            if line.len() > 40 {
                continue;
            }
            let mut variants: Vec<Vec<Tok>> = vec![line.to_vec()];
            for j in 0..line.len() - 1 {
                let mut deleted = line.to_vec();
                deleted.remove(j);
                variants.push(deleted);
                let mut doubled = line.to_vec();
                doubled.insert(j, line[j].clone());
                variants.push(doubled);
                if j + 2 < line.len() {
                    let mut swapped = line.to_vec();
                    swapped.swap(j, j + 1);
                    variants.push(swapped);
                }
            }
            for variant in variants {
                if !seen.insert(variant.clone()) {
                    continue;
                }
                checked += 1;
                let lalr = lalrspike::parse_qb_kinds(&variant);
                let rd = lalrspike::qb_rd::parse(&variant);
                accepted += u64::from(lalr.is_ok());
                let agree = match (&lalr, &rd) {
                    (Ok(a), Ok(b)) => a == b,
                    (Err(_), Err(_)) => true,
                    _ => false,
                };
                if !agree {
                    bad += 1;
                    if bad <= 25 {
                        println!("lalrpop {:?} rd {:?}\n   {:?}", lalr.as_ref().map(|_| ()), rd.as_ref().map(|_| ()), variant);
                    }
                }
            }
        }
    }
    println!("{checked} variants, {accepted} accepted by lalrpop, {bad} disagreements");
}
