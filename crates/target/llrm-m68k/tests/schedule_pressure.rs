//! `h = h*31 + g(a, b)` 24 times: with the multiply an ALU node (68020) the
//! ILP queue ran every call first and kept the 24 results live, one frame
//! store and load each (select_of_constants -Os 68020: 18532 B, gcc 16990 B).
//! The block is also scheduled in source order when the ILP order holds more
//! than the registers.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const HASH: &str = include_str!("fixtures/hash_calls.ll");

#[test]
fn calls_with_arithmetic_between_them_do_not_all_run_first() {
    let module = llrm_mir::parse::module(HASH).expect("parses");
    let out = text_with(&module, "f", Cpu::M68020, Options { size: true, mid_end: false, ..Options::default() })
        .expect("compiles");
    let stores = out.lines().filter(|l| l.contains("move.l") && l.contains("%d0, (") && l.contains(",%sp)")).count();
    assert!(stores < 8, "{stores} results are stored to the frame:\n{out}");
}
