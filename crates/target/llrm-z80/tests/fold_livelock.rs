//! Branch folding (`BranchFolder::OptimizeBranches`) reaches a fixed layout.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

/// `known_loads_walks` -O3 never finished (over 600 s; -O2 took 1.5 s).
/// `canFallThrough` ignored a jump to the next block, so two blocks at the
/// layout's tail moved before one another forever (`OptimizeBlock` rules 3 and
/// 4). With the jump counted it takes 1.3 s.
#[test]
fn a_jump_to_the_next_block_does_not_make_folding_loop_forever() {
    let (done, wait) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let asm = text(include_str!("kernels/ir/fold_livelock.ll"), "f", 0, &Request::new(Level::O3));
        let _ = done.send(asm.is_ok());
    });
    let compiled = wait.recv_timeout(std::time::Duration::from_secs(60)).expect("branch folding does not return");
    assert!(compiled);
}
