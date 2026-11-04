//! The Z80 description files against the parsers that read them. Expected
//! values are worked out by hand from the rules in the file comments, not read
//! back from the code.

use llrm_target::calling::{Calling, Kind, Place};
use llrm_target::layout::Layout;
use llrm_target::registers;
use llrm_z80::{CALLING, LAYOUT, REGISTERS};

fn registers(names: &[&str]) -> Place {
    Place::Registers(names.iter().map(|n| n.to_string()).collect())
}

#[test]
fn pointers_are_two_bytes_near_in_an_8_bit_machine() {
    let layout = Layout::parse(LAYOUT).expect("datalayout.toml parses");
    assert_eq!(layout.mode, 8);
    assert_eq!(layout.spaces.unmarked(2), Ok(layout.spaces.near));
}

#[test]
fn four_registers_hold_values_and_every_class_is_known() {
    let file = registers::parse(REGISTERS).expect("registers.regs parses");
    // a, bc, de, hl and iy; ix is the frame, sp the stack.
    assert_eq!(registers::allocatable(&file), 5);
    registers::source(REGISTERS, "llrm_lir::registers::RegId", &llrm_lir::registers::CLASSES, "rows")
        .expect("every class is in llrm-lir's list");
}

/// A byte takes the first free of `a l h e d c b`, a word the first free pair
/// of `hl de bc`: f(u8, u16, u8) is a, hl, e.
#[test]
fn arguments_fill_bytes_then_pairs() {
    let calling = Calling::parse(CALLING).expect("calling.toml parses");
    let placed = calling.native().place(&[Kind::Word, Kind::Wide, Kind::Word]);
    assert_eq!(placed.places, [registers(&["a"]), registers(&["l", "h"]), registers(&["e"])]);
    assert_eq!(placed.stack_bytes, 0);
}

/// Three pairs fit hl, de, bc; the fourth word goes to the stack, two bytes.
#[test]
fn a_fourth_word_argument_goes_to_the_stack() {
    let calling = Calling::parse(CALLING).expect("calling.toml parses");
    let placed = calling.native().place(&[Kind::Wide; 4]);
    assert_eq!(
        placed.places,
        [registers(&["l", "h"]), registers(&["e", "d"]), registers(&["c", "b"]), Place::Stack(0)]
    );
    assert_eq!(placed.stack_bytes, 2);
}

#[test]
fn results_leave_in_a_then_de_then_hlde() {
    let calling = Calling::parse(CALLING).expect("calling.toml parses");
    let one = |width| calling.native().result_registers(width).expect("a result register").to_vec();
    assert_eq!(one(1), ["a"]);
    assert_eq!(one(2), ["e", "d"]);
    assert_eq!(one(4), ["e", "d", "l", "h"]);
}

#[test]
fn the_byte_order_names_b_c_d_e_h_l_then_a() {
    let names: Vec<_> = llrm_z80::REGISTER_INFO.bytes.iter().filter_map(|r| llrm_z80::REGISTER_INFO.name(*r)).collect();
    assert_eq!(names, ["b", "c", "d", "e", "h", "l", "a"]);
}
