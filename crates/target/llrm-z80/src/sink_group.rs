//! A closed group of instructions moved into the one successor that needs it.
//!
//! # How it works
//! A branch's two successors: the group of instructions in front of it whose
//! results only that successor (and what it alone leads to) reads is moved to
//! the successor's head, so the branch no longer waits behind them. The long
//! step of a counted loop is one: `inc bc; ld a, b; or c; jr nz, 1$; inc de`
//! sat between the exit test and its branch, clobbered `a` and the flags, and
//! the test's byte was stored and reloaded around it. Moved onto the
//! continuing edge, the test branches first, as SDCC lays it out.
//!
//! A group is closed when every physical register it writes (the status flags
//! included) is read, before the next write, only inside the group, and every
//! one it reads was written inside it (the flags a carry step reads, the `a`
//! its compare uses). Only a group that holds a reader of the flags moves:
//! the rest is the pin's one-instruction sink, which this target does not run.
//!
//! DEVIATE: `MachineSink::SinkInstruction` moves one instruction and refuses
//! one with a physical register operand, so an `add`/`adc` chain stays
//! where it is. Reason: the z80's long step is a chain through the flags and
//! `a`, and `a` is one register of three. Exit: none.
//!
//! # See also
//! `llvm/lib/CodeGen/MachineSink.cpp` (`MachineSinking::SinkInstruction`,
//! `FindSuccToSinkTo`).

use llrm_spike_mf::regalloc::{Target, successors};
use llrm_support::hash::{HashMap, HashSet};

use crate::mfunc::{Inst, MFunction, Operand, Reg};
use crate::regalloc::Z80Registers;

/// Moves the closed groups; whether any moved.
pub fn sink_closed_groups(function: &mut MFunction) -> bool {
    let mut changed = false;
    // a moved group can make the block it lands in a source for the next
    for _ in 0..function.blocks.len() {
        if !sink_one(function) {
            break;
        }
        changed = true;
    }
    changed
}

/// One move, if there is one.
fn sink_one(function: &mut MFunction) -> bool {
    if function.blocks.iter().any(|block| !block.phis.is_empty()) {
        return false;
    }
    let next = successors(function);
    let mut entered = vec![0usize; function.blocks.len()];
    for to in next.iter().flatten() {
        entered[*to] += 1;
    }
    let mut defs: HashMap<u32, Vec<(usize, usize)>> = HashMap::default();
    let mut uses: HashMap<u32, Vec<(usize, usize)>> = HashMap::default();
    for (b, block) in function.blocks.iter().enumerate() {
        for (n, inst) in block.insts.iter().enumerate() {
            for operand in &inst.operands {
                match operand {
                    Operand::Def(Reg::Virt(v)) => defs.entry(*v).or_default().push((b, n)),
                    Operand::Use(Reg::Virt(v)) => uses.entry(*v).or_default().push((b, n)),
                    _ => {}
                }
            }
        }
    }
    for b in 0..function.blocks.len() {
        let mut targets: Vec<usize> = next[b].clone();
        targets.sort_unstable();
        targets.dedup();
        if targets.len() != 2 {
            continue;
        }
        for &s in &targets {
            if s == b || entered[s] != 1 {
                continue;
            }
            let dominated = dominated_by(function, &next, s);
            let Some(group) = closed_group(function, b, &dominated, &defs, &uses) else { continue };
            if std::env::var_os("LLRM_Z80_SINK_WHY").is_some() {
                eprintln!("sink: bb{} -> bb{}: {:?}", function.blocks[b].id, function.blocks[s].id,
                    group.iter().map(|&at| (function.blocks[b].insts[at].kind, function.blocks[b].insts[at].operands.clone())).collect::<Vec<_>>());
            }
            let mut moved = Vec::with_capacity(group.len());
            for &at in group.iter().rev() {
                moved.push(function.blocks[b].insts.remove(at));
            }
            moved.reverse();
            function.blocks[s].insts.splice(0..0, moved);
            return true;
        }
    }
    false
}

/// The blocks every path from the entry to which passes through `s`.
fn dominated_by(
    function: &MFunction,
    next: &[Vec<usize>],
    s: usize,
) -> HashSet<usize> {
    let mut reached = HashSet::default();
    let mut work = vec![0usize];
    while let Some(block) = work.pop() {
        if block == s || !reached.insert(block) {
            continue;
        }
        work.extend(next[block].iter().copied());
    }
    (0..function.blocks.len()).filter(|block| !reached.contains(block)).collect()
}

fn movable(inst: &Inst) -> bool {
    // a marker (`ADJCALLSTACKDOWN`) defines nothing: it stays beside its call
    inst.operands.iter().any(|operand| matches!(operand, Operand::Def(_)))
        && !(inst.call || inst.barrier || inst.terminator || inst.may_store || inst.may_load || inst.side_effects)
}

/// Whether two physical operands are one register.
fn overlap(
    a: Reg,
    b: Reg,
) -> bool {
    match (a, b) {
        (Reg::Phys(a), Reg::Phys(b)) => Z80Registers::overlaps(a, b),
        _ => false,
    }
}

/// The indices of the instructions of block `b` that can move together, or
/// none: the greatest set whose every member's requirements ([`needs`]) are
/// met by members. Each requirement is "that instruction moves too", so a
/// dropped member drops those that need it, once, in one pass.
fn closed_group(
    function: &MFunction,
    b: usize,
    dominated: &HashSet<usize>,
    defs: &HashMap<u32, Vec<(usize, usize)>>,
    uses: &HashMap<u32, Vec<(usize, usize)>>,
) -> Option<Vec<usize>> {
    let insts = &function.blocks[b].insts;
    let end = insts.iter().position(|inst| inst.terminator).unwrap_or(insts.len());
    let mut group: Vec<bool> = (0..insts.len()).map(|at| at < end && movable(&insts[at])).collect();
    let needs = needs(insts, end, &group, b, dominated, defs, uses);
    let mut needed_by: Vec<Vec<usize>> = vec![Vec::new(); insts.len()];
    let mut dropped = Vec::new();
    for (at, need) in needs.iter().enumerate() {
        match need {
            Some(need) => need.iter().for_each(|&other| needed_by[other].push(at)),
            None if group[at] => {
                group[at] = false;
                dropped.push(at);
            }
            None => {}
        }
    }
    dropped.extend((0..end).filter(|&at| !group[at] && needs[at].is_some()));
    while let Some(gone) = dropped.pop() {
        for &at in &needed_by[gone] {
            if group[at] {
                group[at] = false;
                dropped.push(at);
            }
        }
    }
    let chosen: Vec<usize> = (0..end).filter(|&at| group[at]).collect();
    // what the pin's MachineSink moves one instruction at a time is not this pass's: a group is here for the
    // flag chain it holds together (a reader of the flags and, by `needs`, the setter it reads)
    chosen.iter().any(|&at| insts[at].reads_flags()).then_some(chosen)
}

/// For each instruction `at` below `end` that is a candidate, the instructions
/// that must move with it, or `None` where it cannot move whatever the rest
/// does. Computed in one scan each way, so the cost is linear in the block:
/// the instruction that last wrote or next touches a register comes from a
/// table of the registers seen so far, not from a search of the block.
fn needs(
    insts: &[Inst],
    end: usize,
    candidate: &[bool],
    b: usize,
    dominated: &HashSet<usize>,
    defs: &HashMap<u32, Vec<(usize, usize)>>,
    uses: &HashMap<u32, Vec<(usize, usize)>>,
) -> Vec<Option<Vec<usize>>> {
    let mut needs: Vec<Option<Vec<usize>>> = vec![Some(Vec::new()); insts.len()];
    // the instruction at `n` is a member: a later reader outside the group (or past the branch) cannot be
    let member = |needs: &mut Option<Vec<usize>>, n: usize| match needs {
        Some(list) if n < end => list.push(n),
        _ => *needs = None,
    };

    // the status flags are no operand: an instruction `sets_flags` or `reads_flags`
    let mut next_flags: Option<usize> = None;
    // the last write of each physical register and the next touch of it (and whether that touch reads it)
    let mut next_touch: Vec<(Reg, usize, bool)> = Vec::new();
    let mut last_def_in_group: HashMap<u32, usize> = HashMap::default();
    for (at, inst) in insts.iter().enumerate().take(end) {
        for operand in &inst.operands {
            if let Operand::Def(Reg::Virt(v)) = operand {
                last_def_in_group.insert(*v, at);
            }
        }
    }
    for at in (0..insts.len()).rev() {
        let inst = &insts[at];
        if candidate[at] {
            if inst.sets_flags() {
                if let Some(n) = next_flags.filter(|&n| insts[n].reads_flags()) {
                    member(&mut needs[at], n);
                }
            }
            for operand in &inst.operands {
                if let Operand::Def(phys @ Reg::Phys(_)) = operand {
                    // the next instruction to touch it, the branch included, must write it, or none
                    let first = next_touch.iter().filter(|(reg, ..)| overlap(*reg, *phys)).map(|&(_, n, _)| n).min();
                    if let Some(n) = first {
                        let read = next_touch.iter().any(|&(reg, m, read)| m == n && read && overlap(reg, *phys));
                        if read {
                            member(&mut needs[at], n);
                        }
                    }
                }
            }
        }
        if inst.reads_flags() || inst.sets_flags() {
            next_flags = Some(at);
        }
        for operand in &inst.operands {
            let (reg @ Reg::Phys(_), read) = (match operand {
                Operand::Use(reg) => (*reg, true),
                Operand::Def(reg) => (*reg, false),
                _ => continue,
            }) else {
                continue;
            };
            match next_touch.iter_mut().find(|(seen, ..)| *seen == reg) {
                Some(entry) if entry.1 == at => entry.2 |= read,
                Some(entry) => *entry = (reg, at, read),
                None => next_touch.push((reg, at, read)),
            }
        }
    }

    let mut last_setter: Option<usize> = None;
    let mut last_write: Vec<(Reg, usize)> = Vec::new();
    for (at, inst) in insts.iter().enumerate().take(end) {
        if candidate[at] {
            if inst.reads_flags() {
                match last_setter {
                    Some(w) => member(&mut needs[at], w),
                    None => needs[at] = None,
                }
            }
            for operand in &inst.operands {
                match operand {
                    Operand::Def(Reg::Virt(v)) => {
                        // a two-address step defines its register twice: every write is the group's
                        let Some(all) = defs.get(v) else {
                            needs[at] = None;
                            continue;
                        };
                        for &(block, n) in all {
                            if block != b {
                                needs[at] = None;
                            } else {
                                member(&mut needs[at], n);
                            }
                        }
                        for &(block, n) in uses.get(v).map_or(&[][..], Vec::as_slice) {
                            // a two-address step reads what it writes
                            if block == b && n == at {
                                continue;
                            }
                            if block == b && n > at {
                                member(&mut needs[at], n);
                            } else if block == b || !dominated.contains(&block) {
                                needs[at] = None;
                            }
                        }
                    }
                    Operand::Use(Reg::Virt(v)) => {
                        // a later write in the block would be read by the moved reader
                        if last_def_in_group.get(v).is_some_and(|&last| last > at) {
                            needs[at] = None;
                        }
                    }
                    Operand::Use(phys @ Reg::Phys(_)) => {
                        // what it reads was written by an earlier member of the group
                        let writer = last_write.iter().filter(|(reg, _)| overlap(*reg, *phys)).map(|&(_, n)| n).max();
                        match writer {
                            Some(w) => member(&mut needs[at], w),
                            None => needs[at] = None,
                        }
                    }
                    _ => {}
                }
            }
        }
        if inst.sets_flags() {
            last_setter = Some(at);
        }
        for operand in &inst.operands {
            if let Operand::Def(reg @ Reg::Phys(_)) = operand {
                match last_write.iter_mut().find(|(seen, _)| seen == reg) {
                    Some(entry) => entry.1 = at,
                    None => last_write.push((*reg, at)),
                }
            }
        }
    }
    needs
}
