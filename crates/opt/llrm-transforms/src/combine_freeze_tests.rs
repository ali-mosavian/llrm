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
