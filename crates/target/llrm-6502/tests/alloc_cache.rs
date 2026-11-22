//! A function emitted more than once (a layout, a sink, a callee-save choice)
//! ran the register allocator again on the very function it had just
//! allocated: 9 emissions of `bench_recmany`'s file allocated 9 times. The
//! file's emissions are now 16 (see `emit_variants`): four more
//! emissions: the functions that now save through `__csave_ax` are each
//! written once more inline to price the helpers. That writes the text again,
//! not the allocation: `allocations` below is unchanged. The module is also
//! compiled again with the carry chain for its two long steps
//! (`long_step_trial`), which it keeps: 16 more, 32 in all, and as many
//! allocations again.

use llrm_6502::alloc::{allocations, emit_runs};
use llrm_6502::driver::{Level, Request, assembly};

#[test]
fn a_function_emitted_again_is_allocated_again_only_when_it_changed() {
    let (runs, made) = (emit_runs(), allocations());
    let text = assembly(include_str!("data/recmany_o1.ll"), &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    assert!(text.contains("bench_recmany:"), "{text}");
    let (runs, made) = (emit_runs() - runs, allocations() - made);
    assert_eq!(runs, 32, "emissions of the functions of the file");
    // the 4 inputs of the plain and layout emissions, and one each for
    // `bench_recmany` and `main` without the wide commute
    // doubled by the second compile (`long_step_trial`), whose selections are
    // new ones
    assert_eq!(made, 12, "allocations of them");
}
