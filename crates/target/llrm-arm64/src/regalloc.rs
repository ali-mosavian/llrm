//! Register allocation: linear scan over one live interval per register. It is
//! the allocator the spike needs, not LLVM's greedy (`RAGreedy`): an interval
//! has no holes, a spilled one lives in its frame object, and every use of it
//! is reloaded into a reserved scratch register. An interval that crosses a
//! call takes a callee-saved register or is spilled. A value that is one
//! constant (`movz`, `adrp`, an immediate) is never spilled and never takes a
//! callee-saved register of its own: each use makes it again (LLVM
//! rematerializes it: `TargetInstrInfo::isReMaterializable`). So is a chain of
//! such values, each read by the next and by nothing else that could change
//! it: the `adrp` and the `add` of a global's address are made again together.
//! A reload of a slot into the scratch register that still holds it (stored or
//! loaded since, nothing written to it, no call between) is dropped, and so is
//! a copy between two registers that already hold one value (`mov x19, x0`
//! then `mov x0, x19`).
//!
//! A 128-bit value (one a `q` access names) is spilled and copied whole
//! (`VectorMov`) and never takes `d8`-`d15`, whose upper half a call does not
//! keep.
//!
//! DEVIATE 1: LLVM allocates with `RAGreedy` on the shared machine IR. This
//! allocator stands in until the core machine passes run on arm64 LIR (see
//! docs/architecture/arm64-spike.md); it is not a translation.

use std::collections::{BTreeSet, HashMap, HashSet};

use llrm_codegen::model::lir::SrcLoc;

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
/// Each register's weight is its definitions and uses over the loop depth of
/// their blocks (10 per level); `uses` is the uses' share of it and `copies`
/// the share of those that are the source of a copy.
fn analyse(function: &MFunc) -> (HashMap<Reg, Ranges>, Vec<u32>, HashMap<Reg, f64>, Uses) {
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
        .map(|(at, block)| crate::mfunc::successors_of(block, at, function.blocks.len(), &labels))
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
    let mut use_weight = Uses::default();
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
                *use_weight.all.entry(*register).or_default() += frequency;
                if matches!(op, Op::Copy { src, .. } if src == register) {
                    *use_weight.copies.entry(*register).or_default() += frequency;
                }
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
    (ranges, calls, weight, use_weight)
}

/// The weight of a register's uses, and of those that are copies.
#[derive(Default)]
struct Uses {
    all: HashMap<Reg, f64>,
    copies: HashMap<Reg, f64>,
}

/// Joins the virtual registers a copy connects whose live ranges do not
/// overlap, so the copy disappears (the register coalescer,
/// `RegisterCoalescer`, for the copies phi elimination leaves): the registers
/// are renamed in place.
fn coalesce(function: &mut MFunc) {
    let (ranges, _, weight, _) = analyse(function);
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

/// The floating-point registers that hold 128 bits: a `q` load or store names
/// them (an instruction that makes a `q` value lists its destination here), and
/// so does every register a copy ties to one. Such a register is spilled and
/// copied whole, and a call preserves only the low 64 bits of `d8`-`d15`, so it
/// never takes one.
fn wide_registers(function: &MFunc) -> HashSet<Reg> {
    let mut wide: HashSet<Reg> = HashSet::new();
    let mut copies: Vec<(Reg, Reg)> = Vec::new();
    for op in function.blocks.iter().flat_map(|block| &block.ops) {
        match op {
            Op::I(Inst::Load { mem: Memory::Fp { bytes: 16, reg }, .. })
            | Op::I(Inst::Store { mem: Memory::Fp { bytes: 16, reg }, .. }) => {
                wide.insert(Reg::Fpr(*reg));
            }
            Op::I(Inst::VectorFill { rd, .. }) => {
                wide.insert(Reg::Fpr(*rd));
            }
            Op::I(Inst::LoadPair { first, second, .. }) | Op::I(Inst::StorePair { first, second, .. }) => {
                for memory in [first, second] {
                    if let Memory::Fp { bytes: 16, reg } = memory {
                        wide.insert(Reg::Fpr(*reg));
                    }
                }
            }
            Op::I(Inst::VectorMov { rd, rn }) => {
                wide.extend([Reg::Fpr(*rd), Reg::Fpr(*rn)]);
            }
            Op::Copy { dst: dst @ Reg::Fpr(_), src } => copies.push((*dst, *src)),
            _ => {}
        }
    }
    let mut changed = true;
    while changed {
        changed = false;
        for (dst, src) in &copies {
            if wide.contains(dst) != wide.contains(src) {
                wide.extend([*dst, *src]);
                changed = true;
            }
        }
    }
    wide
}

/// `function` with every register physical.
pub fn allocate(function: &mut MFunc) -> Allocation {
    coalesce(function);
    let (ranges, calls, weight, use_weight) = analyse(function);
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
    let candidates = rematerializable(function);
    let wide = wide_registers(function);
    let mut made_again: HashMap<Reg, Vec<Op>> = HashMap::new();
    let mut assigned: HashMap<Reg, Reg> = HashMap::new();
    let mut spilled: HashMap<Reg, u32> = HashMap::new();
    let mut used_callee_saved: BTreeSet<Reg> = BTreeSet::new();
    for register in order {
        let live = &ranges[&register];
        let class: Vec<Reg> = match register {
            Reg::Gpr(_) => universe.iter().copied().filter(|one| matches!(one, Reg::Gpr(_))).collect(),
            Reg::Fpr(_) => universe
                .iter()
                .copied()
                .filter(|one| matches!(one, Reg::Fpr(_)) && !(wide.contains(&register) && pool::callee_saved(*one)))
                .collect(),
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
            // A new callee-saved register (a save and a restore) for a value
            // one instruction makes is dearer than making it at each use.
            Some(chosen)
                if pool::callee_saved(chosen)
                    && !used_callee_saved.contains(&chosen)
                    && cheaper_again(&candidates, &register, &weight, &use_weight, 1.0) =>
            {
                made_again.insert(register, candidates[&register].0.clone());
            }
            Some(chosen) => {
                if pool::callee_saved(chosen) {
                    used_callee_saved.insert(chosen);
                }
                occupied.entry(chosen).or_default().extend(live);
                assigned.insert(register, chosen);
            }
            None if cheaper_again(&candidates, &register, &weight, &use_weight, 1.0 + use_weight.all[&register]) => {
                made_again.insert(register, candidates[&register].0.clone());
            }
            None => {
                let slot = if wide.contains(&register) { function.slot(16, 16) } else { function.slot(8, 8) };
                spilled.insert(register, slot);
            }
        }
    }
    let unused = unused_constants(function, &candidates, &made_again);
    rewrite(function, &assigned, &spilled, &made_again, &unused, &wide);
    let mut allocation = Allocation::default();
    for register in used_callee_saved {
        match register {
            Reg::Gpr(gpr) => allocation.saved_gprs.push(gpr.0),
            Reg::Fpr(fpr) => allocation.saved_fprs.push(fpr.0),
        }
    }
    allocation
}

/// The constants nothing reads once the ones made again at each use are
/// gone: the `adrp` whose only reader was such a global address.
fn unused_constants(
    function: &MFunc,
    candidates: &HashMap<Reg, (Vec<Op>, u32)>,
    made_again: &HashMap<Reg, Vec<Op>>,
) -> HashSet<Reg> {
    let mut unused: HashSet<Reg> = HashSet::new();
    loop {
        let mut read: HashSet<Reg> = HashSet::new();
        for op in function.blocks.iter().flat_map(|block| &block.ops) {
            let (defs, uses) = op.defs_and_uses();
            if defs.iter().any(|register| made_again.contains_key(register) || unused.contains(register)) {
                continue;
            }
            read.extend(uses);
        }
        let more: Vec<Reg> = candidates
            .keys()
            .copied()
            .filter(|register| {
                !made_again.contains_key(register) && !unused.contains(register) && !read.contains(register)
            })
            .collect();
        if more.is_empty() {
            return unused;
        }
        unused.extend(more);
    }
}

/// Replaces each virtual register with its physical one; a spilled one is
/// reloaded into a scratch register before each use and stored after each
/// definition.
fn rewrite(
    function: &mut MFunc,
    assigned: &HashMap<Reg, Reg>,
    spilled: &HashMap<Reg, u32>,
    made_again: &HashMap<Reg, Vec<Op>>,
    unused: &HashSet<Reg>,
    wide: &HashSet<Reg>,
) {
    let spill_slots: HashSet<u32> = spilled.values().copied().collect();
    for block in &mut function.blocks {
        let mut out: Vec<(Op, Option<SrcLoc>)> = Vec::with_capacity(block.ops.len());
        for (mut op, loc) in std::mem::take(&mut block.ops) {
            // The one definition of a value made at each use.
            if op
                .defs_and_uses()
                .0
                .iter()
                .any(|register| made_again.contains_key(register) || unused.contains(register))
            {
                continue;
            }
            // A copy of such a value is the definition, aimed at the copy's
            // target.
            if let Op::Copy { dst, src } = &op
                && let Some(def) = made_again.get(src)
                && !made_again.contains_key(dst)
            {
                match spilled.get(dst) {
                    None => out.extend(
                        aimed_at(def, assigned.get(dst).copied().unwrap_or(*dst)).into_iter().map(|one| (one, loc)),
                    ),
                    Some(slot) => {
                        let scratch = Reg::Gpr(Gpr(pool::GPR_SCRATCH[0]));
                        out.extend(aimed_at(def, scratch).into_iter().map(|one| (one, loc)));
                        out.push((spill_access(scratch, *slot, false, false), loc));
                    }
                }
                continue;
            }
            // A copy of a register to itself (a coalesced phi) is nothing,
            // spilled or not.
            if matches!(&op, Op::Copy { dst, src } if dst == src) {
                continue;
            }
            // A copy between a spill slot and a register is a load or store.
            if let Op::Copy { dst, src } = &op
                && let Some(done) = spilled_copy(*dst, *src, assigned, spilled, wide.contains(dst))
            {
                out.extend(done.into_iter().map(|one| (one, loc)));
                continue;
            }
            // A copy of 128 bits moves all of them.
            if let Op::Copy { dst: Reg::Fpr(rd), src: Reg::Fpr(rn) } = &op
                && wide.contains(&Reg::Fpr(*rd))
            {
                op = Op::I(Inst::VectorMov { rd: *rd, rn: *rn });
            }
            let (mut loads, mut stores): (Vec<(Reg, u32, bool)>, Vec<(Reg, u32, bool)>) = (Vec::new(), Vec::new());
            let mut again: Vec<(Reg, Reg)> = Vec::new();
            let (mut next_gpr, mut next_fpr) = (0usize, 0usize);
            // One scratch register per spilled register of this operation.
            let mut scratch: HashMap<Reg, Reg> = HashMap::new();
            op.visit(&mut |register, role| {
                let key = register.key();
                let replacement = if made_again.contains_key(&key) {
                    let Reg::Gpr(_) = key else { unreachable!("a constant is a general register") };
                    let scratch_reg = *scratch
                        .entry(key)
                        .or_insert_with(
                            || {
                                next_gpr += 1;
                                Reg::Gpr(Gpr(pool::GPR_SCRATCH[next_gpr - 1]))
                            },
                        );
                    if !again.iter().any(|(r, _)| *r == scratch_reg) {
                        again.push((scratch_reg, key));
                    }
                    scratch_reg
                } else if let Some(slot) = spilled.get(&key) {
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
                    if role.reads() && !loads.iter().any(|(r, ..)| *r == scratch_reg) {
                        loads.push((scratch_reg, *slot, wide.contains(&key)));
                    }
                    if role.writes() && !stores.iter().any(|(r, ..)| *r == scratch_reg) {
                        stores.push((scratch_reg, *slot, wide.contains(&key)));
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
            for (register, slot, quad) in loads {
                out.push((spill_access(register, slot, true, quad), loc));
            }
            for (register, key) in again {
                out.extend(aimed_at(&made_again[&key], register).into_iter().map(|one| (one, loc)));
            }
            let is_noop = matches!(&op, Op::Copy { dst, src } if dst == src)
                || matches!(&op, Op::I(Inst::VectorMov { rd, rn }) if rd == rn);
            let is_return = matches!(op, Op::Return { .. });
            if !is_noop {
                out.push((op, loc));
            }
            debug_assert!(!(is_return && !stores.is_empty()));
            for (register, slot, quad) in stores {
                out.push((spill_access(register, slot, false, quad), loc));
            }
        }
        block.ops = forward_copies(forward_spills(out, &spill_slots)).into_iter().collect();
    }
}

/// Drops a copy between two registers that already hold one value: `mov x19,
/// x0` then `mov x0, x19` with neither written between. A write to either ends
/// the pair; a call ends every pair with a caller-saved register. The
/// allocator's scratch registers are left alone.
fn forward_copies(ops: Vec<(Op, Option<SrcLoc>)>) -> Vec<(Op, Option<SrcLoc>)> {
    let scratch = |register: &Reg| match register {
        Reg::Gpr(gpr) => pool::GPR_SCRATCH.contains(&gpr.0),
        Reg::Fpr(fpr) => pool::FPR_SCRATCH.contains(&fpr.0),
    };
    let mut equal: Vec<(Reg, Reg)> = Vec::new();
    let mut out = Vec::with_capacity(ops.len());
    for (op, loc) in ops {
        if let Op::Copy { dst, src } = &op {
            if equal.iter().any(|&(a, b)| (a, b) == (*dst, *src) || (a, b) == (*src, *dst)) {
                continue;
            }
            equal.retain(|&(a, b)| a != *dst && b != *dst);
            if !scratch(dst) && !scratch(src) {
                equal.push((*dst, *src));
            }
        } else if matches!(op, Op::Call { .. }) {
            equal.retain(|&(a, b)| pool::callee_saved(a) && pool::callee_saved(b));
        } else {
            for register in op.defs_and_uses().0 {
                equal.retain(|&(a, b)| a != register && b != register);
            }
        }
        out.push((op, loc));
    }
    out
}

/// Drops the reload of a spill slot into a register that still holds what was
/// stored to it or loaded from it since: `str x15, [slot]` then `ldr x15,
/// [slot]` with nothing writing `x15` between. Only the allocator's own slots,
/// which nothing else writes; a call forgets everything.
fn forward_spills(
    ops: Vec<(Op, Option<SrcLoc>)>,
    spill_slots: &HashSet<u32>,
) -> Vec<(Op, Option<SrcLoc>)> {
    let spill = |op: &Op| -> Option<(Reg, u32, u8, bool)> {
        let (Op::I(Inst::Load { mem, addr: Addr::Slot { slot, offset: 0 }, .. })
        | Op::I(Inst::Store { mem, addr: Addr::Slot { slot, offset: 0 }, .. })) = op
        else {
            return None;
        };
        let (register, bytes) = match mem {
            Memory::Int { reg, bytes, .. } => (Reg::Gpr(*reg), *bytes),
            Memory::Fp { reg, bytes } => (Reg::Fpr(*reg), *bytes),
        };
        (spill_slots.contains(slot)).then(|| (register, *slot, bytes, matches!(op, Op::I(Inst::Load { .. }))))
    };
    let mut held: HashMap<Reg, (u32, u8)> = HashMap::new();
    let mut out = Vec::with_capacity(ops.len());
    for (op, loc) in ops {
        if matches!(op, Op::Call { .. }) {
            held.clear();
        } else if let Some((register, slot, bytes, is_load)) = spill(&op) {
            if is_load {
                if held.get(&register) == Some(&(slot, bytes)) {
                    continue;
                }
            } else {
                held.retain(|_, (held_slot, _)| *held_slot != slot);
            }
            held.insert(register, (slot, bytes));
        } else {
            for register in op.defs_and_uses().0 {
                held.remove(&register);
            }
        }
        out.push((op, loc));
    }
    out
}

/// Whether making `register` (a candidate) at each use costs less than
/// keeping it: its instructions at each use (a use that is a copy costs the
/// copy's one instruction less), against its instructions at the definition
/// plus `extra` (a callee-saved register's save and restore, or the store and
/// loads of a slot). Uses and definition count by loop depth.
fn cheaper_again(
    candidates: &HashMap<Reg, (Vec<Op>, u32)>,
    register: &Reg,
    weight: &HashMap<Reg, f64>,
    uses: &Uses,
    extra: f64,
) -> bool {
    let Some((_, length)) = candidates.get(register) else { return false };
    let (all, copies) =
        (uses.all.get(register).copied().unwrap_or(0.0), uses.copies.get(register).copied().unwrap_or(0.0));
    let length = f64::from(*length);
    all * length - copies < (weight[register] - all) * length + extra
}

/// The registers that hold one constant: defined once, by an operation that
/// reads no register (or only the register of such a constant, for the one
/// operation that adds a symbol's page offset), and the operations that make
/// it, in order, with their count.
fn rematerializable(function: &MFunc) -> HashMap<Reg, (Vec<Op>, u32)> {
    let mut defined: HashMap<Reg, usize> = HashMap::new();
    for op in function.blocks.iter().flat_map(|block| &block.ops) {
        for register in op.defs_and_uses().0 {
            *defined.entry(register).or_default() += 1;
        }
    }
    let mut made: HashMap<Reg, (Vec<Op>, u32)> = HashMap::new();
    for op in function.blocks.iter().flat_map(|block| &block.ops) {
        let (defs, uses) = op.defs_and_uses();
        // The instructions it takes to make the value.
        let length = match op {
            Op::Imm { bits, value, dst } => Some(crate::expand_imm::mov_imm(*value, *bits, *dst).len() as u32),
            Op::FrameAddress { .. } => Some(1),
            Op::I(Inst::MoveWide { op, .. }) => (*op != Wide::Keep).then_some(1),
            Op::I(Inst::Adr { .. }) => Some(1),
            Op::I(Inst::AddPageOffset { .. }) => Some(1),
            _ => None,
        };
        let defs: Vec<Reg> = defs.iter().copied().collect();
        let (Some(length), [register]) = (length, &defs[..]) else { continue };
        let register = *register;
        if !register.is_virtual() || defined[&register] != 1 {
            continue;
        }
        match uses[..] {
            [] if !matches!(op, Op::I(Inst::AddPageOffset { .. })) => {
                made.insert(register, (vec![op.clone()], length));
            }
            // `add rd, page, :lo12:`: the page's own operations come first.
            [page] if matches!(op, Op::I(Inst::AddPageOffset { .. })) => {
                if let Some((chain, before)) = made.get(&page)
                    && defined[&page] == 1
                {
                    let mut chain = chain.clone();
                    chain.push(op.clone());
                    let before = *before;
                    made.insert(register, (chain, before + length));
                }
            }
            _ => {}
        }
    }
    made
}

/// The operations `chain` that make a constant, every register they define
/// and read replaced by `register`: each reads what the one before wrote.
fn aimed_at(
    chain: &[Op],
    register: Reg,
) -> Vec<Op> {
    let made: HashSet<Reg> = chain.iter().flat_map(|op| op.defs_and_uses().0).collect();
    chain
        .iter()
        .map(|op| {
            let mut aimed = op.clone();
            aimed.visit(&mut |slot, _| {
                let key = slot.key();
                if made.contains(&key) {
                    match (slot, register) {
                        (RegRef::Gpr(slot), Reg::Gpr(new)) => *slot = new,
                        (RegRef::Fpr(slot), Reg::Fpr(new)) => *slot = new,
                        _ => unreachable!("a register keeps its class"),
                    }
                }
            });
            aimed
        })
        .collect()
}

fn spilled_copy(
    dst: Reg,
    src: Reg,
    assigned: &HashMap<Reg, Reg>,
    spilled: &HashMap<Reg, u32>,
    quad: bool,
) -> Option<Vec<Op>> {
    let physical = |register: Reg| assigned.get(&register).copied().unwrap_or(register);
    match (spilled.get(&dst), spilled.get(&src)) {
        (None, None) => None,
        (Some(slot), None) => Some(vec![spill_access(physical(src), *slot, false, quad)]),
        (None, Some(slot)) => Some(vec![spill_access(physical(dst), *slot, true, quad)]),
        (Some(to), Some(from)) => {
            let scratch = match dst {
                Reg::Gpr(_) => Reg::Gpr(Gpr(pool::GPR_SCRATCH[0])),
                Reg::Fpr(_) => Reg::Fpr(Vreg(pool::FPR_SCRATCH[0])),
            };
            Some(vec![spill_access(scratch, *from, true, quad), spill_access(scratch, *to, false, quad)])
        }
    }
}

fn spill_access(
    register: Reg,
    slot: u32,
    load: bool,
    quad: bool,
) -> Op {
    let addr = Addr::Slot { slot, offset: 0 };
    let mem = match register {
        Reg::Gpr(reg) => Memory::Int { width: Width::X, bytes: 8, sign: false, reg },
        Reg::Fpr(reg) => Memory::Fp { bytes: if quad { 16 } else { 8 }, reg },
    };
    Op::I(if load {
        Inst::Load { mem, addr, mmo: MemOperand::default() }
    } else {
        Inst::Store { mem, addr, mmo: MemOperand::default() }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mfunc::MBlock;

    fn quad(register: Vreg) -> Memory {
        Memory::Fp { bytes: 16, reg: register }
    }

    fn function(ops: Vec<Op>) -> MFunc {
        let mut function = MFunc::new("f");
        function.slot(16, 16);
        function.blocks.push(MBlock { label: "entry".into(), ops: ops.into() });
        function
    }

    fn load(register: Vreg) -> Op {
        Op::I(Inst::Load { mem: quad(register), addr: Addr::Slot { slot: 0, offset: 0 }, mmo: MemOperand::default() })
    }

    fn store(register: Vreg) -> Op {
        Op::I(Inst::Store { mem: quad(register), addr: Addr::Slot { slot: 0, offset: 0 }, mmo: MemOperand::default() })
    }

    fn call() -> Op {
        Op::Call { callee: Callee::Symbol("g".into()), uses: vec![], defs: vec![], tail: false }
    }

    fn eight_byte_fp_accesses(function: &MFunc) -> usize {
        function
            .blocks
            .iter()
            .flat_map(|block| &block.ops)
            .filter(|op| {
                matches!(
                    op,
                    Op::I(
                        Inst::Load { mem: Memory::Fp { bytes: 8, .. }, .. }
                            | Inst::Store { mem: Memory::Fp { bytes: 8, .. }, .. }
                    )
                )
            })
            .count()
    }

    fn add(
        rd: u32,
        rn: u32,
    ) -> Op {
        Op::I(Inst::AddSubImm {
            op: AddSub { subtract: false, set_flags: false },
            width: Width::X,
            rd: Gpr(rd),
            rn: Gpr(rn),
            imm12: 1,
            shift12: false,
        })
    }

    fn spill_loads(function: &MFunc) -> usize {
        function.blocks[0]
            .ops
            .iter()
            .filter(|op| matches!(op, Op::I(Inst::Load { addr: Addr::Slot { slot: 1, .. }, .. })))
            .count()
    }

    /// A spilled value used by the next operation was stored from `x15` and
    /// reloaded into `x15` at once (`fin_number`, `str x15, [sp, #40]; ldr
    /// x15, [sp, #40]`, twice): the register still held it.
    #[test]
    fn a_spilled_value_used_at_once_is_not_reloaded() {
        let value = Reg::Gpr(Gpr(100));
        let spilled = HashMap::from([(value, 1)]);
        let mut function = function(vec![add(100, 0), add(1, 100)]);
        rewrite(&mut function, &HashMap::new(), &spilled, &HashMap::new(), &HashSet::new(), &HashSet::new());
        assert_eq!(spill_loads(&function), 0, "{:?}", function.blocks[0].ops);
    }

    /// The register is gone after a call, or once something else is written to
    /// it: the reload stays.
    #[test]
    fn a_spilled_value_is_reloaded_after_a_call() {
        let value = Reg::Gpr(Gpr(100));
        let spilled = HashMap::from([(value, 1)]);
        let mut function = function(vec![add(100, 0), call(), add(1, 100)]);
        rewrite(&mut function, &HashMap::new(), &spilled, &HashMap::new(), &HashSet::new(), &HashSet::new());
        assert_eq!(spill_loads(&function), 1, "{:?}", function.blocks[0].ops);
    }

    /// A coalesced phi copy of a spilled double to itself was `ldr d29, [slot]`
    /// then `str d29, [slot]`: two instructions for nothing (`fnbody`, three
    /// times).
    #[test]
    fn a_spilled_register_copied_to_itself_costs_nothing() {
        let value = Reg::Fpr(Vreg(100));
        let spilled = HashMap::from([(value, 1)]);
        let mut function = function(vec![Op::Copy { dst: value, src: value }]);
        rewrite(&mut function, &HashMap::new(), &spilled, &HashMap::new(), &HashSet::new(), &HashSet::new());
        assert!(function.blocks[0].ops.is_empty(), "{:?}", function.blocks[0].ops);
    }

    /// A 128-bit value live across a call took `d8`, whose upper half the
    /// callee may clobber, or was spilled as 8 bytes: its top 64 bits were
    /// lost.
    #[test]
    fn a_quad_across_a_call_is_spilled_whole() {
        let mut function = function(vec![load(Vreg(100)), call(), store(Vreg(100))]);
        function.next_register = 101;
        allocate(&mut function);
        let ops = &function.blocks[0].ops;
        assert_eq!(eight_byte_fp_accesses(&function), 0, "{ops:?}");
        let spills = ops.iter().filter(|op| matches!(
            op,
            Op::I(Inst::Store { addr: Addr::Slot { slot, .. }, mem: Memory::Fp { bytes: 16, .. }, .. }) if *slot != 0
        )).count();
        assert_eq!(spills, 1, "{ops:?}");
    }

    /// A `VectorFill` made a 128-bit value that no `q` access named before its
    /// store, so it was spilled as 8 bytes across a call: the top 64 bits
    /// were lost.
    #[test]
    fn a_vector_fill_across_a_call_is_spilled_whole() {
        let fill = Op::I(Inst::VectorFill { rd: Vreg(100), fill: Fill::Zero });
        let mut function = function(vec![fill, call(), store(Vreg(100))]);
        function.next_register = 101;
        allocate(&mut function);
        let ops = &function.blocks[0].ops;
        assert_eq!(eight_byte_fp_accesses(&function), 0, "{ops:?}");
    }

    /// A copy of a 128-bit value that is not coalesced was `fmov d`, which
    /// zeroes the upper half.
    #[test]
    fn a_copy_of_a_quad_moves_all_of_it() {
        let (a, b) = (Vreg(100), Vreg(101));
        let mut function =
            function(vec![load(a), Op::Copy { dst: Reg::Fpr(b), src: Reg::Fpr(a) }, load(a), store(a), store(b)]);
        function.next_register = 102;
        allocate(&mut function);
        let ops = &function.blocks[0].ops;
        assert!(ops.iter().any(|op| matches!(op, Op::I(Inst::VectorMov { .. }))), "{ops:?}");
    }
}

#[allow(dead_code)]
fn unused(_: Callee) {}

#[allow(dead_code)]
fn unused_role(_: Role) {}
