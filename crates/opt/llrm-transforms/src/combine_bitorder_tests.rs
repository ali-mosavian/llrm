//! `rev32(s) & 255` kept all five stages of a bit reversal (30 instructions
//! against the pin's 15): nothing made the shifts and masks one
//! `bitreverse`, so the mask never reached the input.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::{BinaryOp, Opcode};
use llrm_mir::passes::Declared;

use crate::combine::{Combiner, Visit, combine_with_declared};
use crate::testing::{parsed, printed};

/// `visit` that does `visitOr`, `visitAnd` and `visitXor`.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    match c.function.instruction(inst).opcode {
        Opcode::Binary(BinaryOp::Or) => crate::combine_or::visit_or(c, inst),
        Opcode::Binary(BinaryOp::And) => crate::combine_and::visit_and(c, inst),
        Opcode::Binary(BinaryOp::Xor) => crate::combine_xor::visit_xor(c, inst),
        _ => Visit::None,
    }
}

fn combined(
    params: &str,
    returns: &str,
    body: &str,
) -> String {
    let mut module = parsed(&format!("define {returns} @f({params}) {{\nb0:\n{body}}}\n"));
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with_declared(context, &globals, &metadata, &DataLayout::default(), function, visit, &mut declared);
    declared.place(&mut module).expect("placed");
    printed(&module)
}

/// The four byte moves of a 32-bit byte swap.
const BSWAP32: &str = "  %a = shl i32 %x, 24
  %b = and i32 %x, 65280
  %c = shl i32 %b, 8
  %d = lshr i32 %x, 8
  %e = and i32 %d, 65280
  %f = lshr i32 %x, 24
  %g = or i32 %a, %c
  %h = or i32 %e, %f
  %r = or i32 %g, %h
  ret i32 %r
";

#[test]
fn shifts_and_masks_that_move_the_four_bytes_are_a_bswap() {
    let out = combined("i32 %x", "i32", BSWAP32);
    assert!(out.contains("call i32 @llvm.bswap.i32(i32 %x)"), "{out}");
    assert!(!out.contains("shl") && !out.contains("lshr"), "{out}");
}

/// A reversal's five stages; the pin's `rev32`.
const REV32: &str = "  %a = lshr i32 %x, 1
  %b = and i32 %a, 1431655765
  %c = and i32 %x, 1431655765
  %d = shl i32 %c, 1
  %s1 = or i32 %b, %d
  %e = lshr i32 %s1, 2
  %f = and i32 %e, 858993459
  %g = and i32 %s1, 858993459
  %h = shl i32 %g, 2
  %s2 = or i32 %f, %h
  %i = lshr i32 %s2, 4
  %j = and i32 %i, 252645135
  %k = and i32 %s2, 252645135
  %l = shl i32 %k, 4
  %s3 = or i32 %j, %l
  %m = lshr i32 %s3, 8
  %n = and i32 %m, 16711935
  %o = and i32 %s3, 16711935
  %p = shl i32 %o, 8
  %s4 = or i32 %n, %p
  %q = lshr i32 %s4, 16
  %t = shl i32 %s4, 16
  %rev = or i32 %q, %t
";

#[test]
fn the_five_stages_of_a_bit_reversal_are_a_bitreverse() {
    let out = combined("i32 %x", "i32", &format!("{REV32}  ret i32 %rev\n"));
    assert!(out.contains("call i32 @llvm.bitreverse.i32(i32 %x)"), "{out}");
    assert!(!out.contains("shl") && !out.contains("lshr"), "{out}");
}

/// What the pin makes of `rev32(s) & 255`: the mask is of the input, reversed.
#[test]
fn a_masked_byte_of_a_bit_reversal_masks_the_input() {
    let out = combined("i32 %x", "i32", &format!("{REV32}  %r = and i32 %rev, 255\n  ret i32 %r\n"));
    assert!(out.contains("and i32 %x, -16777216"), "{out}");
    assert!(out.contains("@llvm.bitreverse.i32"), "{out}");
    assert!(!out.contains("shl") && !out.contains("lshr"), "{out}");
}

/// Two bits swapped at the ends of nine (`x0 -> r8`, `x8 -> r0`): a reversal of
/// nine bits, whose width is no multiple of 8. `correct_for_bswap` took
/// `(9 >> 3) - (8 >> 3) - 1` in unsigned arithmetic and panicked with "subtract
/// with overflow" (jpeg_fixed at -O2 -g); the pin's unsigned wrap says "no".
#[test]
fn a_nine_bit_reversal_is_not_a_byte_swap_and_does_not_panic() {
    let body = "  %a = and i32 %x, 1
  %b = shl i32 %a, 8
  %c = and i32 %x, 256
  %d = lshr i32 %c, 8
  %r = or i32 %b, %d
  ret i32 %r
";
    let out = combined("i32 %x", "i32", body);
    assert!(out.contains("llvm.bitreverse.i9"), "{out}");
}
