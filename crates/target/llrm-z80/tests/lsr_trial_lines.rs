//! The second compile of a module (LSR-KEY-TRIAL) allocated, expanded and
//! priced every function again. A function the mid-end makes as it did the
//! first time keeps its lines.

use llrm_z80::compile::{effective, module_priced, take_lines_made};
use llrm_z80::driver::{Level, Request};

#[test]
fn a_function_the_twin_compile_leaves_alone_is_not_made_again() {
    let ir = format!(
        "{}\ndefine i16 @plain(i16 %a) {{\n  %b = add i16 %a, 1\n  ret i16 %b\n}}\n",
        include_str!("data/histo_o1.ll")
    );
    let base = Request::new(Level::O1);
    module_priced(&ir, &base).expect("prices");
    let first = take_lines_made();
    let chosen = effective(&ir, &base).expect("chooses");
    let twin = take_lines_made();
    assert!(chosen.lsr_alternate, "histo no longer takes the alternate order");
    assert!(first.iter().any(|name| name == "plain"), "{first:?}");
    assert!(!twin.iter().any(|name| name == "plain" || name == "main"), "the twin made {twin:?}");
}
