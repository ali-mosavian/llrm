//! `combine_assume`: an assume of a conjunction is two assumes, of a constant
//! is gone or unreachable.

use llrm_mir::datalayout::DataLayout;

use crate::combine::combine_with_declared;
use crate::testing::{parsed, printed};

fn combined(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with_declared(
        context,
        &globals,
        &metadata,
        &DataLayout::default(),
        function,
        |c, inst| {
            if matches!(
                c.function.instruction(inst).opcode,
                llrm_mir::opcode::Opcode::Call(_)
            ) {
                c.visit_call(inst)
            } else {
                crate::combine::Visit::None
            }
        },
        &mut declared,
    );
    declared.place(&mut module).unwrap();
    printed(&module)
}

/// `assume(a & b)` stayed one assume of the `and`, so ValueTracking saw one
/// opaque condition: pinned `opt -passes=instcombine` gives two assumes.
#[test]
fn an_assume_of_an_and_is_two_assumes() {
    let after = combined(
        "declare void @llvm.assume(i1)

define void @f(i32 %x, i32 %y) {
b0:
  %a = icmp sgt i32 %x, 0
  %b = icmp sgt i32 %y, 0
  %ab = and i1 %a, %b
  call void @llvm.assume(i1 %ab)
  ret void
}
",
    );
    assert_eq!(after.matches("call void @llvm.assume(").count(), 2, "{after}");
    assert!(!after.contains("and i1"), "{after}");
}

/// `assume(!(a | b))` is `assume(!a); assume(!b)`.
#[test]
fn an_assume_of_a_negated_or_is_two_negated_assumes() {
    let after = combined(
        "declare void @llvm.assume(i1)

define void @f(i32 %x, i32 %y) {
b0:
  %a = icmp sgt i32 %x, 0
  %b = icmp sgt i32 %y, 0
  %ab = or i1 %a, %b
  %n = xor i1 %ab, true
  call void @llvm.assume(i1 %n)
  ret void
}
",
    );
    assert_eq!(after.matches("call void @llvm.assume(").count(), 2, "{after}");
    assert!(!after.contains(" = or i1 "), "{after}");
    assert_eq!(after.matches("xor i1").count(), 2, "{after}");
}

/// An assume of true is erased; of false it is unreachable, a store to poison.
#[test]
fn an_assume_of_a_constant_is_erased_or_unreachable() {
    let yes = combined(
        "declare void @llvm.assume(i1)

define void @f() {
b0:
  call void @llvm.assume(i1 true)
  ret void
}
",
    );
    assert!(!yes.contains("call void @llvm.assume("), "{yes}");
    assert!(!yes.contains("store"), "{yes}");
    let no = combined(
        "declare void @llvm.assume(i1)

define void @f() {
b0:
  call void @llvm.assume(i1 false)
  ret void
}
",
    );
    assert!(!no.contains("call void @llvm.assume("), "{no}");
    assert!(no.contains("store i1 true, ptr poison"), "{no}");
}

/// A plain compare is left alone (no bundle is made, DEVIATE 1).
#[test]
fn an_assume_of_a_compare_is_left_alone() {
    let after = combined(
        "declare void @llvm.assume(i1)

define void @f(i32 %x) {
b0:
  %a = icmp sgt i32 %x, 0
  call void @llvm.assume(i1 %a)
  ret void
}
",
    );
    assert_eq!(after.matches("call void @llvm.assume(").count(), 1, "{after}");
}
