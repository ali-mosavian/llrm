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
