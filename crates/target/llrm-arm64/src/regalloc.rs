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

/// A register's live ranges: half-open `[start, end)` over operation points
/// (an operation `i` reads at `2i` and writes at `2i + 1`), disjoint and
/// sorted.
#[derive(Clone, Debug, Default)]
struct Ranges(Vec<(u32, u32)>);

impl Ranges {
    fn push(
        &mut self,
        start: u32,
        end: u32,
    ) {
        self.0.push((start, end));
    }

    /// Sorted, and adjoining or overlapping ranges joined.
    fn normalize(&mut self) {
        self.0.sort_unstable();
        let mut joined: Vec<(u32, u32)> = Vec::with_capacity(self.0.len());
        for &(start, end) in &self.0 {
            match joined.last_mut() {
                Some(last) if start <= last.1 => last.1 = last.1.max(end),
                _ => joined.push((start, end)),
            }
        }
        self.0 = joined;
    }

    fn overlaps(
        &self,
        other: &Ranges,
    ) -> bool {
        let (mut a, mut b) = (0, 0);
        while a < self.0.len() && b < other.0.len() {
            let (x, y) = (self.0[a], other.0[b]);
            if x.0 < y.1 && y.0 < x.1 {
                return true;
            }
            if x.1 <= y.1 { a += 1 } else { b += 1 }
        }
        false
    }

    fn start(&self) -> u32 {
        self.0.first().map_or(0, |one| one.0)
    }

    fn extend(
        &mut self,
        other: &Ranges,
    ) {
        self.0.extend(other.0.iter().copied());
        self.normalize();
    }
}

/// The live ranges of every register, and the points of the calls.
fn analyse(function: &MFunc) -> (HashMap<Reg, Ranges>, Vec<u32>, HashMap<Reg, f64>) {
    let mut position = 0u32;
    let mut extents = Vec::new();
    let mut calls = Vec::new();
    let mut gen_kill: Vec<(BTreeSet<Reg>, BTreeSet<Reg>)> = Vec::new();
    for block in &function.blocks {
        let first = position;
        let (mut gen_set, mut kill_set) = (BTreeSet::new(), BTreeSet::new());
        for op in &block.ops {
            let (op_defs, op_uses) = op.defs_and_uses();
            for register in &op_uses {
                if !kill_set.contains(register) {
                    gen_set.insert(*register);
                }
            }
            kill_set.extend(op_defs);
            if matches!(op, Op::Call { .. }) {
                calls.push(position);
            }
            position += 2;
        }
        extents.push((first, position));
        gen_kill.push((gen_set, kill_set));
    }
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
    // How deeply nested in loops a block is: a back edge `b -> h` (h at or
    // before b) makes the blocks h..=b one level deeper.
    let mut depth = vec![0u32; function.blocks.len()];
    for (at, targets) in successors.iter().enumerate() {
        for &target in targets {
            if target <= at {
                for block in target..=at {
                    depth[block] += 1;
                }
            }
        }
    }
    let mut ranges: HashMap<Reg, Ranges> = HashMap::new();
    let mut weight: HashMap<Reg, f64> = HashMap::new();
    for (at, block) in function.blocks.iter().enumerate() {
        let (first, last) = extents[at];
        // Walking backward, the end of the segment each live register has open.
        let mut open: HashMap<Reg, u32> = live_out[at].iter().map(|register| (*register, last)).collect();
        let frequency = 10f64.powi(depth[at].min(6) as i32);
        for (index, op) in block.ops.iter().enumerate().rev() {
            let point = first + 2 * index as u32;
            let (defs, uses) = op.defs_and_uses();
            for register in &defs {
                *weight.entry(*register).or_default() += frequency;
                match open.remove(register) {
                    Some(end) => ranges.entry(*register).or_default().push(point + 1, end),
                    // Defined and never read.
                    None => ranges.entry(*register).or_default().push(point + 1, point + 2),
                }
            }
            for register in &uses {
                *weight.entry(*register).or_default() += frequency;
                open.entry(*register).or_insert(point + 1);
            }
        }
        for (register, end) in open {
            ranges.entry(register).or_default().push(first, end);
        }
    }
    for one in ranges.values_mut() {
        one.normalize();
    }
    (ranges, calls, weight)
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

/// Joins the virtual registers a copy connects whose live ranges do not
/// overlap, so the copy disappears (the register coalescer,
/// `RegisterCoalescer`, for the copies phi elimination leaves): the registers
/// are renamed in place.
fn coalesce(function: &mut MFunc) {
    let (ranges, _, weight) = analyse(function);
    let mut ranges = ranges;
    let mut parent: HashMap<Reg, Reg> = HashMap::new();
    fn find(
        parent: &mut HashMap<Reg, Reg>,
        register: Reg,
    ) -> Reg {
        let mut root = register;
        while let Some(&up) = parent.get(&root) {
            root = up;
        }
        let mut walk = register;
        while let Some(&up) = parent.get(&walk) {
            parent.insert(walk, root);
            walk = up;
        }
        root
    }
    // The copies, the heaviest (deepest in loops) first.
    let mut copies: Vec<(f64, Reg, Reg)> = Vec::new();
    for block in &function.blocks {
        for op in &block.ops {
            if let Op::Copy { dst, src } = op
                && dst.is_virtual()
                && src.is_virtual()
            {
                let w = weight.get(dst).copied().unwrap_or(0.0).min(weight.get(src).copied().unwrap_or(0.0));
                copies.push((w, *dst, *src));
            }
        }
    }
    copies.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut merged = false;
    for (_, dst, src) in copies {
        let (a, b) = (find(&mut parent, dst), find(&mut parent, src));
        if a == b {
            continue;
        }
        let (Some(left), Some(right)) = (ranges.get(&a), ranges.get(&b)) else { continue };
        if left.overlaps(right) {
            continue;
        }
        let right = right.clone();
        ranges.get_mut(&a).expect("present").extend(&right);
        parent.insert(b, a);
        merged = true;
    }
    if !merged {
        return;
    }
    for block in &mut function.blocks {
        for op in &mut block.ops {
            op.visit(&mut |register, _| match register {
                RegRef::Gpr(slot) => {
                    if let Reg::Gpr(root) = find(&mut parent, Reg::Gpr(*slot)) {
                        *slot = root;
                    }
                }
                RegRef::Fpr(slot) => {
                    if let Reg::Fpr(root) = find(&mut parent, Reg::Fpr(*slot)) {
                        *slot = root;
                    }
                }
            });
        }
    }
}

/// `function` with every register physical.
pub fn allocate(function: &mut MFunc) -> Allocation {
    coalesce(function);
    let (ranges, calls, weight) = analyse(function);
    // What each physical register holds: its own live ranges (arguments,
    // results) and, once a virtual register is given it, that one's; a call
    // destroys every caller-saved register at its point.
    let mut occupied: HashMap<Reg, Ranges> = HashMap::new();
    for (register, live) in ranges.iter().filter(|(register, _)| !register.is_virtual()) {
        occupied.entry(*register).or_default().extend(live);
    }
    let clobbered = |register: Reg| !pool::callee_saved(register);
    let mut universe: Vec<Reg> = Vec::new();
    universe.extend(pool::GPR_CALLER_SAVED.iter().map(|n| Reg::Gpr(Gpr(*n))));
    universe.extend(pool::GPR_CALLEE_SAVED.iter().map(|n| Reg::Gpr(Gpr(*n))));
    universe.extend(pool::FPR_CALLER_SAVED.iter().map(|n| Reg::Fpr(Vreg(*n))));
    universe.extend(pool::FPR_CALLEE_SAVED.iter().map(|n| Reg::Fpr(Vreg(*n))));
    for &call in &calls {
        for &register in &universe {
            if clobbered(register) {
                occupied.entry(register).or_default().extend(&Ranges(vec![(call + 1, call + 2)]));
            }
        }
    }
    // Copy partners, for hints.
    let mut partners: HashMap<Reg, Vec<Reg>> = HashMap::new();
    for block in &function.blocks {
        for op in &block.ops {
            if let Op::Copy { dst, src } = op {
                partners.entry(*dst).or_default().push(*src);
                partners.entry(*src).or_default().push(*dst);
            }
        }
    }
    // Heaviest first, by use frequency over length; ties by position.
    let mut order: Vec<Reg> = ranges.keys().copied().filter(|register| register.is_virtual()).collect();
    let priority = |register: &Reg| {
        let live = &ranges[register];
        let length: u32 = live.0.iter().map(|(start, end)| end - start).sum();
        (weight.get(register).copied().unwrap_or(0.0) / f64::from(length.max(1)), std::cmp::Reverse(live.start()))
    };
    order.sort_by(|a, b| priority(b).partial_cmp(&priority(a)).unwrap_or(std::cmp::Ordering::Equal).then(a.cmp(b)));
    let mut assigned: HashMap<Reg, Reg> = HashMap::new();
    let mut spilled: HashMap<Reg, u32> = HashMap::new();
    let mut used_callee_saved: BTreeSet<Reg> = BTreeSet::new();
    for register in order {
        let live = &ranges[&register];
        let class: Vec<Reg> = match register {
            Reg::Gpr(_) => universe.iter().copied().filter(|one| matches!(one, Reg::Gpr(_))).collect(),
            Reg::Fpr(_) => universe.iter().copied().filter(|one| matches!(one, Reg::Fpr(_))).collect(),
        };
        let fits = |candidate: &Reg| !occupied.get(candidate).is_some_and(|held| held.overlaps(live));
        // A register a copy partner has (or is) is tried first.
        let hinted = partners
            .get(&register)
            .into_iter()
            .flatten()
            .map(|partner| if partner.is_virtual() { assigned.get(partner).copied() } else { Some(*partner) })
            .flatten()
            .find(|candidate| class.contains(candidate) && fits(candidate));
        match hinted.or_else(|| class.iter().copied().find(fits)) {
            Some(chosen) => {
                if pool::callee_saved(chosen) {
                    used_callee_saved.insert(chosen);
                }
                occupied.entry(chosen).or_default().extend(live);
                assigned.insert(register, chosen);
            }
            None => {
                let slot = function.slot(8, 8);
                spilled.insert(register, slot);
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
