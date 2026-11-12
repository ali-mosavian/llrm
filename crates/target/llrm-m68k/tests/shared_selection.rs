//! Every trial build of a function selected it again: `c_ushort_3` at -Os on
//! the 68020 ran selection and emission once per build (about ten) although
//! only the shared-CSE shape, the multiply and the carry chain change what
//! they make. The builds share one emission per distinct input.

use llrm_m68k::compile::{Options, items_with, selections_run};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, lowered_for};

const CASE: &str = include_str!("fixtures/div_ushort3_raw.ll");

#[test]
fn trial_builds_of_a_function_share_its_selection() {
    let module = lowered_for(CASE, Cpu::M68020, Level::Os).expect("lowers");
    let before = selections_run();
    let options = Options { size: true, mid_end: false, ..Options::default() };
    items_with(&module, "c_ushort_3", Cpu::M68020, options).expect("compiles");
    let selected = selections_run() - before;
    assert!(selected <= 3, "selection ran {selected} times for one function");
}
