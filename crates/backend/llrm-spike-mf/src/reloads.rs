//! Forwarding of reloads: a value stored to a spill slot is still in the register it was stored from until
//! something writes that register, so a reload of the slot there is that register (or nothing, if it is the one
//! the reload names).
//!
//! The spiller stores a spilled register after each definition and loads it before each use, in every block,
//! whether or not the register was touched since. The pin's greedy allocator splits the live range instead
//! and reloads only after the call or write that took the register (`RAGreedy` splitting, `InlineSpiller`).
//!
//! DEVIATE: a forward analysis over the allocated function, not live-range splitting. Reason: the spill layer
//! here is spill-everywhere. Exit: the neutral-LIR task, with the pin's allocator.

use llrm_lir::registers::RegId;

use crate::mfunc::{Inst, MFunction, Operand, Reg};
use crate::regalloc::{Assignment, Target, successors};

/// A slot's value is also in `root`, which `reg` names.
#[derive(Clone, Copy, Debug)]
struct Held {
    slot: i32,
    root: RegId,
    reg: Reg,
}

impl Held {
    fn same(
        &self,
        other: &Held,
    ) -> bool {
        self.slot == other.slot && self.root == other.root
    }
}

type State = Vec<Held>;

fn meet(
    one: &State,
    other: &State,
) -> State {
    one.iter().filter(|held| other.iter().any(|also| held.same(also))).copied().collect()
}

fn equal(
    one: &State,
    other: &State,
) -> bool {
    one.len() == other.len() && one.iter().all(|held| other.iter().any(|also| held.same(also)))
}

/// Replaces each reload of a spill slot whose value is still in a register by a copy from it, or drops it.
/// `store` and `load` recognize the target's spill instructions: the slot and the register stored or loaded.
/// `wide` says whether a register holds a word, `cheap_copy` whether the target copies the second register to
/// the first without a detour.
pub fn forward_reloads<T: Target>(
    function: &mut MFunction<T::Class>,
    assignment: &Assignment,
    store: impl Fn(&Inst) -> Option<(i32, Reg)>,
    load: impl Fn(&Inst) -> Option<(i32, Reg)>,
    wide: impl Fn(RegId) -> bool,
    cheap_copy: impl Fn(RegId, RegId) -> bool,
) {
    let root = |reg: Reg| match reg {
        Reg::Virt(v) => T::root(assignment.0[v as usize]),
        Reg::Phys(id) => T::root(id),
    };
    let clobbered_by_call = T::caller_saved();
    let next = successors(function);
    let count = function.blocks.len();
    let mut preds = vec![Vec::new(); count];
    for (at, targets) in next.iter().enumerate() {
        for &target in targets {
            preds[target].push(at);
        }
    }

    // What one block does to the state; `edit` rewrites its reloads.
    let step = |block: &mut crate::mfunc::Block, mut state: State, edit: bool| -> State {
        let mut kept = Vec::with_capacity(block.insts.len());
        for mut inst in std::mem::take(&mut block.insts) {
            if let Some((slot, reg)) = load(&inst) {
                let target = root(reg);
                if state.iter().any(|held| held.slot == slot && held.root == target) {
                    // the register already holds it: nothing is written
                    if !edit {
                        kept.push(inst);
                    }
                    continue;
                }
                if let Some(held) = state
                    .iter()
                    .find(|held| held.slot == slot && wide(held.root) == wide(target) && cheap_copy(target, held.root))
                    .copied()
                    && edit
                {
                    inst = Inst::copy(reg, held.reg);
                }
                // what the reload (or the copy) writes
                state.retain(|held| !T::overlaps(target, held.root));
                state.push(Held { slot, root: target, reg });
                kept.push(inst);
                continue;
            }
            let written: Vec<RegId> = inst
                .operands
                .iter()
                .filter_map(|operand| if let Operand::Def(reg) = operand { Some(root(*reg)) } else { None })
                .collect();
            state.retain(|held| {
                !written.iter().any(|w| T::overlaps(*w, held.root))
                    && !(inst.call && clobbered_by_call.iter().any(|c| T::overlaps(*c, held.root)))
            });
            if let Some((slot, reg)) = store(&inst) {
                state.retain(|held| held.slot != slot);
                state.push(Held { slot, root: root(reg), reg });
            }
            kept.push(inst);
        }
        block.insts = kept;
        state
    };
    let entering = |at: usize, out: &[Option<State>], blocks: &[crate::mfunc::Block]| -> Option<State> {
        if at == 0 || preds[at].is_empty() || !blocks[at].phis.is_empty() {
            return Some(Vec::new());
        }
        // a predecessor not yet seen is everything (the top of the lattice)
        preds[at].iter().filter_map(|p| out[*p].as_ref()).cloned().reduce(|one, other| meet(&one, &other))
    };

    let mut out: Vec<Option<State>> = vec![None; count];
    loop {
        let mut changed = false;
        for at in 0..count {
            let Some(input) = entering(at, &out, &function.blocks) else { continue };
            let mut scratch = function.blocks[at].clone();
            let result = step(&mut scratch, input, false);
            if !out[at].as_ref().is_some_and(|before| equal(before, &result)) {
                out[at] = Some(result);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    for at in 0..count {
        let input = entering(at, &out, &function.blocks).unwrap_or_default();
        step(&mut function.blocks[at], input, true);
    }
}
