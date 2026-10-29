//! `ir FILE`: the IR after the m68k mid-end.
fn main() {
    let source = std::fs::read_to_string(std::env::args().nth(1).unwrap()).unwrap();
    let module = llrm_mir::parse::module(&source).unwrap();
    print!("{}", llrm_mir::print::module(&llrm_m68k::compile::mid_end(&module).unwrap()));
}
