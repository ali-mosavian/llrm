//! A function emitted more than once (a layout, a sink, a callee-save choice)
//! ran the register allocator again on the very function it had just
//! allocated: 9 emissions of `bench_recmany`'s file allocated 9 times. The
//! file's emissions are now 12 (see `emit_variants`).

use llrm_6502::alloc::{allocations, emit_runs};
use llrm_6502::driver::{Level, Request, assembly};

#[test]
fn a_function_emitted_again_is_allocated_again_only_when_it_changed() {
    let (runs, made) = (emit_runs(), allocations());
    let text = assembly(include_str!("data/recmany_o1.ll"), &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    assert!(text.contains("bench_recmany:"), "{text}");
    let (runs, made) = (emit_runs() - runs, allocations() - made);
    assert_eq!(runs, 12, "emissions of the functions of the file");
    // the 4 inputs of the plain and layout emissions, and one each for
    // `bench_recmany` and `main` without the wide commute
    assert_eq!(made, 6, "allocations of them");
}
