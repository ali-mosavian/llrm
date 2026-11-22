//! The multiply trial wrote the text of the selection it kept twice: once to
//! price it (`mulimage::expand_when_unshared`) and again in `module_text`, the
//! allocator and the peephole passes running on both. A selection's text is
//! written once while the module is in hand.

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};
use llrm_6502::select::text_compiles;

/// `x * 11` has two selections, the call of `__mulsi3` and the shifts and
/// adds; the second is kept. Writing each once is 2 texts (3 before the cache);
/// its two dead carry saves compile it again with them dropped after allocation
/// (`save_drop_trial`), 4.
#[test]
fn the_kept_selection_of_a_multiply_trial_is_written_once() {
    let ir = format!(
        "target datalayout = \"{}\"\ndefine i32 @by_k(i32 %x) nounwind {{\n  %r = mul i32 %x, 11\n  ret i32 %r\n}}\n",
        datalayout()
    );
    let before = text_compiles();
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    assert!(!text.contains("__mulsi3"), "the shifts were not kept:\n{text}");
    assert_eq!(text_compiles() - before, 4, "texts written for one function");
}
