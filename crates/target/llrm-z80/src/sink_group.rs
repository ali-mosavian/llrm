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
/// none: the greatest set whose every member passes [`keeps`].
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
    loop {
        let mut dropped = false;
        for at in 0..end {
            if group[at] && !keeps(insts, end, at, &group, b, dominated, defs, uses) {
                group[at] = false;
                dropped = true;
            }
        }
        if !dropped {
            break;
        }
    }
    let chosen: Vec<usize> = (0..end).filter(|&at| group[at]).collect();
    // what the pin's MachineSink moves one instruction at a time is not this pass's: a group is here for the
    // flag chain it holds together (a reader of the flags and, by `keeps`, the setter it reads)
    chosen.iter().any(|&at| insts[at].reads_flags()).then_some(chosen)
}

#[allow(clippy::too_many_arguments)]
fn keeps(
    insts: &[Inst],
    end: usize,
    at: usize,
    group: &[bool],
    b: usize,
    dominated: &HashSet<usize>,
    defs: &HashMap<u32, Vec<(usize, usize)>>,
    uses: &HashMap<u32, Vec<(usize, usize)>>,
) -> bool {
    let inst = &insts[at];
    // the status flags are no operand: an instruction `sets_flags` or `reads_flags`
    if inst.sets_flags() {
        for (n, later) in insts.iter().enumerate().skip(at + 1) {
            if later.reads_flags() {
                if n >= end || !group[n] {
                    return false;
                }
                break;
            }
            if later.sets_flags() {
                break;
            }
        }
    }
    if inst.reads_flags() {
        let setter = insts[..at].iter().rposition(Inst::sets_flags);
        if !setter.is_some_and(|w| group[w]) {
            return false;
        }
    }
    for operand in &inst.operands {
        match operand {
            Operand::Def(Reg::Virt(v)) => {
                // a two-address step defines its register twice: every write is the group's
                if !defs.get(v).is_some_and(|all| all.iter().all(|&(block, n)| block == b && n < end && group[n])) {
                    return false;
                }
                for &(block, n) in uses.get(v).map_or(&[][..], Vec::as_slice) {
                    // a two-address step reads what it writes
                    if block == b && n == at {
                        continue;
                    }
                    let inside = block == b && n > at && n < end && group[n];
                    if !(inside || (block != b && dominated.contains(&block))) {
                        return false;
                    }
                }
            }
            Operand::Use(Reg::Virt(v)) => {
                // a later write in the block would be read by the moved reader
                let redefined = insts[at + 1..end]
                    .iter()
                    .any(|later| later.operands.iter().any(|o| matches!(o, Operand::Def(Reg::Virt(d)) if d == v)));
                if redefined {
                    return false;
                }
            }
            Operand::Def(phys @ Reg::Phys(_)) => {
                // the next instruction to touch it, the branch included, must write it, or none
                for (n, later) in insts.iter().enumerate().skip(at + 1) {
                    if later.operands.iter().any(|o| matches!(o, Operand::Use(r) if overlap(*r, *phys))) {
                        if n >= end || !group[n] {
                            return false;
                        }
                        break;
                    }
                    if later.operands.iter().any(|o| matches!(o, Operand::Def(r) if overlap(*r, *phys))) {
                        break;
                    }
                }
            }
            Operand::Use(phys @ Reg::Phys(_)) => {
                // what it reads was written by an earlier member of the group
                let writer = insts[..at].iter().rposition(|earlier| {
                    earlier.operands.iter().any(|o| matches!(o, Operand::Def(r) if overlap(*r, *phys)))
                });
                if !writer.is_some_and(|w| group[w]) {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}
