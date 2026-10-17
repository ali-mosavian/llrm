//! `nz`, `p2` and `p2z` against LLVM's answers on `tests/vt/nonzero/*.ll`
//! (`expected` is `tools/llvmport/vtdump.sh nz,p2,p2z` on the file).
//!
//! A value whose answer rests on another area (`known`, `context`) is listed
//! in `pending`: it may be short of LLVM's until that area is translated, never
//! beyond it.

use crate::datalayout::DataLayout;
use crate::dominators::DominatorTree;
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
        let dt = DominatorTree::new(function);
        let assumptions = AssumptionCache::new(&module, function);
        let entry_first = function.block(function.entry().expect("an entry")).instructions().first().copied();
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

/// Every line of `expected` is in `source`'s dump, but for the `pending`
/// values, which may only claim less.
pub(super) fn check(
    source: &str,
    expected: &str,
    pending: &[(&str, usize)],
) {
    let ours = dump(source, &["nz", "p2", "p2z"]);
    assert_eq!(ours.len(), expected.lines().count());
    for (ours_line, theirs_line) in ours.iter().zip(expected.lines()) {
        if ours_line == theirs_line {
            continue;
        }
        let mut words = ours_line.split(' ');
        let (name, n) = (words.next().unwrap(), words.next().unwrap());
        let listed = pending.iter().any(|(pending_name, at)| *pending_name == name && at.to_string() == n);
        for (ours, theirs) in words.zip(theirs_line.split(' ').skip(2)).filter(|(ours, theirs)| ours != theirs) {
            assert!(listed && ours.ends_with("=0"), "llrm {ours} against llvm {theirs} in `{ours_line}`");
        }
    }
}

#[test]
fn pointers_are_non_null_as_llvm_says() {
    check(
        include_str!("../../tests/vt/nonzero/pointers.ll"),
        include_str!("../../tests/vt/nonzero/pointers.expected"),
        &[],
    );
}

#[test]
fn integers_are_non_zero_as_llvm_says() {
    check(
        include_str!("../../tests/vt/nonzero/ints.ll"),
        include_str!("../../tests/vt/nonzero/ints.expected"),
        &[("casts", 10), ("casts", 16), ("intrinsics", 11), ("intrinsics", 17), ("phis", 11)],
    );
}

#[test]
fn calls_guards_and_edges_are_non_null_as_llvm_says() {
    check(include_str!("../../tests/vt/nonzero/calls.ll"), include_str!("../../tests/vt/nonzero/calls.expected"), &[]);
}
