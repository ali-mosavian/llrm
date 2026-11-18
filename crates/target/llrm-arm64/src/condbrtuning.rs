//! A branch on a value made in the same block becomes a conditional branch on
//! the flags that making it sets: `sub w8, w0, #1; cbz w8, L` is
//! `subs w8, w0, #1; b.eq L`.
//!
//! Translated from `AArch64CondBrTuning`
//! ([AArch64CondBrTuning.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64CondBrTuning.cpp)),
//! which the pin runs after `MachineCombiner` and before `EarlyIfConverter`
//! ([AArch64TargetMachine.cpp#L850](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64TargetMachine.cpp#L850),
//! on by default).
//!
//! # How it works
//! Machine form, virtual registers, before allocation, after the combiner and
//! the compare passes (as the pin). A `cbz`, `cbnz`,
//! `tbz` or `tbnz` (the last two only on the sign bit) that ends a block tests
//! a register last defined earlier in the same block by an add, subtract,
//! `and` or `bic` (or their flag-setting forms). Nothing between the two may
//! read or write the flags. The maker becomes its flag-setting form (its
//! result goes to the zero register when the branch was its only reader) and
//! the branch becomes `b.eq`, `b.ne`, `b.pl` or `b.mi`. One branch per block
//! changes, as the flags would clash.
//!
//! Differences: the pin's pass is skipped at `-O0`, which this path does not
//! tell apart; the pin's frame-index test has no counterpart, a frame address
//! being a separate operation here.

use crate::facts;
use crate::mc::inst::*;
use crate::mc::visit::Reg;
use crate::mfunc::{MFunc, Op};

/// The condition a `cbz`/`cbnz`/`tbz`/`tbnz` becomes, and the register and
/// width it tests; `None` where it is not one the pass takes (a `tbz` of
/// another bit than the sign).
fn branch_to_condition(op: &Op) -> Option<(Cond, Gpr, Width)> {
    match op {
        Op::I(Inst::CompareBranch { nonzero, width, rt, .. }) => {
            Some((if *nonzero { Cond::Ne } else { Cond::Eq }, *rt, *width))
        }
        // The sign bit of a W register is bit 31, of an X register bit 63; a
        // `tbz` of bit 31 on an X register tests a different bit.
        Op::I(Inst::TestBranch { nonzero, rt, bit, .. }) if *bit == 31 || *bit == 63 => {
            let width = if *bit == 31 { Width::W } else { Width::X };
            Some((if *nonzero { Cond::Mi } else { Cond::Pl }, *rt, width))
        }
        _ => None,
    }
}

/// The target label of a branch `branch_to_condition` takes.
fn target_of(op: &Op) -> Option<Target> {
    match op {
        Op::I(Inst::CompareBranch { target, .. } | Inst::TestBranch { target, .. }) => Some(target.clone()),
        _ => None,
    }
}

/// The destination and width of an operation `convertToFlagSettingOpc`
/// converts (or that already sets the flags and has one).
fn flag_maker(op: &Op) -> Option<(Gpr, Width)> {
    match op {
        Op::I(
            Inst::AddSubImm { rd, width, .. }
            | Inst::AddSubShifted { rd, width, .. }
            | Inst::AddSubExtended { rd, width, .. }
            | Inst::LogicImm { op: Logic::And | Logic::Ands, rd, width, .. }
            | Inst::LogicShifted { op: Logic::And | Logic::Ands, rd, width, .. },
        ) => Some((*rd, *width)),
        _ => None,
    }
}

/// `convertToFlagSettingOpc`, with the destination given: the maker as the
/// flag-setting form.
fn set_flags(
    op: &mut Op,
    destination: Gpr,
) {
    match op {
        Op::I(
            Inst::AddSubImm { op, rd, .. } | Inst::AddSubShifted { op, rd, .. } | Inst::AddSubExtended { op, rd, .. },
        ) => {
            op.set_flags = true;
            *rd = destination;
        }
        Op::I(Inst::LogicImm { op, rd, .. } | Inst::LogicShifted { op, rd, .. }) => {
            *op = Logic::Ands;
            *rd = destination;
        }
        _ => unreachable!("flag_maker named it"),
    }
}

/// `isNZCVTouchedInInstructionRange`: whether an operation reads or writes the
/// flags.
fn touches_flags(op: &Op) -> bool {
    let facts = facts::of(op);
    facts.reads_flags || facts.sets_flags
}

pub fn run(function: &mut MFunc) {
    let mut reads: rustc_hash::FxHashMap<Reg, usize> = Default::default();
    for block in &function.blocks {
        for op in &block.ops {
            for used in op.defs_and_uses().1 {
                *reads.entry(used).or_default() += 1;
            }
        }
    }
    for block in &mut function.blocks {
        let ops: Vec<Op> = block.ops.iter().cloned().collect();
        // The terminators: the run of branches at the end.
        let first_terminator = ops
            .iter()
            .rposition(|op| {
                !matches!(
                    op,
                    Op::I(
                        Inst::Branch { .. }
                            | Inst::BranchCond { .. }
                            | Inst::CompareBranch { .. }
                            | Inst::TestBranch { .. }
                    )
                )
            })
            .map_or(0, |at| at + 1);
        for at in first_terminator..ops.len() {
            let Some((cond, tested, width)) = branch_to_condition(&ops[at]) else { continue };
            let register = Reg::Gpr(tested);
            if !register.is_virtual() {
                continue;
            }
            let Some(maker) = ops[..at].iter().rposition(|op| op.defs_and_uses().0.contains(&register)) else {
                continue;
            };
            let Some((_, maker_width)) = flag_maker(&ops[maker]) else { continue };
            if maker_width != width || ops[maker + 1..at].iter().any(touches_flags) {
                continue;
            }
            // `hasOneNonDBGUse`: the branch was the only reader.
            let destination = if reads.get(&register) == Some(&1) { Gpr::R31 } else { tested };
            let target = target_of(&ops[at]).expect("branch_to_condition took it");
            set_flags(block.ops.op_mut(maker), destination);
            *block.ops.op_mut(at) = Op::I(Inst::BranchCond { cond, target });
            // The flags clash with any other branch's: one change per block.
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mfunc::MBlock;

    fn sub_one(
        rd: u32,
        rn: u32,
    ) -> Op {
        Op::I(Inst::AddSubImm {
            op: AddSub { subtract: true, set_flags: false },
            width: Width::W,
            rd: Gpr(rd),
            rn: Gpr(rn),
            imm12: 1,
            shift12: false,
        })
    }

    fn branch_if_zero(rt: u32) -> Op {
        Op::I(Inst::CompareBranch {
            nonzero: false,
            width: Width::W,
            rt: Gpr(rt),
            target: Target::Label("done".into()),
        })
    }

    fn keep(rd: u32) -> Op {
        Op::I(Inst::Store {
            mem: Memory::Int { width: Width::W, bytes: 4, sign: false, reg: Gpr(rd) },
            addr: Addr::Slot { slot: 0, offset: 0 },
            mmo: MemOperand::default(),
        })
    }

    /// `entry` ends in the branch; `done` holds `later` and returns.
    fn function(
        ops: Vec<Op>,
        later: Vec<Op>,
    ) -> MFunc {
        let mut function = MFunc::new("f");
        function.next_register = 80;
        let mut done = later;
        done.push(Op::Return { uses: vec![], tail: None });
        function.blocks =
            vec![MBlock { label: "entry".into(), ops: ops.into() }, MBlock { label: "done".into(), ops: done.into() }];
        function
    }

    /// `sub w65, w64, #1; cbz w65` ended in a `sub` and a `cbz`, and the next
    /// block's own `sub w64, #1` recomputed the value (rectwo -O2: a `sub`
    /// more per loop than the pin's `subs; b.eq`).
    #[test]
    fn a_branch_on_a_value_with_a_later_reader_reads_the_flags_the_value_sets() {
        let mut function = function(vec![sub_one(65, 64), branch_if_zero(65)], vec![keep(65)]);
        run(&mut function);
        let ops: Vec<&Op> = function.blocks[0].ops.iter().collect();
        assert!(
            matches!(
                ops[0],
                Op::I(Inst::AddSubImm { op: AddSub { subtract: true, set_flags: true }, rd: Gpr(65), .. })
            ),
            "{function:#?}"
        );
        assert!(matches!(ops[1], Op::I(Inst::BranchCond { cond: Cond::Eq, .. })), "{function:#?}");
    }

    /// The branch was the only reader: the result goes to the zero register.
    #[test]
    fn a_branch_the_only_reader_sends_the_result_to_the_zero_register() {
        let mut function = function(vec![sub_one(65, 64), branch_if_zero(65)], vec![]);
        run(&mut function);
        assert!(
            matches!(function.blocks[0].ops.iter().next(), Some(Op::I(Inst::AddSubImm { rd: Gpr::R31, .. }))),
            "{function:#?}"
        );
    }

    /// A flag write between the two stops it.
    #[test]
    fn flags_touched_between_the_value_and_the_branch_leave_it_alone() {
        let compare = Op::I(Inst::AddSubImm {
            op: AddSub { subtract: true, set_flags: true },
            width: Width::W,
            rd: Gpr::R31,
            rn: Gpr(66),
            imm12: 0,
            shift12: false,
        });
        let mut function = function(vec![sub_one(65, 64), compare, branch_if_zero(65)], vec![keep(65)]);
        let before = format!("{:?}", function.blocks[0].ops);
        run(&mut function);
        assert_eq!(before, format!("{:?}", function.blocks[0].ops));
    }
}
