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

use crate::ccr;
use crate::dagisel::{indexed, local};
use crate::mfunc::{Class, MFunction, op_inst, operand_class};

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
    // the user: the first reader of the value
    let user = insts[at + 1..]
        .iter()
        .position(|one| one.operands.contains(&Operand::Use(Reg::Virt(v))))
        .map(|offset| at + 1 + offset);
    let Some(user) = user else { return false };
    let Kind::Op(name) = insts[user].kind else { return false };
    let store = indexed::store_twin(name);
    let Some(twin) = store.or_else(|| indexed::source_twin(name)) else { return false };
    if !name.contains(bits) || !twin.contains(bits) {
        return false;
    }
    let loaded = Operand::Use(Reg::Virt(v));
    let mut operands;
    let mut readers = Vec::new();
    if store.is_some() {
        // a store of the loaded value, its last operand: the move from `(An)+`
        // to where it was stored, the stores' own results first
        let stored = &insts[user].operands;
        if stored.last() != Some(&loaded) || stored[..stored.len() - 1].contains(&loaded) {
            return false;
        }
        operands = stored[..stored.len() - 1].to_vec();
        let defs = operands.iter().take_while(|operand| matches!(operand, Operand::Def(_))).count();
        operands.insert(defs, Operand::Def(Reg::Virt(updated)));
    } else {
        // the memory source takes the place of the operand the text prints
        // first
        let Some(&(_, _, printed)) = local::text(name) else { return false };
        let Some(first) = printed.first().map(|one| usize::from(one.at)) else { return false };
        operands = insts[user].operands.clone();
        if operands[first] != loaded {
            let other = operands.iter().position(|one| *one == loaded).unwrap_or(first);
            let both_registers = matches!(operands[first], Operand::Use(Reg::Virt(_)));
            if insts[user].commutable && both_registers {
                operands.swap(first, other);
            } else if operands.len() == 2
                && both_registers
                && !operands.iter().any(|one| matches!(one, Operand::Def(_)))
            {
                // a compare of the loaded value with another: the other way
                // round, and the conditions that read its codes
                // change with it
                let Some(found) = swapped_readers(&insts[user + 1..]) else { return false };
                readers = found;
                operands.swap(0, 1);
            } else {
                return false;
            }
        }
        operands.remove(first);
        let defs = operands.iter().take_while(|operand| matches!(operand, Operand::Def(_))).count();
        operands.insert(defs, Operand::Def(Reg::Virt(updated)));
    }
    operands.push(Operand::Use(base));
    // the load moves down to the user, or the user up to the load
    let between = &insts[at + 1..user];
    let touches = |one: &Inst, register: u32| {
        one.operands
            .iter()
            .any(
                |operand| matches!(
                    operand,
                    Operand::Use(Reg::Virt(r)) | Operand::Def(Reg::Virt(r)) if *r == register
                ),
            )
    };
    // copies of the updated base wait for the folded instruction, which
    // defines it
    let late = |at: usize| {
        let one = &between[at];
        let Kind::Copy = one.kind else { return false };
        let [Operand::Def(Reg::Virt(copy)), Operand::Use(Reg::Virt(from))] = one.operands[..] else { return false };
        from == updated
            && !between
                .iter()
                .chain(&insts[user..=user])
                .any(|other| other.operands.contains(&Operand::Use(Reg::Virt(copy))))
    };
    let late_copies: Vec<bool> = (0..between.len()).map(late).collect();
    let between_len = between.len();
    let down = between
        .iter()
        .enumerate()
        .all(
            |(at, one)| !(one.may_store
                || one.call
                || one.side_effects
                || one.barrier
                || one.terminator
                || one.reads_flags
                || touches(one, updated) && !late(at)),
        );
    // a store moved up past a load or store would change what they see
    let up = store.is_none()
        && between.iter().all(|one| {
            !(one.sets_flags
                || one.reads_flags
                || one.call
                || one.barrier
                || one.terminator
                || operands.iter().any(|operand| {
                    matches!(
                        operand,
                        Operand::Use(Reg::Virt(r)) if one.operands.contains(&Operand::Def(Reg::Virt(*r)))
                    )
                }))
        });
    if !down && !up {
        return false;
    }
    let Some(desc) = local::code_of(twin).and_then(local::desc) else { return false };
    // the twin's classes are the pin's: a register of either class that its
    // operand takes only a data register for becomes one
    let mut narrowed = Vec::new();
    for (index, operand) in operands.iter().enumerate() {
        let (Operand::Use(Reg::Virt(r)) | Operand::Def(Reg::Virt(r))) = operand else { continue };
        let Some(want) = operand_class(twin, index) else { continue };
        let have = function.classes[*r as usize];
        if have == want {
            continue;
        }
        if have != Class::Xr32 {
            return false;
        }
        narrowed.push((*r, want));
    }
    for (r, want) in narrowed {
        function.classes[r as usize] = want;
    }
    let folded: Inst = op_inst(twin, desc, operands);
    let insts = &mut function.blocks[b].insts;
    for (offset, reader) in readers {
        insts[user + 1 + offset] = reader;
    }
    if down {
        let waiting: Vec<Inst> =
            (0..between_len).filter(|&i| late_copies[i]).map(|i| insts[at + 1 + i].clone()).collect();
        insts[user] = folded;
        for (i, copy) in waiting.into_iter().enumerate() {
            insts.insert(user + 1 + i, copy);
        }
        // the load and the copies that moved after the folded instruction
        let mut drop: Vec<usize> = (0..between_len).filter(|&i| late_copies[i]).map(|i| at + 1 + i).collect();
        drop.push(at);
        drop.sort_unstable_by(|a, b| b.cmp(a));
        for one in drop {
            insts.remove(one);
        }
    } else {
        insts[at] = folded;
        insts.remove(user);
    }
    true
}

/// The readers of the codes after a compare, as they read once its operands
/// change places, with their offsets; `None` when one cannot.
fn swapped_readers(after: &[Inst]) -> Option<Vec<(usize, Inst)>> {
    let mut found = Vec::new();
    for (offset, inst) in after.iter().enumerate() {
        if inst.sets_flags || inst.call {
            break;
        }
        if !inst.reads_flags {
            continue;
        }
        let Kind::Op(name) = inst.kind else { return None };
        let mut swapped = inst.clone();
        if name.starts_with("CMOV") {
            let Some(Operand::Imm(code)) = swapped.operands.last_mut() else { return None };
            *code = ccr::swapped_code(*code)?;
        } else {
            let text = local::text(name)?.1.split('\t').next()?;
            let condition = text.strip_prefix('b').or_else(|| text.strip_prefix('s'))?;
            let to = ccr::swapped_name(condition)?;
            let renamed = local::name_of(u32::from(local::code_of(&name.replacen(condition, to, 1))?))?;
            swapped.kind = Kind::Op(renamed);
        }
        found.push((offset, swapped));
    }
    Some(found)
}
