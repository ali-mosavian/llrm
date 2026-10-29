use llrm_m68k::dagisel::hooks::Cpu;
fn main() {
    let text = std::fs::read_to_string(std::env::args().nth(1).unwrap()).unwrap();
    let module = llrm_mir::parse::module(&text).unwrap();
    print!("{}", llrm_m68k::compile::text(&module, &std::env::args().nth(2).unwrap(), Cpu::M68000).unwrap());
}
