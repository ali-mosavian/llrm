//! `combine_switch`: a switch on an invertible condition or one whose high
//! bits do not vary switches on the simpler value.

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
                llrm_mir::opcode::Opcode::Switch
            ) {
                c.visit_switch(inst)
            } else {
                crate::combine::Visit::None
            }
        },
        &mut declared,
    );
    declared.place(&mut module).unwrap();
    printed(&module)
}

/// `switch (x + 2)` kept the add: pinned instcombine switches on `x` with
/// every case value less 2.
#[test]
fn a_switch_on_an_add_switches_on_the_operand_with_shifted_cases() {
    let after = combined(
        "define i32 @f(i32 %x) {
b0:
  %n = add i32 %x, 2
  switch i32 %n, label %d [ i32 5, label %a
                            i32 9, label %a ]
a:
  ret i32 1
d:
  ret i32 0
}
",
    );
    assert!(after.contains("switch i32 %x"), "{after}");
    assert!(after.contains("i32 3, label %a") && after.contains("i32 7, label %a"), "{after}");
}

/// `switch (zext i8 x)` stayed at 32 bits: the cases fit, so it switches on
/// the byte.
#[test]
fn a_switch_on_a_zero_extension_switches_on_the_narrow_value() {
    let after = combined(
        "define i32 @f(i8 %x) {
b0:
  %e = zext i8 %x to i32
  switch i32 %e, label %d [ i32 5, label %a ]
a:
  ret i32 1
d:
  ret i32 0
}
",
    );
    assert!(after.contains("switch i8 %x"), "{after}");
    assert!(after.contains("i8 5, label %a"), "{after}");
}

/// A switch whose condition is masked to a byte and whose cases are bytes
/// kept the 32-bit compare: pinned instcombine truncates to `i8`.
#[test]
fn a_switch_on_a_masked_value_switches_on_the_truncated_value() {
    let after = combined(
        "define i32 @f(i32 %x) {
b0:
  %m = and i32 %x, 255
  switch i32 %m, label %d [ i32 5, label %a ]
a:
  ret i32 1
d:
  ret i32 0
}
",
    );
    assert!(after.contains("trunc i32 %x to i8") || after.contains("switch i8"), "{after}");
}

/// `switch (x << 2)` with cases that are multiples of 4 kept the shift:
/// pinned instcombine switches on `x` with the cases shifted down.
#[test]
fn a_switch_on_a_nuw_shift_switches_on_the_operand_with_shifted_down_cases() {
    let after = combined(
        "define i32 @f(i32 %x) {
b0:
  %s = shl nuw i32 %x, 2
  switch i32 %s, label %d [ i32 4, label %a
                            i32 12, label %a ]
a:
  ret i32 1
d:
  ret i32 0
}
",
    );
    assert!(after.contains("switch i32 %x"), "{after}");
    assert!(after.contains("i32 1, label %a") && after.contains("i32 3, label %a"), "{after}");
}
