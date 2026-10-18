//! Masks derived by hand from `DemandedBits.cpp`; `tools/difftest` checks the
//! same analysis against `opt -passes='print<demanded-bits>'`.

use llrm_mir::module::{Function, InstId, Module, Use, ValueDef};

use super::{DemandedBits, Environment};
use crate::testing::{function, parsed, value};

fn instruction(
    function: &Function,
    name: &str,
) -> InstId {
    match function.value(value(function, name)).def {
        ValueDef::Instruction(inst) => inst,
        ValueDef::Argument(_) => panic!("%{name} is a parameter"),
    }
}

/// The bits of `%name` read in `@f`, or none if the instruction is dead.
fn demanded(
    text: &str,
    name: &str,
) -> Option<u128> {
    let module: Module = parsed(text);
    let callees = llrm_mir::memory::callees(&module);
    let environment = Environment {
        context: &module.context,
        layout: &crate::testing::layout(&module),
        callees: &callees,
        globals: &module.globals,
        metadata: &module.metadata,
    };
    let f = function(&module, "f");
    let found = DemandedBits::of(&environment, f);
    let inst = instruction(f, name);
    (!found.is_instruction_dead(&environment, f, inst)).then(|| found.demanded_bits(&environment, f, inst))
}

/// `and %a, 255` stored in full reads the low byte of `%a`, not all of it.
#[test]
fn test_a_mask_leaves_the_bits_it_clears_unread() {
    let text = "define void @f(ptr %p, i32 %x) {
b0:
  %a = add i32 %x, 1
  %m = and i32 %a, 255
  store i32 %m, ptr %p
  ret void
}
";
    assert_eq!(demanded(text, "m"), Some(u128::MAX >> 96));
    assert_eq!(demanded(text, "a"), Some(0xff));
}

/// A value nothing that stays reads is dead, and so is what only it read.
#[test]
fn test_a_value_nothing_reads_is_dead() {
    let text = "define i32 @f(i32 %x) {
b0:
  %a = mul i32 %x, 3
  %b = add i32 %a, 1
  ret i32 %x
}
";
    assert_eq!(demanded(text, "a"), None);
    assert_eq!(demanded(text, "b"), None);
}

/// A sum read at its low byte reads the low byte of each addend: the carry
/// only runs upward (`isMask` shortcut).
#[test]
fn test_a_sum_read_in_its_low_bits_reads_the_low_bits_of_its_addends() {
    let text = "define void @f(ptr %p, i32 %x, i32 %y) {
b0:
  %s = add i32 %x, %y
  %t = trunc i32 %s to i8
  store i8 %t, ptr %p
  ret void
}
";
    assert_eq!(demanded(text, "s"), Some(0xff));
}

/// Hand-derived from `determineLiveOperandBitsAddCarry` for `add i16 %a, 256`
/// read at 0xff00: the constant is known, so no carry can come from the low
/// byte of `%a`, and the demand is 0xff00 and not all of it.
#[test]
fn test_a_sum_with_a_constant_does_not_read_the_bits_below_what_is_read() {
    let text = "define void @f(ptr %p, i16 %x) {
b0:
  %a = add i16 %x, 5
  %s = add i16 %a, 256
  %m = and i16 %s, 65280
  store i16 %m, ptr %p
  ret void
}
";
    assert_eq!(demanded(text, "s"), Some(0xff00));
    assert_eq!(demanded(text, "a"), Some(0xff00));
}

/// A zero extension passes down only the bits of the narrow value; a sign
/// extension read above its source reads the sign bit.
#[test]
fn test_extensions_pass_down_the_source_bits() {
    let text = "define void @f(ptr %p, ptr %q, i8 %x, i8 %y) {
b0:
  %a = add i8 %x, 1
  %z = zext i8 %a to i32
  %c = and i32 %z, 15
  store i32 %c, ptr %p
  %b = add i8 %y, 1
  %s = sext i8 %b to i32
  %d = and i32 %s, 65535
  store i32 %d, ptr %q
  ret void
}
";
    assert_eq!(demanded(text, "a"), Some(0x0f));
    assert_eq!(demanded(text, "b"), Some(0xff));
}

/// A constant left shift reads the bits of its operand that reach what is read.
#[test]
fn test_a_left_shift_reads_the_bits_that_reach_the_read_ones() {
    let text = "define void @f(ptr %p, i32 %x) {
b0:
  %a = add i32 %x, 1
  %s = shl i32 %a, 8
  %m = and i32 %s, 65280
  store i32 %m, ptr %p
  ret void
}
";
    assert_eq!(demanded(text, "a"), Some(0xff));
}

/// A use whose operand bits are all unread is dead: `%x` in `and %x, 0`.
#[test]
fn test_a_use_nothing_of_is_read_is_dead() {
    let text = "define void @f(ptr %p, i32 %x, i32 %y) {
b0:
  %a = and i32 %x, 0
  %o = or i32 %y, %a
  %t = trunc i32 %o to i8
  store i8 %t, ptr %p
  ret void
}
";
    let module = parsed(text);
    let callees = llrm_mir::memory::callees(&module);
    let environment = Environment {
        context: &module.context,
        layout: &crate::testing::layout(&module),
        callees: &callees,
        globals: &module.globals,
        metadata: &module.metadata,
    };
    let f = function(&module, "f");
    let found = DemandedBits::of(&environment, f);
    let and = instruction(f, "a");
    assert_eq!(found.demanded_bits(&environment, f, and), 0xff);
    let first = Use { user: and, index: 0 };
    assert!(found.is_use_dead(&environment, f, first));
    assert_eq!(found.demanded_bits_of_use(&environment, f, first), Some(0));
}
