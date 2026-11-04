//! DEVIATE HC1: `HoistCommonCodeInSuccs` looks a few independent instructions
//! past the head for the twin of the other successor's head. crc16's guard
//! left `move.l #65535,%d0` on both sides (the preheader starts with the
//! argument load): 6 B and a branch more than gcc.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

#[test]
fn a_constant_both_successors_need_is_loaded_once() {
    let source = std::fs::read_to_string(format!("{}/bench/library/crc16.ll", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let module = llrm_mir::parse::module(&source).expect("parses");
    let text = text_with(&module, "k", Cpu::M68000, Options { size: true, ..Options::default() }).expect("compiles");
    let loads = text.lines().filter(|line| line.trim() == "move.l\t#65535, %d0").count();
    assert_eq!(loads, 1, "{text}");
}
