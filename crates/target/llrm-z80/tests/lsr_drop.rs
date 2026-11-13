//! DEVIATE z80-lsr-drop: LSR's rewrite is dropped where its own cost model
//! rates it worse than the loop as written.

use llrm_z80::compile::{prepare, text_in};
use llrm_z80::driver::{Level, Request};

/// switch_empty_cases: `seen = seen * 3 + i` six times. At -Os LSR rewrote the
/// loop into a count-down counter and kept the accumulator's previous value for
/// the exit (4 registers, 4 instructions against 3 and 3 as written): `main`
/// took a frame (`push ix`) and spilled, 74 B against 40 B at -Oz.
const IR: &str = include_str!("data/lsr_rewrite_worse.ll");

/// builtins_table's `strlen`: `while (s[n]) n++` returning `n`. LSR's pointer
/// plus count looks worse than the baseline's one register; dropping it kept `s
/// + n` and spilled `s` to a frame (+17 B at every level).
const ADDRESS_IR: &str = include_str!("data/lsr_address_loop.ll");

fn function_text(
    ir: &str,
    function: &str,
    level: Level,
) -> String {
    let request = Request::new(level);
    let prepared = prepare(ir, &request).expect("prepares");
    let (number, name) = prepared.names().enumerate().find(|(_, name)| *name == function).expect("function");
    text_in(&prepared, name, number, &request).expect("compiles")
}

fn main_text(level: Level) -> String {
    function_text(IR, "main", level)
}

#[test]
fn a_loop_lsr_would_make_worse_is_kept_as_written() {
    for level in [Level::O1, Level::Os] {
        let text = main_text(level);
        assert!(!text.contains("(ix)"), "{level:?}: the rewrite spilled to the frame:\n{text}");
        assert!(text.lines().count() <= 30, "{level:?}: {} lines:\n{text}", text.lines().count());
    }
}

#[test]
fn a_loop_with_an_address_keeps_its_pointer() {
    for level in [Level::O2, Level::Os] {
        let text = function_text(ADDRESS_IR, "strlen", level);
        assert!(
            !text.contains("(ix)") && !text.contains("push ix"),
            "{level:?}: the pointer was dropped for a frame:\n{text}"
        );
    }
}
