//! `BranchRelaxation` (`llvm/lib/CodeGen/BranchRelaxation.cpp`) with the
//! AArch64 hooks: a conditional branch whose label is out of its field's reach
//! becomes the inverted branch over an unconditional `b` to the label.
//!
//! The pin visits the branches in order and moves the later offsets as it goes;
//! each round here finds every out-of-range branch against one set of offsets
//! and rewrites them together. Code only grows, so a branch out of range stays
//! out of range and both reach the same fixed point.
//!
//! Not translated: the unconditional `b` (128 MiB) and its indirect form, and
//! the cold-section trampolines (there is no function splitting).

use crate::frame::Lowered;
use crate::mc::inst::{Inst, Target};

/// The reach of a conditional branch in bytes, in each direction.
fn reach(inst: &Inst) -> Option<(i64, &Target)> {
    match inst {
        Inst::TestBranch { target, .. } => Some((1 << 15, target)),
        Inst::CompareBranch { target, .. } => Some((1 << 20, target)),
        Inst::BranchCond { target, .. } => Some((1 << 20, target)),
        _ => None,
    }
}

fn in_range(
    reach: i64,
    distance: i64,
) -> bool {
    (-reach..reach).contains(&distance)
}

/// `inst` aimed at `target` with the opposite condition
/// (`AArch64InstrInfo::reverseBranchCondition`).
fn inverted(
    inst: &Inst,
    target: Target,
) -> Option<Inst> {
    Some(match inst {
        Inst::TestBranch { nonzero, rt, bit, .. } => Inst::TestBranch { nonzero: !nonzero, rt: *rt, bit: *bit, target },
        Inst::CompareBranch { nonzero, width, rt, .. } => {
            Inst::CompareBranch { nonzero: !nonzero, width: *width, rt: *rt, target }
        }
        Inst::BranchCond { cond, .. } if !matches!(cond, crate::mc::inst::Cond::Al | crate::mc::inst::Cond::Nv) => {
            Inst::BranchCond { cond: cond.inverted(), target }
        }
        _ => return None,
    })
}

fn jump(target: Target) -> Inst {
    Inst::Branch { link: false, target }
}

/// Relaxes the conditional branches of one function's blocks until every
/// branch to a label of the function reaches it.
pub fn run(blocks: &mut [Lowered]) -> Result<(), String> {
    let mut fresh = 0usize;
    loop {
        let at = crate::jumptable::offsets(blocks);
        let mut far: Vec<(usize, usize, bool)> = Vec::new();
        let mut offset = 0i64;
        for (b, block) in blocks.iter().enumerate() {
            let insts = &block.insts;
            for (i, inst) in insts.iter().enumerate() {
                if let Some((limit, Target::Label(label))) = reach(inst)
                    && let Some(&dest) = at.get(label)
                    && !in_range(limit, dest - offset)
                {
                    // `bcc T; b F` with F in reach just swaps the two.
                    let swap = match insts.get(i + 1) {
                        Some(Inst::Branch { link: false, target: Target::Label(f) }) => {
                            at.get(f).is_some_and(|&f| in_range(limit, f - offset))
                        }
                        _ => false,
                    };
                    far.push((b, i, swap));
                }
                if !inst.is_directive() {
                    offset += 4;
                }
            }
        }
        if far.is_empty() {
            return Ok(());
        }
        for &(b, i, swap) in far.iter().rev() {
            let block = &mut blocks[b];
            let Some((_, Target::Label(to))) = reach(&block.insts[i]) else { unreachable!() };
            let to = to.clone();
            let loc = block.locs[i];
            let next = match block.insts.get(i + 1) {
                Some(Inst::Branch { link: false, target: Target::Label(f) }) => Some(f.clone()),
                _ => None,
            };
            if swap {
                let f = next.expect("a swap has the branch after it");
                block.insts[i] = inverted(&block.insts[i], Target::Label(f))
                    .ok_or_else(|| format!("{}: a branch that cannot be inverted is out of range", block.label))?;
                block.insts[i + 1] = jump(Target::Label(to));
            } else if next.as_ref() == Some(&to) {
                // Both edges go to the label: the condition decides nothing.
                block.insts[i] = jump(Target::Label(to));
                block.insts.remove(i + 1);
                block.locs.remove(i + 1);
            } else {
                fresh += 1;
                let skip = format!("{}.relax{fresh}", block.label);
                block.insts[i] = inverted(&block.insts[i], Target::Label(skip.clone()))
                    .ok_or_else(|| format!("{}: a branch that cannot be inverted is out of range", block.label))?;
                block.insts.insert(i + 1, jump(Target::Label(to)));
                block.insts.insert(i + 2, Inst::Label(skip));
                block.locs.insert(i + 1, loc);
                block.locs.insert(i + 2, None);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mc::encode::encode;
    use crate::mc::inst::{AddSub, Cond, Gpr, Width};

    fn nop() -> Inst {
        Inst::AddSubImm {
            op: AddSub { subtract: false, set_flags: false },
            width: Width::X,
            rd: Gpr(1),
            rn: Gpr(1),
            imm12: 0,
            shift12: false,
        }
    }

    fn block(
        label: &str,
        insts: Vec<Inst>,
    ) -> Lowered {
        Lowered { label: label.into(), locs: vec![None; insts.len()], insts }
    }

    fn label(name: &str) -> Target {
        Target::Label(name.into())
    }

    /// `first` at the top, `words` words of code, then `far`.
    fn function(
        first: Vec<Inst>,
        words: usize,
    ) -> Vec<Lowered> {
        vec![block("top", first), block("mid", vec![nop(); words]), block("far", vec![Inst::Ret { rn: Gpr(30) }])]
    }

    /// Every label branch encoded at its distance, as the object builder does:
    /// the first one that does not fit.
    fn first_error(blocks: &[Lowered]) -> Option<String> {
        let at = crate::jumptable::offsets(blocks);
        let mut offset = 0i64;
        for inst in blocks.iter().flat_map(|block| &block.insts) {
            if inst.is_directive() {
                continue;
            }
            let mut inst = inst.clone();
            if let Some((_, Target::Label(name))) = reach(&inst) {
                let delta = at[name] - offset;
                match &mut inst {
                    Inst::TestBranch { target, .. }
                    | Inst::CompareBranch { target, .. }
                    | Inst::BranchCond { target, .. } => *target = Target::Offset(delta),
                    _ => {}
                }
            }
            if let Err(error) = encode(&inst) {
                return Some(error.to_string());
            }
            offset += 4;
        }
        None
    }

    fn test_branch() -> Inst {
        Inst::TestBranch { nonzero: true, rt: Gpr(0), bit: 0, target: label("far") }
    }

    /// A `tbnz` more than 32 KiB away was left alone and the object failed with
    /// "a branch offset 10815 does not fit 14 bits".
    #[test]
    fn far_tbnz_is_inverted_over_a_jump() {
        let mut blocks = function(vec![test_branch()], 10_000);
        assert!(first_error(&blocks).is_some(), "the premise: it does not encode as it is");
        run(&mut blocks).unwrap();
        assert_eq!(first_error(&blocks), None);
        let Inst::TestBranch { nonzero: false, target: Target::Label(skip), .. } = &blocks[0].insts[0] else {
            panic!("{:?}", blocks[0].insts[0]);
        };
        assert_eq!(blocks[0].insts[1], jump(label("far")));
        assert_eq!(blocks[0].insts[2], Inst::Label(skip.clone()));
    }

    #[test]
    fn far_cbz_and_bcond_reach_past_a_megabyte() {
        for first in [
            Inst::CompareBranch { nonzero: false, width: Width::W, rt: Gpr(0), target: label("far") },
            Inst::BranchCond { cond: Cond::Lt, target: label("far") },
        ] {
            let mut blocks = function(vec![first.clone()], 300_000);
            assert!(first_error(&blocks).is_some());
            run(&mut blocks).unwrap();
            assert_eq!(first_error(&blocks), None);
            assert_eq!(blocks[0].insts[1], jump(label("far")));
        }
    }

    /// A branch followed by a `b` to a near label swaps the two instead of
    /// adding a word.
    #[test]
    fn a_conditional_and_a_near_jump_swap() {
        let mut blocks = function(vec![test_branch(), jump(label("mid"))], 10_000);
        run(&mut blocks).unwrap();
        assert_eq!(first_error(&blocks), None);
        assert_eq!(blocks[0].insts.len(), 2);
        assert_eq!(blocks[0].insts[1], jump(label("far")));
    }

    /// Growing one branch pushes another, which was in reach, out of it.
    #[test]
    fn a_branch_pushed_out_by_another_relaxes_in_a_later_round() {
        let words = 8190;
        let mut blocks = function(vec![test_branch()], words);
        blocks[1].insts.insert(0, Inst::TestBranch { nonzero: true, rt: Gpr(0), bit: 0, target: label("far") });
        blocks[1].locs.insert(0, None);
        run(&mut blocks).unwrap();
        assert_eq!(first_error(&blocks), None);
    }

    /// Code with nothing far is left exactly as it was.
    #[test]
    fn near_branches_are_untouched() {
        let mut blocks = function(vec![test_branch()], 100);
        let before = blocks.clone();
        run(&mut blocks).unwrap();
        assert_eq!(format!("{blocks:?}"), format!("{before:?}"));
    }
}
