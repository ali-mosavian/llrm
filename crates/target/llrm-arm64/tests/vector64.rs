//! A 64-bit integer vector (`<2 x i32>`, `<4 x i16>`, `<8 x i8>`) was refused:
//! "machine opcode ADDv2i32 has no arm64 instruction yet". Each case is one
//! operation of two loads and a store, and must select the pinned `llc`'s
//! `v.2s`/`v.4h`/`v.8b` instruction.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

const HEAD: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n\
target triple = \"aarch64-unknown-linux-gnu\"\n";

fn assembly(
    vector: &str,
    op: &str,
) -> String {
    let ir = format!(
        "{HEAD}define void @f(ptr %p, ptr %q, ptr %r) {{\n\
         \x20 %x = load {vector}, ptr %p, align 8\n\
         \x20 %y = load {vector}, ptr %q, align 8\n\
         \x20 %z = {op} {vector} %x, %y\n\
         \x20 store {vector} %z, ptr %r, align 8\n\
         \x20 ret void\n}}\n"
    );
    driver::assembly(&ir, Platform::Linux, true).expect("lowers")
}

#[test]
fn a_64_bit_integer_vector_add_is_a_d_register_operation() {
    for (vector, arrangement) in [("<2 x i32>", "2s"), ("<4 x i16>", "4h"), ("<8 x i8>", "8b")] {
        let asm = assembly(vector, "add");
        let add = asm.lines().find(|line| line.trim_start().starts_with("add v")).unwrap_or_default();
        assert!(add.contains(&format!(".{arrangement}")), "{asm}");
        assert!(asm.contains("ldr d"), "{asm}");
    }
}

#[test]
fn a_64_bit_integer_vector_has_the_three_same_operations() {
    for (op, mnemonic) in [("sub", "sub"), ("mul", "mul"), ("and", "and"), ("or", "orr"), ("xor", "eor")] {
        let asm = assembly("<2 x i32>", op);
        let lane = if matches!(op, "and" | "or" | "xor") { "8b" } else { "2s" };
        assert!(asm.contains(&format!("{mnemonic} v")) && asm.contains(&format!(".{lane}")), "{op}: {asm}");
    }
}
