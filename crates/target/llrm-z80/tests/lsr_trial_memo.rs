//! The second compile of a module (LSR-KEY-TRIAL) selected every function
//! again: +40% compile instructions at -O1 on `tests/run/c`, 180 programs. A
//! function the mid-end leaves as it was keeps the selection of the first
//! compile.

use llrm_z80::compile::{effective, module_priced};
use llrm_z80::driver::{Level, Request, take_selected_names};

#[test]
fn a_function_the_twin_compile_leaves_alone_is_selected_once() {
    let ir = format!(
        "{}\ndefine i16 @plain(i16 %a) {{\n  %b = add i16 %a, 1\n  ret i16 %b\n}}\n",
        include_str!("data/histo_o1.ll")
    );
    let base = Request::new(Level::O1);
    module_priced(&ir, &base).expect("prices");
    take_selected_names();
    let chosen = effective(&ir, &base).expect("chooses");
    let twin = take_selected_names();
    assert!(chosen.lsr_alternate, "histo no longer takes the alternate order");
    assert!(twin.iter().all(|name| name == "bench_histo") && !twin.is_empty(), "the twin selected {twin:?}");
}
