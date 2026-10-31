//! The 6502 description files against the parsers that read them. Expected
//! values are worked out by hand from the rules in the file comments, not read
//! back from the code.

use llrm_6502::{CALLING, LAYOUT, REGISTERS};
use llrm_target::calling::{Calling, Kind, Place};
use llrm_target::layout::Layout;
use llrm_target::registers;

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
fn fifteen_registers_hold_values_and_every_class_is_known() {
    let file = registers::parse(REGISTERS).expect("registers.regs parses");
    // a, x, y and the pairs rs1..rs12; rs0 is the soft stack pointer.
    assert_eq!(registers::allocatable(&file), 15);
    registers::source(REGISTERS, "llrm_lir::registers::RegId", &llrm_lir::registers::CLASSES, "rows")
        .expect("every class is in llrm-lir's list");
}

/// An i8 takes the first free byte, an i16 the first free pair: f(i8, i16, i8)
/// is rc2, rc4:rc5, rc3.
#[test]
fn arguments_fill_bytes_and_pairs_from_rc2() {
    let calling = Calling::parse(CALLING).expect("calling.toml parses");
    let placed = calling.native().place(&[Kind::Word, Kind::Wide, Kind::Word]);
    assert_eq!(placed.places, [registers(&["rc2"]), registers(&["rc4", "rc5"]), registers(&["rc3"])]);
    assert_eq!(placed.stack_bytes, 0);
}

/// Three pairs fit rc2..rc7; the fourth i16 goes to the static frame, two
/// bytes.
#[test]
fn a_fourth_word_argument_goes_to_the_frame() {
    let calling = Calling::parse(CALLING).expect("calling.toml parses");
    let placed = calling.native().place(&[Kind::Wide; 4]);
    assert_eq!(
        placed.places,
        [registers(&["rc2", "rc3"]), registers(&["rc4", "rc5"]), registers(&["rc6", "rc7"]), Place::Stack(0)]
    );
    assert_eq!(placed.stack_bytes, 2);
}

#[test]
fn results_leave_in_a_then_x_then_rc2() {
    let calling = Calling::parse(CALLING).expect("calling.toml parses");
    let one = |width| calling.native().result_registers(width).expect("a result register").to_vec();
    assert_eq!(one(1), ["a"]);
    assert_eq!(one(2), ["a", "x"]);
    assert_eq!(one(4), ["a", "x", "rc2", "rc3"]);
}

/// Without a `@bytes` line greedy had no byte lanes for the zero-page pairs:
/// every byte value took a whole pair root. The line names the hardware
/// registers first, then rc2..rc25; rc0/rc1 are the soft stack pointer.
#[test]
fn the_byte_order_names_a_x_y_then_the_zero_page_bytes() {
    let names: Vec<_> = llrm_6502::REGISTER_INFO.bytes.iter().filter_map(|r| llrm_6502::REGISTER_INFO.name(*r)).collect();
    assert_eq!(&names[..3], ["a", "x", "y"]);
    assert_eq!(names.len(), 3 + 24);
    assert!(!names.contains(&"rc0") && !names.contains(&"rc1"), "{names:?}");
}
