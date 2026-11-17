//! A run of copies of adjacent cells is `memcpy` at every level: bytes first.

use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{self, Level, Request};
use llrm_6502::target::Mos;
use llrm_target::Target;

/// copy_runs -O1..-O3: the 96-byte run stayed 96 loads and stores, 2,106 B
/// against 1,392 at -Os (sdcc 2,027, llvm-mos 2,010): the run was a call only
/// at -Os.
#[test]
fn a_long_run_of_copies_is_a_memcpy_at_every_level() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data_size/copy_runs.ll");
    let text = std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|line| !line.starts_with("target datalayout"))
        .fold(format!("target datalayout = \"{}\"\n", Mos.layout().datalayout), |text, line| text + line + "\n");
    for level in [Level::O1, Level::O2, Level::O3, Level::Os] {
        let mut request = Request::new(level);
        request.cpu = Cpu::Mos6502;
        let assembly = driver::assembly(&text, &request).unwrap_or_else(|why| panic!("{level:?}: {why}"));
        assert!(assembly.contains("jsr memcpy"), "{level:?}: no memcpy call\n{assembly}");
    }
}
