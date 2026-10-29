//! `pinned FILE FUNCTION [mid]`: the function's asm without the overlay (for
//! `llvm-mc`), with the mid-end if a third argument is given.
use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
fn main() {
    let source = std::fs::read_to_string(std::env::args().nth(1).unwrap()).unwrap();
    let module = llrm_mir::parse::module(&source).unwrap();
    let options = Options { mid_end: std::env::args().nth(3).is_some(), ..Options::PINNED };
    print!("{}", text_with(&module, &std::env::args().nth(2).unwrap(), Cpu::M68000, options).unwrap());
}
