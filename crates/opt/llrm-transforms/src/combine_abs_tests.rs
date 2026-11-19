//! `combine_abs`: `llvm.abs` looks through a negation, a sign extension and
//! `srem x, 2`.

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

/// `abs(0 - x)` stayed a negation feeding the `abs`: pinned instcombine
/// gives `abs(x)`, one instruction fewer.
#[test]
fn the_abs_of_a_negation_is_the_abs_of_the_value() {
    let after = combined(
        "declare i32 @llvm.abs.i32(i32, i1)

define i32 @f(i32 %x) {
b0:
  %n = sub i32 0, %x
  %r = call i32 @llvm.abs.i32(i32 %n, i1 false)
  ret i32 %r
}
",
    );
    assert!(after.contains("@llvm.abs.i32(i32 %x, i1 false)"), "{after}");
    assert!(!after.contains("sub i32 0"), "{after}");
}

/// `abs(srem x, 2)` stayed a division: pinned instcombine gives `x & 1`.
#[test]
fn the_abs_of_a_remainder_by_two_is_the_low_bit() {
    let after = combined(
        "declare i32 @llvm.abs.i32(i32, i1)

define i32 @f(i32 %x) {
b0:
  %m = srem i32 %x, 2
  %r = call i32 @llvm.abs.i32(i32 %m, i1 false)
  ret i32 %r
}
",
    );
    assert!(after.contains("and i32 %x, 1"), "{after}");
    assert!(!after.contains("srem"), "{after}");
}

/// `abs(sext x)` stayed at the wide type: pinned instcombine narrows it to a
/// zero extension of the narrow `abs`.
#[test]
fn the_abs_of_a_sign_extension_is_a_zero_extended_narrow_abs() {
    let after = combined(
        "declare i32 @llvm.abs.i32(i32, i1)

define i32 @f(i8 %x) {
b0:
  %e = sext i8 %x to i32
  %r = call i32 @llvm.abs.i32(i32 %e, i1 false)
  ret i32 %r
}
",
    );
    assert!(after.contains("@llvm.abs.i8(i8 %x, i1 false)"), "{after}");
    assert!(after.contains("zext i8"), "{after}");
}
