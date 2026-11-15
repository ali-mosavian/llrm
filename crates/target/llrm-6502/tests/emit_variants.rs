//! A function with spills and calls ran one more whole emission to try
//! "splitting" a register across the calls, though the run took no such
//! argument any more: it gave the text already in hand, and was never kept.

use llrm_6502::alloc::emit_runs;
use llrm_6502::driver::{Level, Request, assembly};

/// The file's two functions spill across calls and share no compare: the
/// first emission of each, one more each for the layout it tries, and one more
/// each for the function without the sink, which now moves something in both
/// (4 before the sink took copies and steps). A splitting run would make it 8.
#[test]
fn a_function_with_spills_and_calls_pays_for_no_variant_that_changes_nothing() {
    let before = emit_runs();
    let text = assembly(include_str!("data/recmany_o1.ll"), &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    assert!(text.contains("bench_recmany:"), "{text}");
    let runs = emit_runs() - before;
    assert_eq!(runs, 6, "emissions of the functions of the file");
}
