//! The typed parser against the table-driven one: the same `Module`, and
//! through the same semantic pass the same output, on every program the
//! repository has.

use std::path::{Path, PathBuf};
use std::process::Command;

use qbfront::Dialect;
use qbfront::semantic::{Options, compile_with_options};

fn tracked() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../..");
    let output =
        Command::new("git").arg("-C").arg(&root).args(["ls-files", "*.bas", "*.BAS", "*.bi", "*.BI"]).output().unwrap();
    // corpus/ holds generated oracle programs, many of them deliberately
    // malformed
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .filter(|line| !line.contains("lalrspike/corpus/"))
        .map(|line| root.join(line))
        .collect()
}

/// Programs whose trees differ on purpose: the table-driven parser reads `-(a +
/// 3) * .5` as `-((a + 3) * .5)`, a unary minus over the product, where BASIC's
/// minus binds tighter than `*`. Programs the table-driven parser takes and BC
/// refuses (`1E+10!`: a type character after an exponent).
const BC_REFUSES: [&str; 2] = ["tests/qbrt/real_print.bas", "tests/qbrt/real_print_edges.bas"];

const GROUPS_THE_MINUS_OVER_A_PRODUCT: [&str; 1] = ["tests/qbrt/gfx_circle_sweep.bas"];

#[test]
fn the_typed_parser_builds_the_module_the_table_driven_parser_builds() {
    let (mut same, mut compiled) = (0, 0);
    for path in tracked() {
        let source = lalrspike::dos_source(&std::fs::read(&path).unwrap());
        let excused = GROUPS_THE_MINUS_OVER_A_PRODUCT.iter().any(|name| path.ends_with(name));
        let family = path.to_string_lossy();
        let dialects: &[Dialect] = if family.contains("/vbdos/") {
            &[Dialect::VbDos]
        } else if family.contains("/pds71/") {
            &[Dialect::Pds71, Dialect::VbDos]
        } else {
            &[Dialect::QuickBasic45, Dialect::Pds71, Dialect::VbDos]
        };
        for &dialect in dialects {
            let Ok(old) = qbfront::parse(&source, dialect) else { continue };
            let new = match lalrspike::qb_ast::parse(&source, dialect) {
                Ok(new) => new,
                // programs written to be refused (BC refuses q45n41's
                // 41-character name too)
                Err(_) if family.contains("/negative/") || BC_REFUSES.iter().any(|name| path.ends_with(name)) => {
                    continue;
                }
                // OPTION EXPLICIT is VBDOS BC's alone ("BASE missing" in the
                // others)
                Err(_) if dialect != Dialect::VbDos && source.to_ascii_lowercase().contains("option explicit") => {
                    continue;
                }
                // an underscore in a name is VBDOS BC's alone; a
                // dialect-neutral program may use it
                Err(error) if error.message.contains("underscore") && dialect != Dialect::VbDos => continue,
                Err(error) => panic!("{} under {dialect:?}: {error:?}", path.display()),
            };
            if excused {
                continue;
            }
            // Spans of an omitted CIRCLE argument and the type of a real
            // literal aside, the modules are equal: BC types a
            // literal of eight digits or more DOUBLE, the table-driven scanner
            // past fifteen.
            let strip = |module: &qbfront::syntax::Module| {
                let text = format!("{module:?}");
                let mut out = String::new();
                let mut rest = text.as_str();
                while let Some(at) = rest.find("Span { line: ") {
                    out.push_str(&rest[..at]);
                    rest = &rest[rest[at..].find('}').map_or(rest.len(), |end| at + end + 1)..];
                }
                let mut out = out + rest;
                // the typed parser records the variable a NEXT names; the old
                // one never did
                while let Some(at) = out.find("next_variable: Some((") {
                    let close = out[at..].find("))").map_or(out.len(), |end| at + end + 2);
                    out.replace_range(at..close, "next_variable: None");
                }
                // the old parser records neither a DECLARE without a parameter
                // list nor `SUB p ()`
                let out =
                    out.replace("unchecked: true", "unchecked: false").replace("empty_list: true", "empty_list: false");
                // CURRENCY is a type of its own here; the old parser read it as
                // a user type's name (and a FUNCTION named with
                // `@` has a CURRENCY result, where the old parser left it
                // untyped)
                let out = out
                    .replace("result: Some(Currency)", "result: None")
                    .replace("Some(Currency)", "Some(Named(\"CURRENCY\"))");
                out.replace("\", Single)", "\", _)").replace("\", Double)", "\", _)")
            };
            assert!(strip(&old) == strip(&new), "{} under {dialect:?}: the modules differ", path.display());
            same += 1;
            if old != new {
                continue;
            }
            let options = Options::default();
            let old_out = compile_with_options(&old, "t", dialect, "llrm", &options);
            let new_out = compile_with_options(&new, "t", dialect, "llrm", &options);
            assert_eq!(old_out.is_ok(), new_out.is_ok(), "{} under {dialect:?}", path.display());
            if let (Ok(a), Ok(b)) = (&old_out, &new_out) {
                assert!(a == b, "{} under {dialect:?}: the outputs differ", path.display());
                compiled += 1;
            }
        }
    }
    assert!(same > 1500, "{same} programs compared");
    assert!(compiled > 800, "{compiled} programs compiled to the same output");
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
