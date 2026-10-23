//! The typed parser against the table-driven one: the same `Module`, and through the same semantic
//! pass the same output, on every program the repository has.

use std::path::{Path, PathBuf};
use std::process::Command;

use qbfront::semantic::{compile_with_options, Options};
use qbfront::Dialect;

fn tracked() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let output = Command::new("git").arg("-C").arg(&root).args(["ls-files", "*.bas", "*.BAS", "*.bi", "*.BI"]).output().unwrap();
    String::from_utf8(output.stdout).unwrap().lines().map(|line| root.join(line)).collect()
}

/// Programs whose trees differ on purpose: the table-driven parser reads `-(a + 3) * .5` as
/// `-((a + 3) * .5)`, a unary minus over the product, where BASIC's minus binds tighter than `*`.
const GROUPS_THE_MINUS_OVER_A_PRODUCT: [&str; 1] = ["tests/qbrt/gfx_circle_sweep.bas"];

#[test]
fn the_typed_parser_builds_the_module_the_table_driven_parser_builds() {
    let (mut same, mut compiled) = (0, 0);
    for path in tracked() {
        let source = lalrspike::dos_source(&std::fs::read(&path).unwrap());
        let excused = GROUPS_THE_MINUS_OVER_A_PRODUCT.iter().any(|name| path.ends_with(name));
        for dialect in [Dialect::QuickBasic45, Dialect::Pds71, Dialect::VbDos] {
            let Ok(old) = qbfront::parse(&source, dialect) else { continue };
            let new = lalrspike::qb_ast::parse(&source, dialect)
                .unwrap_or_else(|error| panic!("{} under {dialect:?}: {error:?}", path.display()));
            if excused {
                continue;
            }
            // Spans of an omitted CIRCLE argument aside, the modules are equal.
            let strip = |module: &qbfront::syntax::Module| {
                let text = format!("{module:?}");
                let mut out = String::new();
                let mut rest = text.as_str();
                while let Some(at) = rest.find("Span { line: ") {
                    out.push_str(&rest[..at]);
                    rest = &rest[rest[at..].find('}').map_or(rest.len(), |end| at + end + 1)..];
                }
                out + rest
            };
            assert_eq!(strip(&old), strip(&new), "{} under {dialect:?}", path.display());
            same += 1;
            let options = Options::default();
            let old_out = compile_with_options(&old, "t", dialect, "llrm", &options);
            let new_out = compile_with_options(&new, "t", dialect, "llrm", &options);
            assert_eq!(old_out.is_ok(), new_out.is_ok(), "{} under {dialect:?}", path.display());
            if let (Ok(a), Ok(b)) = (&old_out, &new_out) {
                assert_eq!(a, b, "{} under {dialect:?}", path.display());
                compiled += 1;
            }
        }
    }
    assert!(same > 1500, "{same} programs compared");
    assert!(compiled > 1000, "{compiled} programs compiled to the same output");
}

fn value_of(source: &str) -> qbfront::syntax::Expr {
    let module = lalrspike::qb_ast::parse(&format!("x = {source}\n"), Dialect::QuickBasic45).expect("parses");
    match module.statements.into_iter().next() {
        Some(qbfront::syntax::Statement::Assign { value, .. }) => value,
        other => panic!("{other:?}"),
    }
}

/// QB 4.5 itself (BC and LINK, run under DOSBox-X) printed, for
///   PRINT 2 ^ 3 ^ 2          64     (^ is left to right)
///   PRINT NOT 1 = 2          -1     (NOT is below the relations)
///   PRINT NOT 0 + 1          -2
///   PRINT -2 ^ 2             -4     (^ is above the sign)
///   PRINT -3 * 2 ^ 2         -12
///   PRINT 7 MOD 3 * 2        1      (MOD is below *)
/// The table-driven parser reads the first three the other way (512, 0, 0).
#[test]
fn precedence_is_the_compilers() {
    use qbfront::syntax::{Binary, Expr, Unary};
    let Expr::Binary { op: Binary::Power, left, .. } = value_of("2 ^ 3 ^ 2") else { panic!("not a power") };
    assert!(matches!(*left, Expr::Binary { op: Binary::Power, .. }), "a ^ b ^ c is (a ^ b) ^ c");

    let Expr::Unary { op: Unary::Not, operand, .. } = value_of("NOT 1 = 2") else { panic!("not a NOT") };
    assert!(matches!(*operand, Expr::Binary { op: Binary::Eq, .. }), "NOT takes the comparison");
    let Expr::Unary { op: Unary::Not, operand, .. } = value_of("NOT 0 + 1") else { panic!("not a NOT") };
    assert!(matches!(*operand, Expr::Binary { op: Binary::Add, .. }), "NOT takes the sum");

    let Expr::Unary { op: Unary::Negative, operand, .. } = value_of("-2 ^ 2") else { panic!("not a negation") };
    assert!(matches!(*operand, Expr::Binary { op: Binary::Power, .. }), "the sign is below ^");
    let Expr::Binary { op: Binary::Multiply, left, .. } = value_of("-3 * 2 ^ 2") else { panic!("not a product") };
    assert!(matches!(*left, Expr::Unary { op: Unary::Negative, .. }), "the sign is above *");

    let Expr::Binary { op: Binary::Modulo, right, .. } = value_of("7 MOD 3 * 2") else { panic!("not a MOD") };
    assert!(matches!(*right, Expr::Binary { op: Binary::Multiply, .. }), "MOD is below *");
}
