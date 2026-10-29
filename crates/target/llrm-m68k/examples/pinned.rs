//! `pinned FILE FUNCTION`: the function's asm without the overlay, for
//! `llvm-mc`.
use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
fn main() {
    let source = std::fs::read_to_string(std::env::args().nth(1).unwrap()).unwrap();
    let module = llrm_mir::parse::module(&source).unwrap();
    print!("{}", text_with(&module, &std::env::args().nth(2).unwrap(), Cpu::M68000, Options::PINNED).unwrap());
}
