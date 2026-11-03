//! Block layout inverts a compare-and-branch over a jump, as it does a
//! conditional one.

use llrm_arm64::frame::Lowered;
use llrm_arm64::layout::fold;
use llrm_arm64::mc::inst::{Gpr, Inst, Target, Width};

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

/// A loop that ended `cbz x0, exit; b top` (a `cbz` over a jump to the top, 4 B
/// and one instruction a trip: grep's scan loop, 812 B against clang's 717).
/// It ends `cbnz x0, top`.
#[test]
fn a_compare_branch_over_a_jump_is_the_inverse_compare_branch() {
    for nonzero in [false, true] {
        let top = block(
            "top",
            vec![
                Inst::CompareBranch { nonzero, width: Width::X, rt: Gpr(0), target: label("exit") },
                Inst::Branch { link: false, target: label("top") },
            ],
        );
        let exit = block("exit", vec![Inst::Ret { rn: Gpr(30) }]);
        let out = fold(vec![top, exit], &mut []);
        assert!(
            matches!(
                out[0].insts.as_slice(),
                [Inst::CompareBranch { nonzero: flipped, target: Target::Label(to), .. }] if *flipped != nonzero && to == "top"
            ),
            "{:?}",
            out[0].insts
        );
    }
}

/// The same for a test-bit branch.
#[test]
fn a_test_branch_over_a_jump_is_the_inverse_test_branch() {
    let top = block(
        "top",
        vec![
            Inst::TestBranch { nonzero: false, rt: Gpr(0), bit: 3, target: label("exit") },
            Inst::Branch { link: false, target: label("top") },
        ],
    );
    let exit = block("exit", vec![Inst::Ret { rn: Gpr(30) }]);
    let out = fold(vec![top, exit], &mut []);
    assert!(
        matches!(
            out[0].insts.as_slice(),
            [Inst::TestBranch { nonzero: true, bit: 3, target: Target::Label(to), .. }] if to == "top"
        ),
        "{:?}",
        out[0].insts
    );
}
