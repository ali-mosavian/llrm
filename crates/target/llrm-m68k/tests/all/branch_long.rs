//! A 16-bit branch over more than 32 KiB is the 32-bit branch on the 68020 and
//! later (`M68kAsmBackend::getRelaxedOpcodeBranch`, 16 -> 32): the object
//! writer refused it ("a value of 40004 does not fit its 2-byte field"), which
//! QB `synth.c` (55,890 B) hit before #10723 shrank it. The 68000 has no
//! 32-bit branch; the pin's `fixupNeedsRelaxationAdvanced` is unreachable
//! there, and here too the object stays refused.

use llrm_m68k::compile::Item;
use llrm_m68k::driver::{self, Level, cpu_of};
use llrm_m68k::elf::relocatable_with_branches;
use llrm_m68k::mc::{Inst, Operand};
use llrm_support::hash::HashMap;

fn far_branch() -> Vec<Item> {
    vec![
        Item::Label("f".into()),
        Item::Inst(Inst { opcode: "Bne16", operands: vec![Operand::Symbol(".far".into(), 0)] }),
        Item::Bytes([0x4e, 0x71].repeat(20_000)),
        Item::Label(".far".into()),
        Item::Bytes(vec![0x4e, 0x75]),
    ]
}

#[test]
fn a_branch_over_32_kib_takes_the_32_bit_form_on_the_68020() {
    let object = relocatable_with_branches("far", &far_branch(), &HashMap::default(), &[], true).expect("relaxes");
    // `bne.l`: 0x66ff, then the displacement from the extension word, 40,004 = 0x9c44
    let found = object.windows(6).any(|window| window == [0x66, 0xff, 0x00, 0x00, 0x9c, 0x44]);
    assert!(found, "no bne.l with a 40,004 displacement in the object");
}

#[test]
fn the_68000_keeps_refusing_a_branch_over_32_kib() {
    let error = relocatable_with_branches("far", &far_branch(), &HashMap::default(), &[], false).unwrap_err();
    assert!(error.contains("does not fit"), "{error}");
}

#[test]
fn a_function_over_32_kib_compiles_at_the_68020() {
    let stores: String = (0..6_000).map(|n| format!("  store volatile i32 {}, ptr @g, align 4\n", 100_000 + n)).collect();
    let ir = format!(
        "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n@g = global i32 0\n\
         define void @big(i1 %c) {{\nentry:\n  br i1 %c, label %body, label %done\nbody:\n{stores}  br label %done\ndone:\n  ret void\n}}\n"
    );
    let bytes = driver::object(&ir, cpu_of("68020").unwrap(), Level::Os, "big").expect("compiles");
    assert!(bytes.len() > 32_768);
}
