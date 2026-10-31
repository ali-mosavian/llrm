use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

/// `asm FILE FUNCTION [pinned|raw|size]`: the function's assembly.
fn main() {
    let text = std::fs::read_to_string(std::env::args().nth(1).unwrap()).unwrap();
    let module = llrm_mir::parse::module(&text).unwrap();
    let options = match std::env::args().nth(3).as_deref() {
        Some("pinned") => Options::PINNED,
        Some("raw") => Options { mid_end: false, ..Options::default() },
        Some("size") => Options { size: true, ..Options::default() },
        _ => Options::default(),
    };
    print!(
        "{}",
        text_with(
            &module,
            &std::env::args().nth(2).unwrap(),
            if std::env::var("CPU").is_ok_and(|c| c == "m68020") { Cpu::M68020 } else { Cpu::M68000 },
            options
        )
        .unwrap()
    );
}
