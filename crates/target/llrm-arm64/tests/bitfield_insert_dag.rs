//! `tryBitfieldInsertOp` through the users of the `or`, on the DAG path alone.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

const LAYOUT: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

/// `getUsefulBits` compared the user's opcode with the machine opcode as the matcher stores it, complemented, so it
/// never saw the byte store that reads only 8 bits. The `and w10, w10, #0x80` of the kept operand stayed: llc has
/// `ldrb; ldrb; bfxil w9, w8, #0, #7; strb` and the DAG path had an `and` more (seen first in `copysign` of `fp128`).
#[test]
fn a_byte_store_makes_the_bits_above_it_not_useful_to_the_insert() {
    let ir = format!(
        "{LAYOUT}define void @f(ptr %p, ptr %q, ptr %r) {{\n  %a = load i8, ptr %p\n  %b = load i8, ptr %q\n  \
%ax = zext i8 %a to i32\n  %bx = zext i8 %b to i32\n  %x = and i32 %ax, 127\n  %y = and i32 %bx, 128\n  \
%o = or i32 %x, %y\n  %t = trunc i32 %o to i8\n  store i8 %t, ptr %r\n  ret void\n}}\n"
    );
    let asm = driver::assembly(&ir, Platform::Linux, true).unwrap_or_else(|why| panic!("{why}"));
    assert!(asm.contains("bfm w10, w9, #0, #6"), "{asm}");
    assert!(!asm.contains("and w"), "the kept operand is not masked:\n{asm}");
}
