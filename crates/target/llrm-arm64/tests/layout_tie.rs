//! When the pin's placement and the selector's order execute the same jumps,
//! the pin's order is kept. `recmany` -O1 kept the selector's (entry, test,
//! exit, body), which ends the loop in `cbnz` plus `b exit`: 4 B and an
//! instruction over clang's.

use llrm_arm64::frame::Lowered;
use llrm_arm64::layout::fold;
use llrm_arm64::mc::inst::{Gpr, Inst, Target, Width};
use llrm_mir::blockfrequency::BranchProbability;
use llrm_spike_mf::placement::Odds;

fn label(name: &str) -> Target {
    Target::Label(name.to_owned())
}

fn block(
    name: &str,
    insts: Vec<Inst>,
) -> Lowered {
    let locs = vec![None; insts.len()];
    Lowered { label: name.to_owned(), insts, locs }
}

#[test]
fn equal_jump_counts_keep_the_pins_order() {
    let entry = block(".Lf.0", vec![Inst::Nop, Inst::Branch { link: false, target: label(".Lf.1") }]);
    let test = block(
        ".Lf.1",
        vec![
            Inst::CompareBranch { nonzero: true, width: Width::W, rt: Gpr(5), target: label(".Lf.3") },
            Inst::Branch { link: false, target: label(".Lf.2") },
        ],
    );
    let exit = block(".Lf.2", vec![Inst::Nop, Inst::Ret { rn: Gpr(30) }]);
    let body = block(".Lf.3", vec![Inst::Nop, Inst::Nop, Inst::Branch { link: false, target: label(".Lf.1") }]);
    let mut odds = Odds::default();
    odds.insert((0, 1), (BranchProbability::get_one(), 0));
    odds.insert((1, 2), (BranchProbability::new(67108864, 1 << 31), 0));
    odds.insert((1, 3), (BranchProbability::new(2080374784, 1 << 31), 1));
    odds.insert((3, 1), (BranchProbability::get_one(), 0));
    let out = fold(vec![entry, test, exit, body], &mut [], &odds, false, &[]);
    let order: Vec<&str> = out.iter().map(|block| block.label.as_str()).collect();
    // #9699 (TailDupPlacement): the test is merged into the entry and copied into the body, so the loop is `.Lf.3` alone;
    // what the old order guarded still holds: the loop ends in the `cbnz` back edge and falls into the exit, no `b exit`.
    assert_eq!(order, [".Lf.0", ".Lf.3", ".Lf.2"], "{out:#?}");
    let body = &out[1];
    assert!(
        matches!(body.insts.last(), Some(Inst::CompareBranch { nonzero: true, target: Target::Label(back), .. }) if back == ".Lf.3"),
        "{out:#?}"
    );
    assert!(out.iter().all(|block| !block.insts.iter().any(|inst| matches!(inst, Inst::Branch { .. }))), "{out:#?}");
}
