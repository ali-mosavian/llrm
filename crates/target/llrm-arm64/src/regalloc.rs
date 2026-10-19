//! Register allocation: linear scan over one live interval per register. It is
//! the allocator the spike needs, not LLVM's greedy (`RAGreedy`): an interval
//! has no holes, a spilled one lives in its frame object, and every use of it
//! is reloaded into a reserved scratch register. An interval that crosses a
//! call takes a callee-saved register or is spilled.
//!
//! DEVIATE 1: LLVM allocates with `RAGreedy` on the shared machine IR. This
//! allocator stands in until the core machine passes run on arm64 LIR (see
//! docs/architecture/arm64-spike.md); it is not a translation.

use std::collections::{BTreeSet, HashMap};

use crate::mc::inst::*;
use crate::mc::visit::{Reg, RegRef, Role};
use crate::mfunc::{Callee, MFunc, Op};

/// The registers the allocator hands out, in order of preference, and those it
/// keeps for reloading spilled operands.
pub mod pool {
    use super::*;

    pub const GPR_CALLER_SAVED: [u32; 15] = [9, 10, 11, 12, 13, 14, 8, 7, 6, 5, 4, 3, 2, 1, 0];
    pub const GPR_CALLEE_SAVED: [u32; 10] = [19, 20, 21, 22, 23, 24, 25, 26, 27, 28];
    pub const GPR_SCRATCH: [u32; 3] = [15, 16, 17];
    pub const FPR_CALLER_SAVED: [u32; 21] =
        [16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 7, 6, 5, 4, 3, 2, 1, 0];
    pub const FPR_CALLEE_SAVED: [u32; 8] = [8, 9, 10, 11, 12, 13, 14, 15];
    pub const FPR_SCRATCH: [u32; 3] = [29, 30, 31];

    /// Whether the call leaves `register` as it was.
    pub fn callee_saved(register: Reg) -> bool {
        match register {
            Reg::Gpr(gpr) => (19..=28).contains(&gpr.0),
            Reg::Fpr(fpr) => (8..=15).contains(&fpr.0),
        }
    }
}

/// What allocation leaves for the frame: which callee-saved registers the body
/// writes.
#[derive(Debug, Default)]
pub struct Allocation {
    pub saved_gprs: Vec<u32>,
    pub saved_fprs: Vec<u32>,
}

#[derive(Clone, Copy, Debug)]
struct Interval {
    reg: Reg,
    start: u32,
    end: u32,
}

/// Positions are `2 * index` for an operation; a use is read at the position, a
/// definition written at the position + 1.
fn analyse(function: &MFunc) -> (Vec<Interval>, Vec<u32>) {
    let mut starts: HashMap<Reg, u32> = HashMap::new();
    let mut ends: HashMap<Reg, u32> = HashMap::new();
    let mut touch = |register: Reg, from: u32, to: u32| {
        let start = starts.entry(register).or_insert(from);
        *start = (*start).min(from);
        let end = ends.entry(register).or_insert(to);
        *end = (*end).max(to);
    };
    // Block extents, and the live-in and live-out sets by iterating to a fixed
    // point.
    let mut position = 0u32;
    let mut extents = Vec::new();
    let mut calls = Vec::new();
    let mut uses_defs: Vec<(Vec<(Reg, u32)>, Vec<(Reg, u32)>)> = Vec::new();
    let mut gen_kill: Vec<(BTreeSet<Reg>, BTreeSet<Reg>)> = Vec::new();
    for block in &function.blocks {
        let first = position;
        let (mut uses, mut defs) = (Vec::new(), Vec::new());
        let (mut gen_set, mut kill_set) = (BTreeSet::new(), BTreeSet::new());
        for op in &block.ops {
            let (op_defs, op_uses) = op.defs_and_uses();
            for register in &op_uses {
                if !kill_set.contains(register) {
                    gen_set.insert(*register);
                }
                uses.push((*register, position));
            }
            for register in &op_defs {
                kill_set.insert(*register);
                defs.push((*register, position + 1));
            }
            if matches!(op, Op::Call { .. }) {
                calls.push(position);
            }
            position += 2;
        }
        extents.push((first, position));
        uses_defs.push((uses, defs));
        gen_kill.push((gen_set, kill_set));
    }
    // The control-flow successors, from the labels the branches name.
    let labels: HashMap<&str, usize> =
        function.blocks.iter().enumerate().map(|(at, one)| (one.label.as_str(), at)).collect();
    let successors: Vec<Vec<usize>> = function
        .blocks
        .iter()
        .enumerate()
        .map(|(at, block)| successors_of(block, at, function.blocks.len(), &labels))
        .collect();
    let mut live_in: Vec<BTreeSet<Reg>> = vec![BTreeSet::new(); function.blocks.len()];
    let mut live_out: Vec<BTreeSet<Reg>> = vec![BTreeSet::new(); function.blocks.len()];
    let mut changed = true;
    while changed {
        changed = false;
        for at in (0..function.blocks.len()).rev() {
            let mut out = BTreeSet::new();
            for &successor in &successors[at] {
                out.extend(live_in[successor].iter().copied());
            }
            let mut input: BTreeSet<Reg> = out.difference(&gen_kill[at].1).copied().collect();
            input.extend(gen_kill[at].0.iter().copied());
            if out != live_out[at] || input != live_in[at] {
                live_out[at] = out;
                live_in[at] = input;
                changed = true;
            }
        }
    }
    for (at, (first, last)) in extents.iter().enumerate() {
        for register in &live_in[at] {
            touch(*register, *first, *first);
        }
        for register in &live_out[at] {
            touch(*register, *last, *last);
        }
        let (uses, defs) = &uses_defs[at];
        for (register, at_position) in uses {
            touch(*register, *at_position, *at_position);
        }
        for (register, at_position) in defs {
            touch(*register, *at_position, *at_position);
        }
    }
    let mut intervals: Vec<Interval> = starts
        .iter()
        .map(|(register, start)| Interval { reg: *register, start: *start, end: ends[register] })
        .collect();
    intervals.sort_by_key(|one| (one.start, one.end));
    (intervals, calls)
}

fn successors_of(
    block: &crate::mfunc::MBlock,
    at: usize,
    count: usize,
    labels: &HashMap<&str, usize>,
) -> Vec<usize> {
    let mut out = Vec::new();
    let mut falls_through = true;
    for op in &block.ops {
        match op {
            Op::I(Inst::Branch { link: false, target: Target::Label(label) }) => {
                out.extend(labels.get(label.as_str()).copied());
                falls_through = false;
            }
            Op::I(Inst::BranchCond { target: Target::Label(label), .. })
            | Op::I(Inst::CompareBranch { target: Target::Label(label), .. })
            | Op::I(Inst::TestBranch { target: Target::Label(label), .. }) => {
                out.extend(labels.get(label.as_str()).copied());
            }
            Op::Return { .. } => falls_through = false,
            Op::I(Inst::Brk { .. }) => falls_through = false,
            _ => {}
        }
    }
    if falls_through && at + 1 < count {
        out.push(at + 1);
    }
    out
}

/// `function` with every register physical.
pub fn allocate(function: &mut MFunc) -> Allocation {
    let (intervals, calls) = analyse(function);
    // Busy ranges of physical registers, from the same analysis.
    let mut fixed: HashMap<Reg, Vec<(u32, u32)>> = HashMap::new();
    for interval in intervals.iter().filter(|one| !one.reg.is_virtual()) {
        fixed.entry(interval.reg).or_default().push((interval.start, interval.end));
    }
    let crosses_call = |interval: &Interval| calls.iter().any(|&call| interval.start < call && call + 1 < interval.end);
    let overlaps_fixed = |register: Reg, interval: &Interval| {
        fixed.get(&register).is_some_and(|ranges| ranges.iter().any(|&(s, e)| s <= interval.end && interval.start <= e))
    };
    // Hints: a physical register that a copy joins this register to.
    let mut hints: HashMap<Reg, Reg> = HashMap::new();
    for block in &function.blocks {
        for op in &block.ops {
            if let Op::Copy { dst, src } = op {
                if dst.is_virtual() && !src.is_virtual() {
                    hints.entry(*dst).or_insert(*src);
                } else if src.is_virtual() && !dst.is_virtual() {
                    hints.entry(*src).or_insert(*dst);
                }
            }
        }
    }
    let mut assigned: HashMap<Reg, Reg> = HashMap::new();
    let mut spilled: HashMap<Reg, u32> = HashMap::new();
    let mut active: Vec<(Interval, Reg)> = Vec::new();
    let mut used_callee_saved: BTreeSet<Reg> = BTreeSet::new();
    for interval in intervals.iter().filter(|one| one.reg.is_virtual()) {
        active.retain(|(other, _)| other.end >= interval.start);
        let crossing = crosses_call(interval);
        let candidates: Vec<Reg> = match interval.reg {
            Reg::Gpr(_) => {
                let caller = pool::GPR_CALLER_SAVED.iter().map(|n| Reg::Gpr(Gpr(*n)));
                let callee = pool::GPR_CALLEE_SAVED.iter().map(|n| Reg::Gpr(Gpr(*n)));
                if crossing { callee.collect() } else { caller.chain(callee).collect() }
            }
            Reg::Fpr(_) => {
                let caller = pool::FPR_CALLER_SAVED.iter().map(|n| Reg::Fpr(Vreg(*n)));
                let callee = pool::FPR_CALLEE_SAVED.iter().map(|n| Reg::Fpr(Vreg(*n)));
                if crossing { callee.collect() } else { caller.chain(callee).collect() }
            }
        };
        let free =
            |register: &Reg| !active.iter().any(|(_, held)| held == register) && !overlaps_fixed(*register, interval);
        let hinted = hints.get(&interval.reg).copied().filter(|hint| candidates.contains(hint) && free(hint));
        let choice = hinted.or_else(|| candidates.iter().copied().find(free));
        match choice {
            Some(register) => {
                if pool::callee_saved(register) {
                    used_callee_saved.insert(register);
                }
                assigned.insert(interval.reg, register);
                active.push((*interval, register));
            }
            None => {
                // Spill the interval that lives longest: this one, or an active
                // one whose register this could take.
                let victim = active
                    .iter()
                    .enumerate()
                    .filter(|(_, (other, held))| {
                        candidates.contains(held)
                            && other.end > interval.end
                            && std::mem::discriminant(&other.reg) == std::mem::discriminant(&interval.reg)
                            && !overlaps_fixed(*held, interval)
                    })
                    .max_by_key(|(_, (other, _))| other.end)
                    .map(|(at, _)| at);
                match victim {
                    Some(at) => {
                        let (other, held) = active.remove(at);
                        assigned.remove(&other.reg);
                        let slot = function.slot(8, 8);
                        spilled.insert(other.reg, slot);
                        assigned.insert(interval.reg, held);
                        active.push((*interval, held));
                    }
                    None => {
                        let slot = function.slot(8, 8);
                        spilled.insert(interval.reg, slot);
                    }
                }
            }
        }
    }
    rewrite(function, &assigned, &spilled);
    let mut allocation = Allocation::default();
    for register in used_callee_saved {
        match register {
            Reg::Gpr(gpr) => allocation.saved_gprs.push(gpr.0),
            Reg::Fpr(fpr) => allocation.saved_fprs.push(fpr.0),
        }
    }
    allocation
}

/// Replaces each virtual register with its physical one; a spilled one is
/// reloaded into a scratch register before each use and stored after each
/// definition.
fn rewrite(
    function: &mut MFunc,
    assigned: &HashMap<Reg, Reg>,
    spilled: &HashMap<Reg, u32>,
) {
    for block in &mut function.blocks {
        let mut out = Vec::with_capacity(block.ops.len());
        for mut op in std::mem::take(&mut block.ops) {
            // A copy between a spill slot and a register is a load or store.
            if let Op::Copy { dst, src } = &op
                && let Some(done) = spilled_copy(*dst, *src, assigned, spilled)
            {
                out.extend(done);
                continue;
            }
            let (mut loads, mut stores) = (Vec::new(), Vec::new());
            let (mut next_gpr, mut next_fpr) = (0usize, 0usize);
            // One scratch register per spilled register of this operation.
            let mut scratch: HashMap<Reg, Reg> = HashMap::new();
            op.visit(&mut |register, role| {
                let key = register.key();
                let replacement = if let Some(slot) = spilled.get(&key) {
                    let scratch_reg = *scratch
                        .entry(key)
                        .or_insert_with(
                            || match key {
                                Reg::Gpr(_) => {
                                    next_gpr += 1;
                                    Reg::Gpr(Gpr(pool::GPR_SCRATCH[next_gpr - 1]))
                                }
                                Reg::Fpr(_) => {
                                    next_fpr += 1;
                                    Reg::Fpr(Vreg(pool::FPR_SCRATCH[next_fpr - 1]))
                                }
                            },
                        );
                    if role.reads() && !loads.iter().any(|(r, _): &(Reg, u32)| *r == scratch_reg) {
                        loads.push((scratch_reg, *slot));
                    }
                    if role.writes() && !stores.iter().any(|(r, _): &(Reg, u32)| *r == scratch_reg) {
                        stores.push((scratch_reg, *slot));
                    }
                    scratch_reg
                } else {
                    assigned.get(&key).copied().unwrap_or(key)
                };
                match (register, replacement) {
                    (RegRef::Gpr(slot), Reg::Gpr(new)) => *slot = new,
                    (RegRef::Fpr(slot), Reg::Fpr(new)) => *slot = new,
                    _ => unreachable!("a register keeps its class"),
                }
            });
            for (register, slot) in loads {
                out.push(spill_access(register, slot, true));
            }
            let is_noop = matches!(&op, Op::Copy { dst, src } if dst == src);
            let is_return = matches!(op, Op::Return { .. });
            if !is_noop {
                out.push(op);
            }
            debug_assert!(!(is_return && !stores.is_empty()));
            for (register, slot) in stores {
                out.push(spill_access(register, slot, false));
            }
        }
        block.ops = out;
    }
}

fn spilled_copy(
    dst: Reg,
    src: Reg,
    assigned: &HashMap<Reg, Reg>,
    spilled: &HashMap<Reg, u32>,
) -> Option<Vec<Op>> {
    let physical = |register: Reg| assigned.get(&register).copied().unwrap_or(register);
    match (spilled.get(&dst), spilled.get(&src)) {
        (None, None) => None,
        (Some(slot), None) => Some(vec![spill_access(physical(src), *slot, false)]),
        (None, Some(slot)) => Some(vec![spill_access(physical(dst), *slot, true)]),
        (Some(to), Some(from)) => {
            let scratch = match dst {
                Reg::Gpr(_) => Reg::Gpr(Gpr(pool::GPR_SCRATCH[0])),
                Reg::Fpr(_) => Reg::Fpr(Vreg(pool::FPR_SCRATCH[0])),
            };
            Some(vec![spill_access(scratch, *from, true), spill_access(scratch, *to, false)])
        }
    }
}

fn spill_access(
    register: Reg,
    slot: u32,
    load: bool,
) -> Op {
    let addr = Addr::Slot { slot, offset: 0 };
    let mem = match register {
        Reg::Gpr(reg) => Memory::Int { width: Width::X, bytes: 8, sign: false, reg },
        Reg::Fpr(reg) => Memory::Fp { bytes: 8, reg },
    };
    Op::I(if load { Inst::Load { mem, addr } } else { Inst::Store { mem, addr } })
}

#[allow(dead_code)]
fn unused(_: Callee) {}

#[allow(dead_code)]
fn unused_role(_: Role) {}
