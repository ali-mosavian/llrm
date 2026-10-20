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
        let name = global.name.owned().unwrap_or_default();
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

/// `Value::getPointerOffsetFrom`: the bytes one pointer is past another, when
/// they differ by constants only, through casts, GEPs and a shared prefix of
/// variable indices.
#[test]
fn test_a_pointer_offset_from_another_is_found_through_geps() {
    let module = parse::module(
        "define void @f(ptr %p, ptr %q, i64 %i) {
b0:
  %a = getelementptr i8, ptr %p, i64 4
  %b = getelementptr i8, ptr %p, i64 12
  %x = getelementptr [4 x i32], ptr %p, i64 %i, i64 1
  %y = getelementptr [4 x i32], ptr %p, i64 %i, i64 3
  %z = getelementptr [4 x i32], ptr %p, i64 %i
  ret void
}
",
    )
    .expect("a module");
    let layout = DataLayout::default();
    let GlobalKind::Function(function) = &module.globals[0].kind else { panic!("a function") };
    let value = |name: &str| {
        let found =
            function.parameters().iter().copied().find(|&one| function.value(one).name.as_deref() == Some(name));
        Operand::Value(found.unwrap_or_else(|| {
            let (_, inst) = function
                .walk()
                .find(|(_, inst)| {
                    function.instruction(*inst).result.is_some_and(|r| function.value(r).name.as_deref() == Some(name))
                })
                .expect("a named value");
            function.instruction(inst).result.expect("a value")
        }))
    };
    let query = Query::new(&module, function, &layout);
    let offset = |this: &str, other: &str| super::nonzero::get_pointer_offset_from(&query, value(this), value(other));
    assert_eq!(offset("b", "a"), Some(8));
    assert_eq!(offset("a", "p"), Some(4));
    assert_eq!(offset("p", "b"), Some(-12));
    assert_eq!(offset("y", "x"), Some(8), "constant indices past a shared variable one");
    assert_eq!(offset("x", "z"), Some(4), "one more constant index, past a shared variable one");
    assert_eq!(offset("q", "p"), None, "unrelated pointers");
    assert_eq!(offset("a", "x"), None, "a variable index on one side only");
}
