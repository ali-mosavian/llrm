//! A load through `(An)+` that one arithmetic instruction consumes is part of
//! it: `move.l (%a1)+, %d2; add.l %d2, %d0` is `add.l (%a1)+, %d0`.
//!
//! # How it works
//! [`fold_loads`] looks at each post-increment load whose value has one use,
//! later in the same block, as the register source of an arithmetic
//! instruction that has a memory-source form ([`indexed::source_twin`]). The
//! load moves down to the user and the two become the one writeback
//! instruction. Nothing between them may store, call, read or write the
//! updated base, or read the codes the load set.
//!
//! DEVIATE: the pin folds only plain loads, at selection, and has no
//! post-increment form of the arithmetic instructions. Reason: best of gcc and
//! clang is the goal, and gcc folds the post-increment too. Exit: none.
//!
//! # See also
//! `PeepholeOptimizer::optimizeLoadInstr` and
//! `TargetInstrInfo::foldMemoryOperand`.

use llrm_spike_mf::{Inst, Kind, Operand, Reg};

use crate::dagisel::{indexed, local};
use crate::mfunc::{MFunction, op_inst};

/// How many instructions read each virtual register.
fn use_counts(function: &MFunction) -> Vec<u32> {
    let mut counts = vec![0; function.classes.len()];
    for block in &function.blocks {
        for phi in &block.phis {
            for (_, incoming) in &phi.incoming {
                if let Reg::Virt(v) = incoming {
                    counts[*v as usize] += 1;
                }
            }
        }
        for inst in &block.insts {
            for operand in &inst.operands {
                if let Operand::Use(Reg::Virt(v)) = operand {
                    counts[*v as usize] += 1;
                }
            }
        }
    }
    counts
}

pub fn fold_loads(function: &mut MFunction) {
    let counts = use_counts(function);
    for b in 0..function.blocks.len() {
        let mut at = 0;
        while at < function.blocks[b].insts.len() {
            if fold_one(function, &counts, b, at) {
                // the load at `at` is gone and the next instruction took its
                // place
                continue;
            }
            at += 1;
        }
    }
}

fn fold_one(
    function: &mut MFunction,
    counts: &[u32],
    b: usize,
    at: usize,
) -> bool {
    let insts = &function.blocks[b].insts;
    let Kind::Op(load) = insts[at].kind else { return false };
    let Some(bits) = indexed::post_load_bits(load) else { return false };
    let [Operand::Def(Reg::Virt(v)), Operand::Def(Reg::Virt(updated)), Operand::Use(base)] = insts[at].operands[..]
    else {
        return false;
    };
    if counts[v as usize] != 1 {
        return false;
    }
    // the user, with nothing in between that the move down would cross
    let mut user = None;
    for (offset, one) in insts[at + 1..].iter().enumerate() {
        if one.operands.contains(&Operand::Use(Reg::Virt(v))) {
            user = Some(at + 1 + offset);
            break;
        }
        let touches_base = one
            .operands
            .iter()
            .any(
                |operand| match operand {
                    Operand::Use(Reg::Virt(r)) | Operand::Def(Reg::Virt(r)) => *r == updated,
                    _ => false,
                },
            );
        if one.may_store
            || one.call
            || one.side_effects
            || one.barrier
            || one.terminator
            || one.reads_flags
            || touches_base
        {
            return false;
        }
    }
    let Some(user) = user else { return false };
    let Kind::Op(name) = insts[user].kind else { return false };
    let Some(twin) = indexed::source_twin(name) else { return false };
    if !name.contains(bits) || !twin.contains(bits) {
        return false;
    }
    // the loaded value is the last operand, the memory source's place
    let mut operands = insts[user].operands.clone();
    let last = operands.len() - 1;
    let loaded = Operand::Use(Reg::Virt(v));
    if operands[last] != loaded {
        let swappable = insts[user].commutable
            && operands.len() == 3
            && operands[1] == loaded
            && matches!(operands[2], Operand::Use(Reg::Virt(_)));
        if !swappable {
            return false;
        }
        operands.swap(1, 2);
    }
    operands.pop();
    let defs = operands.iter().take_while(|operand| matches!(operand, Operand::Def(_))).count();
    operands.insert(defs, Operand::Def(Reg::Virt(updated)));
    operands.push(Operand::Use(base));
    let Some(desc) = local::code_of(twin).and_then(local::desc) else { return false };
    let folded: Inst = op_inst(twin, desc, operands);
    let insts = &mut function.blocks[b].insts;
    insts[user] = folded;
    insts.remove(at);
    true
}
