use llrm_wasm::Width;
use llrm_wasm::dag::Options;
use llrm_wasm::driver::{Level, assembly};

/// The tail merge dropped a block's branch back to itself from its successor
/// list when it replaced the unconditional `br` at the end with a jump to the
/// shared tail: the block looked like it had one successor, the block
/// removal left a `br_if` to a block that was gone and `fold_branches`
/// indexed out of bounds (sieve -O1 failed to compile). With the edges right
/// the tail is shared (595 B before the pass, 562 after).
#[test]
fn a_loop_block_keeps_its_back_edge_when_its_tail_is_shared() {
    let ir = include_str!("ir/sieve_tail_merge.ll");
    let text = assembly(ir, Width::W32, Options { level: Level::O1, ..Options::default() }).unwrap();
    assert!(text.contains("bench_sieve:"), "{text}");
}
