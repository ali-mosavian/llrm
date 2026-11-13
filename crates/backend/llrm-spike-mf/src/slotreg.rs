//! A spill slot that lives in a free register of another class.
//!
//! # How it works
//! [`slots_to_registers`] takes each spill slot whose every access is a plain
//! store or reload, solves where the slot's value is live, and gives the slot
//! a host register that is free over all of that range: no instruction there
//! writes it or clobbers it in a call, and none has it live. The stores become
//! `COPY host <- value` and the reloads `COPY value <- host`.
//!
//! DEVIATE SLOT-REG (owner m68k, exit 2026-11-24): the pin's greedy has no
//! spill to a register of another class: `RAGreedy::tryInstructionSplit`
//! ([RegAllocGreedy.cpp#L1560]) relaxes a split interval to the largest legal
//! superclass of its uses (`getLargestLegalSuperClass`), and a byte value has
//! no address-register class to relax to. gcc's IRA/LRA prices a value in
//! memory against a register of any class (`ira-costs.cc` `memory_move_cost`)
//! and parks it in `a0`/`a1`: known_loads_walks -Os, full `f`: 25 frame
//! references to the pin's 92, 52 register moves to the pin's 0. Reason: best
//! of gcc and clang is the goal. Exit: 2026-11-24, or the greedy's
//! class-relaxing split covers it.
//!
//! The host is a register the function never touches over the slot's range, so
//! the allocator's work is not disturbed: the pass runs after allocation, on
//! physical registers, as `StackSlotColoring` does.
//!
//! [RegAllocGreedy.cpp#L1560]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L1560

use llrm_lir::registers::{Info, RegId};

use crate::livephys::PhysLiveness;
use crate::mfunc::{Inst, Kind, MFunction, Operand, Reg};
use crate::regalloc::{Assignment, Target, successors};

/// What one instruction does to a slot: stores a register, or loads one.
enum Access {
    Store(Reg),
    Load(Reg),
}

/// The access `inst` makes to `slot`: `None` where it does not mention the
/// slot, `Some(None)` where it does in a way a register cannot stand in for.
fn access(
    inst: &Inst,
    slot: i32,
) -> Option<Option<Access>> {
    if !inst.operands.iter().any(|one| matches!(one, Operand::Frame(s) if *s == slot)) {
        return None;
    }
    Some(match &inst.operands[..] {
        [Operand::Imm(0), Operand::Frame(_), Operand::Use(register)] if !inst.may_load => {
            Some(Access::Store(*register))
        }
        [Operand::Def(register), Operand::Imm(0), Operand::Frame(_)] if !inst.may_store => {
            Some(Access::Load(*register))
        }
        _ => None,
    })
}

/// Where the slot's value must survive: `before[b][i]` says the value stored
/// by an earlier instruction is read by a later one, with `i` still ahead.
fn live_ranges<C>(
    function: &MFunction<C>,
    slot: i32,
    successors: &[Vec<usize>],
) -> Vec<Vec<bool>> {
    let step = |inst: &Inst, live: bool| match access(inst, slot) {
        Some(Some(Access::Store(_))) => false,
        Some(Some(Access::Load(_))) => true,
        _ => live,
    };
    let mut live_in = vec![false; function.blocks.len()];
    loop {
        let mut changed = false;
        for (at, block) in function.blocks.iter().enumerate().rev() {
            let out = successors[at].iter().any(|&next| live_in[next]);
            let now = block.insts.iter().rev().fold(out, |live, inst| step(inst, live));
            if now && !live_in[at] {
                live_in[at] = true;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    function
        .blocks
        .iter()
        .enumerate()
        .map(|(at, block)| {
            let mut live = successors[at].iter().any(|&next| live_in[next]);
            let mut before = vec![false; block.insts.len()];
            for (i, inst) in block.insts.iter().enumerate().rev() {
                live = step(inst, live);
                before[i] = live;
            }
            before
        })
        .collect()
}

/// Gives each slot of `slots` a register of `hosts` (full registers of `bits`
/// bits) where one is free over the slot's range, as described above.
/// `may_load(function, b, i)` says a reload may become a copy out of the
/// host (m68k: where the copy's condition codes are dead);
/// `worth(function, host, accesses)` prices the host's first use (a
/// callee-saved register is saved and restored); `finish(copy, into_host)` sets
/// what the target needs of the copy (its flag effects). Returns the slots
/// moved.
#[allow(clippy::too_many_arguments)]
pub fn slots_to_registers<T: Target>(
    function: &mut MFunction<T::Class>,
    assignment: &Assignment,
    info: &Info,
    slots: &[i32],
    hosts: &[RegId],
    bits: u32,
    may_load: impl Fn(&MFunction<T::Class>, usize, usize) -> bool,
    worth: impl Fn(&MFunction<T::Class>, RegId, usize) -> bool,
    finish: impl Fn(&mut Inst, bool),
) -> usize {
    let register = |reg: Reg| match reg {
        Reg::Virt(v) => assignment.0[v as usize],
        Reg::Phys(id) => id,
    };
    let full = |reg: Reg| info.view(T::root(register(reg)), bits).map(Reg::Phys);
    let successors = successors(function);
    let mut moved = 0;
    let mut live = PhysLiveness::of::<T>(function, assignment, info);
    let mut stale = false;
    for &slot in slots {
        let mut accesses: Vec<(usize, usize, Access)> = Vec::new();
        let mut plain = true;
        for (b, block) in function.blocks.iter().enumerate() {
            for (i, inst) in block.insts.iter().enumerate() {
                match access(inst, slot) {
                    None => {}
                    Some(None) => plain = false,
                    Some(Some(one)) => accesses.push((b, i, one)),
                }
            }
        }
        let loads = accesses.iter().filter(|(_, _, one)| matches!(one, Access::Load(_))).count();
        if !plain || loads == 0 || accesses.iter().any(|(_, _, one)| full(access_register(one)).is_none()) {
            continue;
        }
        if accesses.iter().any(|(b, i, one)| matches!(one, Access::Load(_)) && !may_load(function, *b, *i)) {
            continue;
        }
        let before = live_ranges(function, slot, &successors);
        if stale {
            live = PhysLiveness::of::<T>(function, assignment, info);
            stale = false;
        }
        let free = |host: RegId| {
            if T::reserved(T::root(host)) {
                return false;
            }
            function
                .blocks
                .iter()
                .enumerate()
                .all(
                    |(b, block)| block.insts.iter().enumerate().all(|(i, inst)| {
                        if !before[b][i] {
                            return true;
                        }
                        let clobbered = inst.call && T::caller_saved().iter().any(|&root| T::overlaps(root, host));
                        let written = inst
                            .operands
                            .iter()
                            .any(|one| matches!(one, Operand::Def(reg) if T::overlaps(register(*reg), host)));
                        !clobbered
                            && !written
                            && !live.live_before::<T>(function, assignment, info, b, i).is_live(info, host)
                    }),
                )
        };
        let Some(host) = hosts.iter().copied().find(|&host| free(host) && worth(function, host, accesses.len())) else {
            continue;
        };
        for (b, i, one) in accesses {
            let (def, used) = match one {
                Access::Store(value) => (Reg::Phys(host), full(value).expect("checked")),
                Access::Load(value) => (full(value).expect("checked"), Reg::Phys(host)),
            };
            let into_host = matches!(def, Reg::Phys(id) if id == host);
            let mut copy = Inst::new(Kind::Copy, vec![Operand::Def(def), Operand::Use(used)]);
            finish(&mut copy, into_host);
            function.blocks[b].insts[i] = copy;
        }
        moved += 1;
        stale = true;
    }
    moved
}

fn access_register(access: &Access) -> Reg {
    match access {
        Access::Store(reg) | Access::Load(reg) => *reg,
    }
}
