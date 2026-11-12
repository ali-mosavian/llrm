//! A two-instruction loop header is copied into the latch, so a trip runs no
//! unconditional branch (`fib` -O1: `b head` on every trip, where the pin's
//! `subs; b.ge body` closes the loop).

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
fn a_loop_whose_test_follows_an_unconditional_entry_runs_no_jump_per_trip() {
    let entry = block(".Lf.0", vec![Inst::Nop, Inst::Branch { link: false, target: label(".Lf.1") }]);
    let head = block(
        ".Lf.1",
        vec![
            Inst::Nop,
            Inst::CompareBranch { nonzero: false, width: Width::X, rt: Gpr(0), target: label(".Lf.3") },
            Inst::Branch { link: false, target: label(".Lf.2") },
        ],
    );
    let body = block(".Lf.2", vec![Inst::Nop, Inst::Nop, Inst::Branch { link: false, target: label(".Lf.1") }]);
    let exit = block(".Lf.3", vec![Inst::Ret { rn: Gpr(30) }]);
    let mut odds = Odds::default();
    odds.insert((0, 1), (BranchProbability::get_one(), 0));
    odds.insert((1, 3), (BranchProbability::new(1, 32), 0));
    odds.insert((1, 2), (BranchProbability::new(31, 32), 1));
    odds.insert((2, 1), (BranchProbability::get_one(), 0));
    let out = fold(vec![entry, head, body, exit], &mut [], &odds, false, &[]);
    let in_loop = |name: &str| name == ".Lf.1" || name == ".Lf.2";
    let jumps = out
        .iter()
        .filter(|block| in_loop(&block.label))
        .flat_map(|block| &block.insts)
        .filter(|inst| matches!(inst, Inst::Branch { link: false, .. }))
        .count();
    assert_eq!(jumps, 0, "{out:#?}");
}

/// A forward jump to a test is not a loop's: the copy would grow the code with
/// no taken branch saved (jpeg -O1: +72 B for one instruction fewer).
#[test]
fn a_test_reached_by_a_forward_jump_is_not_copied() {
    let jump = |to: &str| Inst::Branch { link: false, target: label(to) };
    let mut blocks = vec![
        block(".Lf.0", vec![Inst::Nop, Inst::Ret { rn: Gpr(30) }]),
        block(".Lf.1", vec![Inst::Nop, Inst::Nop, jump(".Lf.3")]),
        block(".Lf.2", vec![Inst::Nop, Inst::Nop, Inst::Nop, Inst::Ret { rn: Gpr(30) }]),
        block(
            ".Lf.3",
            vec![
                Inst::Nop,
                Inst::CompareBranch { nonzero: false, width: Width::X, rt: Gpr(0), target: label(".Lf.5") },
                jump(".Lf.4"),
            ],
        ),
        block(".Lf.4", vec![Inst::Nop, Inst::Ret { rn: Gpr(30) }]),
        block(".Lf.5", vec![Inst::Ret { rn: Gpr(30) }]),
    ];
    assert_eq!(llrm_arm64::taildup::tail_duplicate(&mut blocks, false), vec![]);
}

/// The same test reached by a jump back to it (a loop's latch) is copied.
#[test]
fn a_test_reached_by_a_jump_back_is_copied() {
    let jump = |to: &str| Inst::Branch { link: false, target: label(to) };
    let mut blocks = vec![
        block(".Lf.0", vec![Inst::Nop, Inst::Ret { rn: Gpr(30) }]),
        block(
            ".Lf.3",
            vec![
                Inst::Nop,
                Inst::CompareBranch { nonzero: false, width: Width::X, rt: Gpr(0), target: label(".Lf.5") },
                jump(".Lf.4"),
            ],
        ),
        block(".Lf.1", vec![Inst::Nop, Inst::Nop, jump(".Lf.3")]),
        block(".Lf.4", vec![Inst::Nop, Inst::Ret { rn: Gpr(30) }]),
        block(".Lf.5", vec![Inst::Ret { rn: Gpr(30) }]),
    ];
    let copies = llrm_arm64::taildup::tail_duplicate(&mut blocks, false);
    assert_eq!(copies, vec![(".Lf.1".to_owned(), ".Lf.3".to_owned())]);
}
