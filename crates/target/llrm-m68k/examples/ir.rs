//! `ir FILE`: the IR after the m68k mid-end, described as the overlay does
//! (`ir FILE pinned` for the pin's description).
use std::rc::Rc;

use llrm_m68k::machine::{M68k, M68kMachine};

fn main() {
    let source = std::fs::read_to_string(std::env::args().nth(1).unwrap()).unwrap();
    let module = llrm_mir::parse::module(&source).unwrap();
    let overlay = std::env::args().nth(2).as_deref() != Some("pinned");
    let tti = M68k { post_indexed: overlay, ..M68k::default() };
    let optimized = llrm_m68k::compile::mid_end_described_by(&module, &tti, Some(Rc::new(M68kMachine))).unwrap();
    print!("{}", llrm_mir::print::module(&optimized));
}
