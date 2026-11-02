//! Keeping the condition codes a branch reads: a `move` into a data register
//! sets them, so a phi's copy between the instruction that set them and the
//! branch that reads them would change what the branch sees (`llc` saves and
//! restores `sr` around such a copy, `copyPhysReg`).
//!
//! Done on the allocated function. A copy that does not depend on the setter
//! moves above it. A copy that does stays, and the codes are pushed before it
//! and popped after it, as the pin does.
//!
//! DEVIATE: the copies that can move do move, which the pin does not do (it
//! always saves and restores). Reason: a push and a pop cost more than the
//! move above the setter, which costs nothing. Exit: none. The pin's `move %sr`
//! on the 68000 and `move %ccr` from the 68010 on are both legal in user mode.

use llrm_lir::registers::RegId;
use llrm_spike_mf::{Inst, Kind, Operand, Reg};

use crate::mfunc::MFunction;
use crate::regalloc::Assignment;
use crate::{REGISTER_INFO, reg};

fn root(
    register: Reg,
    assignment: &Assignment,
) -> RegId {
    REGISTER_INFO.root(match register {
        Reg::Virt(v) => assignment.0[v as usize],
        Reg::Phys(p) => p,
    })
}

pub(crate) fn is_data(register: RegId) -> bool {
    [reg::D0, reg::D1, reg::D2, reg::D3, reg::D4, reg::D5, reg::D6, reg::D7].contains(&register)
}

fn defs<'a>(
    inst: &'a Inst,
    assignment: &'a Assignment,
) -> impl Iterator<Item = RegId> + 'a {
    inst.operands.iter().filter_map(move |o| if let Operand::Def(r) = o { Some(root(*r, assignment)) } else { None })
}

fn uses<'a>(
    inst: &'a Inst,
    assignment: &'a Assignment,
) -> impl Iterator<Item = RegId> + 'a {
    inst.operands.iter().filter_map(move |o| if let Operand::Use(r) = o { Some(root(*r, assignment)) } else { None })
}

/// Makes every flag reader in `function` see the codes of the instruction it
/// meant.
pub fn preserve_flags(
    function: &mut MFunction,
    assignment: &Assignment,
) -> Result<(), String> {
    for block in &mut function.blocks {
        let mut r = 0;
        while r < block.insts.len() {
            if !block.insts[r].reads_flags || block.insts[r].sets_flags {
                r += 1;
                continue;
            }
            let Some(mut s) = block.insts[..r].iter().rposition(|inst| inst.sets_flags) else {
                r += 1;
                continue;
            };
            // a copy that becomes a `move` into a data register, from `s + 1`
            // on
            let clobbering = |block: &llrm_spike_mf::Block, s: usize, r: usize| -> Vec<usize> {
                (s + 1..r)
                    .filter(|&i| {
                        matches!(
                            block.insts[i].operands[..],
                            [Operand::Def(to), Operand::Use(from)] if block.insts[i].kind == Kind::Copy && is_data(root(to, assignment)) && root(to, assignment) != root(from, assignment)
                        )
                    })
                    .collect()
            };
            // those that do not depend on what runs from the setter up to them
            // move above the setter
            loop {
                let movable = clobbering(block, s, r).into_iter().find(|&i| {
                    let [Operand::Def(to), Operand::Use(from)] = block.insts[i].operands[..] else { return false };
                    let (dst, src) = (root(to, assignment), root(from, assignment));
                    !block
                        .insts[s..i]
                        .iter()
                        .any(
                            |before| defs(before, assignment).any(|d| d == src || d == dst)
                                || uses(before, assignment).any(|u| u == dst),
                        )
                });
                let Some(i) = movable else { break };
                let moved = block.insts.remove(i);
                block.insts.insert(s, moved);
                s += 1;
            }
            let stay = clobbering(block, s, r);
            if stay.is_empty() {
                r += 1;
                continue;
            }
            // `copyPhysReg`: a push and a pop of the codes round the copies
            let (first, last) = (stay[0], r);
            let mut pop = Inst::new(Kind::Op("CCR_POP"), vec![]);
            pop.sets_flags = true;
            let mut push = Inst::new(Kind::Op("CCR_PUSH"), vec![]);
            push.reads_flags = true;
            block.insts.insert(last, pop);
            block.insts.insert(first, push);
            r += 3;
        }
    }
    Ok(())
}
