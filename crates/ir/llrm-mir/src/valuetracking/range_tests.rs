use crate::datalayout::DataLayout;
use crate::domtree::DomTree;
use crate::module::{GlobalKind, InstId, Operand};
use crate::parse;
use crate::valuetracking::{AssumptionCache, Query, column};

/// What `examples/vtdump.rs` prints for `name` under `tests/vt/range`.
fn dump(
    name: &str,
    cols: &[&str],
) -> Vec<String> {
    let path = format!("{}/tests/vt/range/{name}.ll", env!("CARGO_MANIFEST_DIR"));
    let module = parse::module(&std::fs::read_to_string(&path).expect("a readable file")).expect("parses");
    let layout = DataLayout::default();
    let mut lines = Vec::new();
    for global in &module.globals {
        let GlobalKind::Function(function) = &global.kind else { continue };
        if function.is_declaration() {
            continue;
        }
        let dt = DomTree::of(function);
        let assumptions = AssumptionCache::new(&module, function);
        let entry_first = function.block(function.entry().expect("an entry")).first();
        let mut values: Vec<(Operand, Option<InstId>)> =
            function.parameters().iter().map(|one| (Operand::Value(*one), entry_first)).collect();
        for (_, inst) in function.walk() {
            if let Some(result) = function.instruction(inst).result {
                values.push((Operand::Value(result), Some(inst)));
            }
        }
        let mut n = 0;
        for (value, cxt) in values {
            let mut query = Query::new(&module, function, &layout).with_cxt(cxt);
            query.dt = Some(&dt);
            query.assumptions = Some(&assumptions);
            if !(1..=128).contains(&query.scalar_bits(value)) {
                continue;
            }
            let mut line = format!("{} {n}", global.name.owned().unwrap_or_default());
            n += 1;
            for col in cols {
                line.push_str(&column(&query, value, col));
            }
            lines.push(line);
        }
    }
    lines
}

/// Every line of `expected`, LLVM's, must be ours.
fn assert_llvm(
    name: &str,
    cols: &[&str],
    expected: &str,
) {
    let ours = dump(name, cols);
    assert_eq!(ours.len(), expected.lines().count(), "{name}: values");
    for (ours, llvm) in ours.iter().zip(expected.lines()) {
        assert_eq!(ours, llvm, "{name}");
    }
}

#[test]
fn test_range_of_binary_operators_with_constant_operands() {
    assert_llvm("binops", &["cru", "crs", "pat"], include_str!("../../tests/vt/range/binops.llvm"));
}

#[test]
fn test_range_of_intrinsics() {
    assert_llvm("intrinsics", &["cru", "crs", "pat"], include_str!("../../tests/vt/range/intrinsics.llvm"));
}

#[test]
fn test_range_and_pattern_of_integer_selects() {
    assert_llvm("select", &["cru", "crs", "pat"], include_str!("../../tests/vt/range/select.llvm"));
}

#[test]
fn test_pattern_of_selects_through_casts_and_float_compares() {
    assert_llvm("casts", &["cru", "crs", "pat"], include_str!("../../tests/vt/range/casts.llvm"));
}

#[test]
fn test_range_of_arguments_calls_and_metadata() {
    assert_llvm("attributes", &["cru", "crs", "pat"], include_str!("../../tests/vt/range/attributes.llvm"));
}

#[test]
fn test_range_of_operands_of_overflow_checks() {
    assert_llvm("overflow", &["cru", "crs", "pat"], include_str!("../../tests/vt/range/overflow.llvm"));
}

/// Only the lines that need no known bits, sign bits, `noundef` or dominating
/// condition: those are other areas'.
#[test]
fn test_overflow_of_add_sub_and_mul_where_ranges_decide() {
    let ours = dump("overflow", &["ov"]);
    for llvm in include_str!("../../tests/vt/range/overflow.ov.llvm").lines() {
        let key = llvm.split(" ov=").next().expect("a key");
        let line = ours.iter().find(|one| one.starts_with(&format!("{key} "))).expect("a value");
        assert_eq!(line, llvm);
    }
}
