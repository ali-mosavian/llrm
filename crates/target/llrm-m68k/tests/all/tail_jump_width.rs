//! A tail call to a function of the same object was a 16-bit `bra` (`braw`, 4
//! B), 2 B over gcc's 8-bit one (`sibling_call_callees` -Os, 68000/68020/68040:
//! two tail jumps, +4 B in the object). A branch to a label the object
//! defines starts at its 8-bit form and takes the 16-bit one only where the
//! displacement does not fit.

use llrm_m68k::compile::Item;
use llrm_m68k::elf::object;
use llrm_m68k::mc::{Inst, Operand};

fn text_of(items: &[Item]) -> Vec<u8> {
    let object = object("t", items).expect("assembles");
    object.sections.iter().find(|section| section.name == ".text").expect("a .text section").image.clone()
}

fn jump_to_f() -> Item {
    Item::Inst(Inst { opcode: "BRA16", operands: vec![Operand::Symbol("f".to_owned(), 0)] })
}

fn rts() -> Item {
    Item::Inst(Inst { opcode: "RTS", operands: vec![] })
}

#[test]
fn a_tail_jump_to_a_function_of_the_object_is_the_8_bit_branch() {
    let items = vec![Item::Label("g".to_owned()), jump_to_f(), rts(), Item::Label("f".to_owned()), rts()];
    let text = text_of(&items);
    assert_eq!(text, [0x60, 0x02, 0x4e, 0x75, 0x4e, 0x75], "bra.b over the rts, then both rts");
}

#[test]
fn a_tail_jump_past_127_bytes_keeps_the_16_bit_branch() {
    let mut items = vec![Item::Label("g".to_owned()), jump_to_f()];
    items.extend(std::iter::repeat_with(rts).take(100));
    items.extend([Item::Label("f".to_owned()), rts()]);
    let text = text_of(&items);
    assert_eq!(&text[..2], [0x60, 0x00], "the 16-bit form: a zero byte displacement, the word follows");
    assert_eq!(text.len(), 4 + 200 + 2);
}
