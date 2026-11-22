//! A function with spills and calls ran one more whole emission to try
//! "splitting" a register across the calls, though the run took no such
//! argument any more: it gave the text already in hand, and was never kept.

use llrm_6502::alloc::emit_runs;
use llrm_6502::driver::{Level, Request, assembly};

/// The file's two functions spill across calls and share no compare: the
/// first emission of each, one more each for the layout it tries, and one more
/// each for the function without the sink, which now moves something in both
/// (4 before the sink took copies and steps). A splitting run would make it 8.
///
/// `main` saves 8 cells, so it is written a second time with them saved inline
/// (`module_text`): 3 more emissions, 9 in all. That one is paid for: inline
/// is 19 B larger than `__csave`/`__crest`, which cost 34 B linked once, so the
/// module is 15 B smaller and the inline text is the one kept. It began with
/// #9933 (the allocator prices a byte as at size at every level), which made
/// `main` a user of the helpers; the count was 6 until then.
///
/// The 32-bit remainders pass their results in registers (#10070), so the wide
/// accumulator commute takes a choice in each of the three compiles
/// (`select::function_text`), and each is also emitted without it, once, at
/// the layout and sink already kept: 3 more, 12 in all. That one is paid for:
/// without it `bench_recmany` is 547 B against 549 B. It ran every variant
/// again, 18 in all, until it was priced at the kept ones.
///
/// The functions that save cells with a word in `a` and `x` use `__csave_ax`
/// and `__crest_ax` (they could not use `__csave`), so they are users of the
/// helpers too, each written once more with its cells inline: 4 more, 16 in
/// all.
///
/// The file has two long `x - 1` steps, and the module is compiled again with
/// the carry chain, which the outliner can share (1,254 B against 1,263 B
/// stepped; `long_step_trial`): the same 16 emissions again, 32 in
/// all. That one is paid for: it is the text kept.
#[test]
fn a_function_with_spills_and_calls_pays_for_no_variant_that_changes_nothing() {
    let before = emit_runs();
    let text = assembly(include_str!("data/recmany_o1.ll"), &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    assert!(text.contains("bench_recmany:"), "{text}");
    let runs = emit_runs() - before;
    assert!(!text.contains("__csave"), "the variant was run and not kept:\n{text}");
    assert_eq!(runs, 32, "emissions of the functions of the file");
}
