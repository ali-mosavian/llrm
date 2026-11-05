//! `combine_freeze`: a freeze moves to the one operand that may be poison, and
//! a freeze of undef becomes the constant its users prefer.

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
                llrm_mir::opcode::Opcode::Freeze
            ) {
                c.visit_freeze(inst)
            } else {
                crate::combine::Visit::None
            }
        },
        &mut declared,
    );
    declared.place(&mut module).unwrap();
    printed(&module)
}

/// `freeze (add x, 1)` kept the freeze on the sum: pinned instcombine
/// freezes the operand `x` and adds to it.
#[test]
fn a_freeze_of_an_add_moves_to_the_possibly_poison_operand() {
    let after = combined(
        "define i32 @f(i32 %x) {
b0:
  %a = add i32 %x, 1
  %r = freeze i32 %a
  ret i32 %r
}
",
    );
    assert!(after.contains("freeze i32 %x"), "{after}");
    assert!(!after.contains("freeze i32 %a"), "{after}");
}

/// `freeze undef` used by an `or` stayed: pinned instcombine picks all ones.
#[test]
fn a_freeze_of_undef_under_an_or_is_all_ones() {
    let after = combined(
        "define i32 @f(i32 %x) {
b0:
  %u = freeze i32 undef
  %r = or i32 %x, %u
  ret i32 %r
}
",
    );
    assert!(after.contains("or i32 %x, -1"), "{after}");
    assert!(!after.contains("freeze"), "{after}");
}

/// A freeze of an argument with another user left that user unfrozen: pinned
/// instcombine (`freezeOtherUses`) hoists the freeze to the entry and reads it
/// in every dominated use (`freeze_dominated_uses_test1`). It cost
/// `freeze i8` after jpeg's `inbuf` load being missing at -O1.
#[test]
fn a_freeze_replaces_the_dominated_uses_of_its_operand() {
    let after = combined(
        "declare void @use(i32)
define void @f(i32 %v) {
b0:
  %fr = freeze i32 %v
  call void @use(i32 %v)
  call void @use(i32 %fr)
  ret void
}
",
    );
    assert!(!after.contains("@use(i32 %v)"), "{after}");
    assert_eq!(after.matches("freeze i32 %v").count(), 1, "{after}");
}

/// A freeze in a branch arm stayed there with the other arm unfrozen: the
/// freeze moves to right after the definition (the entry) and both arms read it.
#[test]
fn a_freeze_moves_next_to_the_definition_to_dominate_more_uses() {
    let after = combined(
        "declare void @use(i32)
define void @f(i32 %v, i1 %c) {
b0:
  br i1 %c, label %a, label %b
a:
  %fr = freeze i32 %v
  call void @use(i32 %fr)
  ret void
b:
  call void @use(i32 %v)
  ret void
}
",
    );
    assert!(!after.contains("@use(i32 %v)"), "{after}");
    assert!(after.find("freeze").unwrap() < after.find("br i1").unwrap(), "{after}");
}
