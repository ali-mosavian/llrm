//! Block placement rotates a loop so its test sits at the bottom. Before,
//! `head: cbz exit; body: nop; b head` ran an unconditional branch every trip
//! (queens O3: one extra `b` per outer row).

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
fn a_loop_runs_no_unconditional_branch_per_trip() {
    let entry = block(
        ".Lf.0",
        vec![
            Inst::CompareBranch { nonzero: false, width: Width::X, rt: Gpr(1), target: label(".Lf.4") },
            Inst::Branch { link: false, target: label(".Lf.1") },
        ],
    );
    let head = block(
        ".Lf.1",
        vec![
            Inst::CompareBranch { nonzero: false, width: Width::X, rt: Gpr(0), target: label(".Lf.3") },
            Inst::Branch { link: false, target: label(".Lf.2") },
        ],
    );
    let body = block(".Lf.2", vec![Inst::Nop, Inst::Branch { link: false, target: label(".Lf.1") }]);
    let exit = block(".Lf.3", vec![Inst::Ret { rn: Gpr(30) }]);
    let mut odds = Odds::default();
    odds.insert((0, 4), (BranchProbability::new(9, 10), 0));
    odds.insert((0, 1), (BranchProbability::new(1, 10), 1));
    odds.insert((1, 2), (BranchProbability::new(31, 32), 1));
    odds.insert((1, 3), (BranchProbability::new(1, 32), 0));
    let early = block(".Lf.4", vec![Inst::Ret { rn: Gpr(29) }]);
    let out = fold(vec![entry, head, body, exit, early], &mut [], &odds, false, &[]);
    let in_loop = |name: &str| name == ".Lf.1" || name == ".Lf.2";
    let jumps = out
        .iter()
        .filter(|block| in_loop(&block.label))
        .flat_map(|block| &block.insts)
        .filter(|inst| matches!(inst, Inst::Branch { link: false, .. }))
        .count();
    assert_eq!(jumps, 0, "{out:?}");
}

/// The pin's placement puts the latch before the header, which costs the swap
/// path of `quicksort`'s inner loop a `b` (122481 -> 128134 instructions).
/// The old chain runs none; it stays unless placement executes fewer jumps.
#[test]
fn placement_that_adds_a_jump_to_a_hot_path_is_not_taken() {
    let entry = block(
        ".Lf.0",
        vec![
            Inst::CompareBranch { nonzero: false, width: Width::X, rt: Gpr(2), target: label(".Lf.5") },
            Inst::Branch { link: false, target: label(".Lf.1") },
        ],
    );
    let early = block(".Lf.5", vec![Inst::Ret { rn: Gpr(29) }]);
    let head = block(
        ".Lf.1",
        vec![
            Inst::CompareBranch { nonzero: false, width: Width::X, rt: Gpr(0), target: label(".Lf.3") },
            Inst::Branch { link: false, target: label(".Lf.2") },
        ],
    );
    let swap = block(".Lf.2", vec![Inst::Nop, Inst::Branch { link: false, target: label(".Lf.3") }]);
    let latch = block(
        ".Lf.3",
        vec![
            Inst::Nop,
            Inst::Nop,
            Inst::CompareBranch { nonzero: true, width: Width::X, rt: Gpr(1), target: label(".Lf.1") },
            Inst::Branch { link: false, target: label(".Lf.4") },
        ],
    );
    let exit = block(".Lf.4", vec![Inst::Ret { rn: Gpr(30) }]);
    let mut odds = Odds::default();
    odds.insert((0, 5), (BranchProbability::new(9, 10), 0));
    odds.insert((0, 1), (BranchProbability::new(1, 10), 1));
    odds.insert((1, 3), (BranchProbability::new(1, 2), 0));
    odds.insert((1, 2), (BranchProbability::new(1, 2), 1));
    odds.insert((3, 1), (BranchProbability::new(31, 32), 0));
    odds.insert((3, 4), (BranchProbability::new(1, 32), 1));
    let out = fold(vec![entry, head, swap, latch, exit, early], &mut [], &odds, false, &[]);
    let jumps = out
        .iter()
        .filter(|block| [".Lf.1", ".Lf.2", ".Lf.3"].contains(&block.label.as_str()))
        .flat_map(|block| &block.insts)
        .filter(|inst| matches!(inst, Inst::Branch { link: false, .. }))
        .count();
    assert_eq!(jumps, 0, "{out:?}");
}
