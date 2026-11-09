//! `reachable_runtime` takes the routines a text names. `memcpy` ends where
//! `memcpy_body` begins, which nothing names, so the copy loop was left out
//! and every program that called `memcpy` ran a two-instruction routine
//! (`stack_arrays_copy` printed -101, wanted 303).

mod common;

use common::pipeline::reachable_runtime;

#[test]
fn a_routine_that_runs_on_into_the_next_label_takes_its_body() {
    let text = reachable_runtime("  jsr memcpy\n");
    assert!(text.contains("memcpy_body:") && text.contains("lda (rc4),y"), "the copy loop is missing:\n{text}");
}
