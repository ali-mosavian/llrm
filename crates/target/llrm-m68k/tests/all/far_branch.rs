//! A 16-bit branch past 32 KiB stopped the 68000 object writer with ".text: a
//! value of 89130 does not fit its 2-byte field" (QB synth.c at -O1/-O2): the
//! 68000 has no 32-bit branch. Every kind takes the far form gas gives
//! `jbra`/`jbcc`: `jmp (xxx).l`, a conditional branch over it with the opposite
//! condition.

use llrm_m68k::compile::Item;
use llrm_m68k::elf::{object, object_with_branches};
use llrm_m68k::mc::{Inst, Operand};
use llrm_support::hash::HashMap;

fn branch(opcode: &'static str) -> Vec<Item> {
    vec![
        Item::Label("g".to_owned()),
        Item::Inst(Inst { opcode, operands: vec![Operand::Symbol("f".to_owned(), 0)] }),
        Item::Bytes(vec![0x4e, 0x71].repeat(20000)),
        Item::Label("f".to_owned()),
        Item::Inst(Inst { opcode: "RTS", operands: vec![] }),
    ]
}

fn text(object: &llrm_object::Object) -> Vec<u8> {
    object.sections.iter().find(|section| section.name == ".text").expect(".text").image.clone()
}

#[test]
fn an_unconditional_branch_past_32_kib_is_a_jump_through_an_absolute_address() {
    let text = text(&object("t", &branch("BRA16")).expect("assembles"));
    assert_eq!(&text[..2], [0x4e, 0xf9], "jmp (xxx).l");
}

#[test]
fn a_conditional_branch_past_32_kib_skips_a_jump_by_the_opposite_condition() {
    let kinds = [
        ("Bhi16", 0x2),
        ("Bls16", 0x3),
        ("Bcc16", 0x4),
        ("Bcs16", 0x5),
        ("Bne16", 0x6),
        ("Beq16", 0x7),
        ("Bvc16", 0x8),
        ("Bvs16", 0x9),
        ("Bpl16", 0xa),
        ("Bmi16", 0xb),
        ("Bge16", 0xc),
        ("Blt16", 0xd),
        ("Bgt16", 0xe),
        ("Ble16", 0xf),
    ];
    for (opcode, condition) in kinds {
        let text = text(&object("t", &branch(opcode)).unwrap_or_else(|why| panic!("{opcode}: {why}")));
        assert_eq!(text[0], 0x60 | (condition ^ 1), "{opcode}: the opposite condition");
        assert_eq!(text[1], 6, "{opcode}: over the six bytes of the jump");
        assert_eq!(&text[2..4], [0x4e, 0xf9], "{opcode}: jmp (xxx).l");
    }
}

#[test]
fn a_cpu_with_a_long_branch_keeps_it() {
    let object = object_with_branches("t", &branch("Beq16"), &HashMap::default(), &[], true).expect("assembles");
    let text = text(&object);
    assert_eq!(&text[..2], [0x67, 0xff], "beq.l: the 32-bit displacement follows");
}
