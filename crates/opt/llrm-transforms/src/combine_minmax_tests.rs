//! `combine_minmax`: a min or max of extended operands is narrow.

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

/// The closed form of `sum i for i < min(n, 10)` took `umin.i33` of a
/// `zext i16`, a second 64-bit compare beside the 16-bit one the same value
/// already had: 107 bytes at m16 -Os where main has 35.
#[test]
fn a_umin_of_a_zext_and_a_constant_is_the_zext_of_a_narrow_umin() {
    let after = combined(
        "declare i33 @llvm.umin.i33(i33, i33)

define i33 @f(i16 %x) {
b0:
  %e = zext i16 %x to i33
  %m = call i33 @llvm.umin.i33(i33 %e, i33 9)
  ret i33 %m
}
",
    );
    assert!(after.contains("@llvm.umin.i16(i16 %x, i16 9)") && after.contains("zext i16 %"), "{after}");
    assert!(!after.contains("@llvm.umin.i33(") || after.contains("declare i33"), "{after}");
    assert!(!after.contains("call i33"), "{after}");
}

/// A constant past the narrow type stays wide.
#[test]
fn a_constant_the_narrow_type_cannot_hold_keeps_the_wide_min() {
    let after = combined(
        "declare i32 @llvm.umin.i32(i32, i32)

define i32 @f(i16 %x) {
b0:
  %e = zext i16 %x to i32
  %m = call i32 @llvm.umin.i32(i32 %e, i32 70000)
  ret i32 %m
}
",
    );
    assert!(after.contains("call i32 @llvm.umin.i32("), "{after}");
}

/// Two extensions of one type, and the signed pair.
#[test]
fn two_extensions_of_one_type_narrow_and_a_smax_takes_sext() {
    let after = combined(
        "declare i32 @llvm.smax.i32(i32, i32)

define i32 @f(i8 %x, i8 %y) {
b0:
  %a = sext i8 %x to i32
  %b = sext i8 %y to i32
  %m = call i32 @llvm.smax.i32(i32 %a, i32 %b)
  ret i32 %m
}
",
    );
    assert!(after.contains("@llvm.smax.i8(i8 %x, i8 %y)") && after.contains("sext i8 %"), "{after}");
}
