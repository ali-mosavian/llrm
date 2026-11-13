//! A compare that repeats the one the only way into its block ended with is
//! dropped: `cmp.l %a0, %d3; beq .L8; cmp.l %a0, %d3; bge .L5` is `cmp.l %a0,
//! %d3; beq .L8; bge .L5`. `a[m] == key` then `a[m] < key` makes two compares
//! of the same registers.
//!
//! # How it works
//! [`drop_repeated_compares`] runs after allocation. A block with one entering
//! edge whose first instruction is a compare is checked against its
//! predecessor: when the last codes setter there is the same compare, of the
//! same registers (by root, at the same size) and only branches follow it, the
//! codes on entry are the compare's and the second goes. Anything between them
//! that could set the codes (a copy that becomes a `move`, a reload) keeps it.
//!
//! DEVIATE: the pin has no `optimizeCompareInstr` for m68k, and its machine CSE
//! leaves a compare that defines the codes. Reason: best of gcc and clang is the
//! goal, and gcc reuses the codes across the branch. Exit: 2026-11-12, the
//! shared flow's peephole owns it (convergence step 5) or the pin's choice
//! stands. Off for `Options::PINNED`.
//!
//! # See also
//! `X86InstrInfo::optimizeCompareInstr`, `cmpelim.rs`.

use std::collections::HashMap;

use llrm_spike_mf::{Inst, Kind, Operand, Reg};

use crate::mfunc::MFunction;
use crate::regalloc::Assignment;
use crate::REGISTER_INFO;

fn is_compare(inst: &Inst) -> bool {
    matches!(inst.kind, Kind::Op(name) if name.starts_with("CMP") && !name.starts_with("CMPA")) && inst.sets_flags
}

/// The operands with each register as its root, so two compares of the same
/// registers read equal.
fn rooted(
    inst: &Inst,
    assignment: &Assignment,
) -> Vec<Operand> {
    let root = |reg: &Reg| match reg {
        Reg::Virt(v) => Reg::Phys(REGISTER_INFO.root(assignment.0[*v as usize])),
        Reg::Phys(p) => Reg::Phys(REGISTER_INFO.root(*p)),
    };
    inst.operands
        .iter()
        .map(|operand| match operand {
            Operand::Use(reg) => Operand::Use(root(reg)),
            Operand::Def(reg) => Operand::Def(root(reg)),
            other => other.clone(),
        })
        .collect()
}

/// Drops each compare that repeats the one its only predecessor ended with.
pub fn drop_repeated_compares(
    function: &mut MFunction,
    assignment: &Assignment,
) {
    // the edges into each block, by id
    let mut entering: HashMap<u32, Vec<usize>> = HashMap::new();
    for (at, block) in function.blocks.iter().enumerate() {
        for inst in block.insts.iter().filter(|inst| inst.terminator) {
            for operand in &inst.operands {
                if let Operand::Block(id) = operand {
                    entering.entry(*id).or_default().push(at);
                }
            }
        }
    }
    // a block that falls into the next one is an edge no terminator names
    for at in 1..function.blocks.len() {
        if !function.blocks[at - 1].insts.last().is_some_and(|last| last.barrier) {
            entering.entry(function.blocks[at].id).or_default().push(at - 1);
        }
    }
    for at in 1..function.blocks.len() {
        let id = function.blocks[at].id;
        let [from] = entering.get(&id).map(Vec::as_slice).unwrap_or(&[]) else { continue };
        let Some(first) = function.blocks[at].insts.first().filter(|inst| is_compare(inst)) else { continue };
        let before = &function.blocks[*from].insts;
        // the setter, then only branches
        let Some(setter) = before.iter().rposition(|inst| !inst.terminator) else { continue };
        if !before[setter + 1..].iter().all(|inst| inst.terminator && !inst.sets_flags && !inst.call) {
            continue;
        }
        let last = &before[setter];
        if is_compare(last) && last.kind == first.kind && rooted(last, assignment) == rooted(first, assignment) {
            function.blocks[at].insts.remove(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use llrm_lir::registers::RegId;
    use llrm_spike_mf::{Block, Inst};

    use super::*;
    use crate::mfunc::MFunction;

    fn phys(n: u8) -> Reg {
        Reg::Phys(RegId::from_raw(n))
    }

    fn compare() -> Inst {
        let mut inst = Inst::new(Kind::Op("CMP32rr"), vec![Operand::Use(phys(0)), Operand::Use(phys(1))]);
        inst.sets_flags = true;
        inst
    }

    fn branch(to: u32) -> Inst {
        let mut inst = Inst::new(Kind::Op("Bcc"), vec![Operand::Block(to)]);
        inst.terminator = true;
        inst.reads_flags = true;
        inst
    }

    /// `A: cmp; bge Y` falls into `B: move`, which falls into `Y: cmp; blt`:
    /// the compare in `Y` was dropped as the repeat of `A`'s, so on the path
    /// through `B` the `blt` read the `move`'s codes.
    #[test]
    fn a_compare_reached_by_falling_through_a_move_stays() {
        let mut function = MFunction::new("f");
        let copy = Inst::new(Kind::Copy, vec![Operand::Def(phys(2)), Operand::Use(phys(3))]);
        for (id, insts) in [(0, vec![compare(), branch(2)]), (1, vec![copy]), (2, vec![compare(), branch(0)])] {
            function.blocks.push(Block { id, insts, phis: vec![] });
        }
        drop_repeated_compares(&mut function, &Assignment(vec![]));
        assert_eq!(function.blocks[2].insts.len(), 2);
    }
}
