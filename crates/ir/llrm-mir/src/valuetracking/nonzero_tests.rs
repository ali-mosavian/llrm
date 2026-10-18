//! `nz`, `p2` and `p2z` against LLVM's answers on `tests/vt/nonzero/*.ll`
//! (`expected` is `tools/llvmport/vtdump.sh nz,p2,p2z` on the file).

use crate::datalayout::DataLayout;
use crate::domtree::DomTree;
use crate::module::{GlobalKind, InstId, Operand};
use crate::parse;
use crate::valuetracking::{AssumptionCache, Query, column};

/// The lines `examples/vtdump.rs` prints for `text`.
pub(super) fn dump(
    text: &str,
    cols: &[&str],
) -> Vec<String> {
    let module = parse::module(text).expect("a module");
    let layout =
        module.datalayout.as_deref().map(|text| DataLayout::parse(text).expect("a layout")).unwrap_or_default();
    let mut lines = Vec::new();
    for global in &module.globals {
        let GlobalKind::Function(function) = &global.kind else { continue };
        if function.is_declaration() {
            continue;
        }
        let name = global.name.clone().unwrap_or_default();
        let dt = DomTree::of(function);
        let assumptions = AssumptionCache::new(&module, function);
        let entry_first = function.block(function.entry().expect("an entry")).first();
        let mut values: Vec<(Operand, Option<InstId>)> =
            function.parameters().iter().map(|parameter| (Operand::Value(*parameter), entry_first)).collect();
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
            if query.scalar_bits(value) == 0 || query.scalar_bits(value) > 128 {
                continue;
            }
            let mut line = format!("{name} {n}");
            n += 1;
            for col in cols {
                line.push_str(&column(&query, value, col));
            }
            lines.push(line);
        }
    }
    lines
}

/// Every line of `expected` is in `source`'s dump.
pub(super) fn check(
    source: &str,
    expected: &str,
) {
    let ours = dump(source, &["nz", "p2", "p2z"]);
    assert_eq!(ours.len(), expected.lines().count());
    for (ours_line, theirs_line) in ours.iter().zip(expected.lines()) {
        assert_eq!(ours_line, theirs_line);
    }
}

#[test]
fn pointers_are_non_null_as_llvm_says() {
    check(include_str!("../../tests/vt/nonzero/pointers.ll"), include_str!("../../tests/vt/nonzero/pointers.expected"));
}

#[test]
fn integers_are_non_zero_as_llvm_says() {
    check(include_str!("../../tests/vt/nonzero/ints.ll"), include_str!("../../tests/vt/nonzero/ints.expected"));
}

#[test]
fn calls_guards_and_edges_are_non_null_as_llvm_says() {
    check(include_str!("../../tests/vt/nonzero/calls.ll"), include_str!("../../tests/vt/nonzero/calls.expected"));
}
