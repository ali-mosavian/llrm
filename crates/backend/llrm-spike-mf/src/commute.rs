//! `TwoAddressInstructionPass`'s choice of which source of a commutable
//! instruction the result is tied to (`tryInstructionCommute`,
//! `isProfitableToCommute`), made before the tie copies are inserted.
//!
//! Tying the source that dies at the instruction, or the one the result is
//! copied back into, lets the allocator give the result that source's register
//! and drop the tie copy.
//!
//! DEVIATE: kills come from block liveness over virtual registers, not
//! `LiveIntervals`; `SrcRegMap`/`DstRegMap` are rebuilt per instruction from the
//! block (a copy from a physical register not clobbered since, and the single
//! copy of the result into one) rather than kept as the pin scans; chains
//! follow copies only, not tied uses; `hasCommutePreference` is not asked
//! (M68k has none). Reason: the spike has no analysis manager. Exit: the
//! neutral-LIR task.
//!
//! [`commute_accumulator`] makes the same choice for a commutable instruction
//! whose tie is on a physical register (`P = copy first; op P, P, second`).
//!
//! # See also
//! `llvm/lib/CodeGen/TwoAddressInstructionPass.cpp`, pin d1106deb71cc:
//! `isRevCopyChain` (L320), `noUseAfterLastDef` (L351), `isKilled` (L454),
//! `isProfitableToCommute` (L648), `tryInstructionCommute` (L1285).

use llrm_lir::registers::RegId;
use llrm_support::hash::HashMap;

use crate::mfunc::{Inst, Kind, MFunction, Operand, Reg};
use crate::regalloc::{Target, successors};

/// `MaxDataFlowEdge`.
const MAX_DATA_FLOW_EDGE: usize = 10;

pub(crate) struct Facts {
    /// Registers live out of each block.
    live_out: Vec<Vec<u64>>,
    /// Every definition of a register: `(block, instruction)`.
    defs: HashMap<u32, Vec<(usize, usize)>>,
    /// How many operands read each virtual register in the function.
    use_counts: Vec<u32>,
    /// Words the liveness solver visited.
    steps: u64,
}

fn virt(reg: Reg) -> Option<u32> {
    if let Reg::Virt(v) = reg { Some(v) } else { None }
}

fn uses(inst: &Inst) -> impl Iterator<Item = u32> + '_ {
    inst.operands.iter().filter_map(|o| if let Operand::Use(r) = o { virt(*r) } else { None })
}

fn defines(inst: &Inst) -> impl Iterator<Item = u32> + '_ {
    inst.operands.iter().filter_map(|o| if let Operand::Def(r) = o { virt(*r) } else { None })
}

/// Words the liveness solver visits on `function`: the cost the tests bound.
pub fn liveness_steps<C>(function: &MFunction<C>) -> u64 {
    Facts::of(function).steps
}

impl Facts {
    pub(crate) fn of<C>(function: &MFunction<C>) -> Facts {
        let count = function.classes.len();
        let blocks = function.blocks.len();
        let words = count.div_ceil(64);
        let succs = successors(function);
        let mut gen_ = vec![vec![0u64; words]; blocks];
        let mut kill = vec![vec![0u64; words]; blocks];
        let mut defs: HashMap<u32, Vec<(usize, usize)>> = HashMap::default();
        let mut use_counts = vec![0u32; count];
        let bit = |set: &[u64], r: u32| set[r as usize / 64] >> (r % 64) & 1 != 0;
        for (b, block) in function.blocks.iter().enumerate() {
            for (i, inst) in block.insts.iter().enumerate() {
                for u in uses(inst) {
                    use_counts[u as usize] += 1;
                    if !bit(&kill[b], u) {
                        gen_[b][u as usize / 64] |= 1 << (u % 64);
                    }
                }
                for d in defines(inst) {
                    kill[b][d as usize / 64] |= 1 << (d % 64);
                    defs.entry(d).or_default().push((b, i));
                }
            }
        }
        let mut live_in = vec![vec![0u64; words]; blocks];
        let mut live_out = vec![vec![0u64; words]; blocks];
        let mut changed = true;
        let mut steps = 0u64;
        while changed {
            changed = false;
            for b in (0..blocks).rev() {
                for w in 0..words {
                    steps += 1;
                    let out = succs[b].iter().fold(0, |acc, &s| acc | live_in[s][w]);
                    let input = gen_[b][w] | (out & !kill[b][w]);
                    if out != live_out[b][w] || input != live_in[b][w] {
                        live_out[b][w] = out;
                        live_in[b][w] = input;
                        changed = true;
                    }
                }
            }
        }
        Facts { live_out, defs, use_counts, steps }
    }

    /// `isPlainlyKilled`: nothing reads `reg` after instruction `at` of block
    /// `b`, and it does not leave the block.
    pub(crate) fn plainly_killed<C>(
        &self,
        function: &MFunction<C>,
        b: usize,
        at: usize,
        reg: u32,
    ) -> bool {
        let rest = &function.blocks[b].insts[at + 1..];
        let read_later = rest.iter().any(|inst| uses(inst).any(|u| u == reg));
        let redefined = rest.iter().any(|inst| defines(inst).any(|d| d == reg));
        !read_later && (redefined || self.live_out[b][reg as usize / 64] >> (reg % 64) & 1 == 0)
    }

    /// `isKilled`: looks through copies, since a copy of a register that lives
    /// on is not coalesced into a dying one.
    fn killed<C>(
        &self,
        function: &MFunction<C>,
        b: usize,
        at: usize,
        reg: u32,
    ) -> bool {
        let (mut b, mut at, mut reg) = (b, at, reg);
        loop {
            if !self.plainly_killed(function, b, at, reg) {
                return false;
            }
            let defs = self.defs.get(&reg).map_or(&[][..], Vec::as_slice);
            let [(def_block, def_at)] = defs else { return true };
            let inst = &function.blocks[*def_block].insts[*def_at];
            let (Kind::Copy, [Operand::Def(_), Operand::Use(Reg::Virt(source))]) = (inst.kind, &inst.operands[..])
            else {
                return true;
            };
            (b, at, reg) = (*def_block, *def_at, *source);
        }
    }

    /// `getMappedReg(reg, SrcRegMap)`: the physical register `reg` was copied
    /// from, through copies, when nothing wrote it (or a call) between that
    /// copy and instruction `at` of block `b`.
    fn copied_from_phys<T: Target>(
        function: &MFunction<T::Class>,
        b: usize,
        at: usize,
        reg: u32,
    ) -> Option<RegId> {
        let insts = &function.blocks[b].insts;
        let mut reg = reg;
        let mut before = at;
        for _ in 0..MAX_DATA_FLOW_EDGE {
            let only = Facts::single_def_index(insts, reg).filter(|&i| i < before)?;
            let (Kind::Copy, [Operand::Def(_), Operand::Use(source)]) = (insts[only].kind, &insts[only].operands[..])
            else {
                return None;
            };
            match *source {
                Reg::Virt(next) => {
                    reg = next;
                    before = only;
                }
                Reg::Phys(phys) => {
                    let root = T::root(phys);
                    let clobbered = insts[only + 1..at].iter().any(|inst| {
                        (inst.call && T::caller_saved().iter().any(|&one| T::overlaps(one, root)))
                            || inst.operands.iter().any(
                                |o| matches!(o, Operand::Def(Reg::Phys(q)) if T::overlaps(T::root(*q), root)),
                            )
                    });
                    return (!clobbered).then_some(phys);
                }
            }
        }
        None
    }

    /// `getMappedReg(reg, DstRegMap)`: the physical register `reg`, read once
    /// in the function, is copied into after instruction `at` of block `b`,
    /// through copies.
    fn copied_to_phys<T: Target>(
        function: &MFunction<T::Class>,
        use_counts: &[u32],
        b: usize,
        at: usize,
        reg: u32,
    ) -> Option<RegId> {
        let insts = &function.blocks[b].insts;
        let mut reg = reg;
        for _ in 0..MAX_DATA_FLOW_EDGE {
            if use_counts[reg as usize] != 1 {
                return None;
            }
            let reader = insts[at + 1..].iter().find(|inst| uses(inst).any(|u| u == reg))?;
            let (Kind::Copy, [Operand::Def(target), Operand::Use(_)]) = (reader.kind, &reader.operands[..]) else {
                return None;
            };
            match *target {
                Reg::Phys(phys) => return Some(phys),
                Reg::Virt(next) => reg = next,
            }
        }
        None
    }

    /// The index of the one definition of `reg` in `insts`.
    fn single_def_index(
        insts: &[Inst],
        reg: u32,
    ) -> Option<usize> {
        let mut found = None;
        for (i, inst) in insts.iter().enumerate() {
            if defines(inst).any(|d| d == reg) {
                if found.is_some() {
                    return None;
                }
                found = Some(i);
            }
        }
        found
    }

    /// `getSingleDef`: the one definition of `reg` in block `b`.
    fn single_def<'a, C>(
        function: &'a MFunction<C>,
        b: usize,
        reg: u32,
    ) -> Option<&'a Inst> {
        let mut found = None;
        for inst in &function.blocks[b].insts {
            if defines(inst).any(|d| d == reg) {
                if found.is_some() {
                    return None;
                }
                found = Some(inst);
            }
        }
        found
    }

    /// `isRevCopyChain`: whether `to` is reached from `from` by following
    /// single definitions through copies and tied uses.
    fn rev_copy_chain<C>(
        function: &MFunction<C>,
        b: usize,
        from: u32,
        to: u32,
    ) -> bool {
        let mut reg = from;
        for _ in 0..MAX_DATA_FLOW_EDGE {
            let Some(def) = Facts::single_def(function, b, reg) else { return false };
            if let (Kind::Copy, [Operand::Def(_), Operand::Use(Reg::Virt(source))]) = (def.kind, &def.operands[..]) {
                reg = *source;
            } else if let Some(tied) = def.ties.iter().find_map(|&(d, u)| match (&def.operands[d], &def.operands[u]) {
                (Operand::Def(Reg::Virt(dst)), Operand::Use(Reg::Virt(src))) if *dst == reg => Some(*src),
                _ => None,
            }) {
                if tied == reg {
                    return false;
                }
                reg = tied;
            } else {
                return false;
            }
            if reg == to {
                return true;
            }
        }
        false
    }

    /// `noUseAfterLastDef`: nothing reads `reg` between its last definition in
    /// the block (before `dist`) and `dist`. Also the position of that
    /// definition (1-based, 0 for none).
    fn no_use_after_last_def<C>(
        function: &MFunction<C>,
        b: usize,
        reg: u32,
        dist: usize,
    ) -> (bool, usize) {
        let (mut last_def, mut last_use) = (0, dist);
        for (i, inst) in function.blocks[b].insts[..dist - 1].iter().enumerate() {
            let at = i + 1;
            if uses(inst).any(|u| u == reg) && at < last_use {
                last_use = at;
            }
            if defines(inst).any(|d| d == reg) && at > last_def {
                last_def = at;
            }
        }
        (!(last_use > last_def && last_use < dist), last_def)
    }
}

/// Swaps the tied source of every commutable instruction with its other
/// register source where the pin's rules say the result is better tied to
/// that one.
pub fn commute_tied<T: Target>(function: &mut MFunction<T::Class>) {
    let facts = Facts::of(function);
    for b in 0..function.blocks.len() {
        for at in 0..function.blocks[b].insts.len() {
            let inst = &function.blocks[b].insts[at];
            if !inst.commutable || inst.ties.len() != 1 {
                continue;
            }
            let (def, used) = inst.ties[0];
            let (Operand::Def(Reg::Virt(a)), Operand::Use(Reg::Virt(base))) =
                (&inst.operands[def], &inst.operands[used])
            else {
                continue;
            };
            let (a, base) = (*a, *base);
            let Some(other) = inst
                .operands
                .iter()
                .enumerate()
                .position(|(n, o)| n != used && matches!(o, Operand::Use(Reg::Virt(r)) if *r != base))
            else {
                continue;
            };
            let Operand::Use(Reg::Virt(c)) = inst.operands[other] else { continue };
            // the result's class must hold the register that becomes the tied
            // one
            if T::meet(function.classes[c as usize], function.classes[a as usize]).is_none() {
                continue;
            }
            let dist = at + 1;
            let base_killed = facts.killed(function, b, at, base);
            let other_killed = facts.killed(function, b, at, c);
            let commute = (!base_killed && other_killed) || profitable::<T>(&facts, function, b, at, dist, a, base, c);
            if commute {
                function.blocks[b].insts[at].operands.swap(used, other);
                relax_demoted::<T>(function, &facts, b, at, base, other);
            }
        }
    }
}

/// `base` was the tied source, so its class was narrowed to the result's; it
/// is now the other source, which may take more. Where it is only a copy of
/// another register, read nowhere else, it takes that register's class met
/// with the operand's, so the allocator may put it where the tied one cannot
/// go (a temporary in an address register, not a data register).
///
/// DEVIATE from the pin, whose class stays narrowed (`constrainRegClass`
/// never widens). Reason: the swap is ours, and the narrowing it leaves
/// costs a data register on a rotation. Exit: the neutral-LIR task, where
/// the commute is the pin's own.
fn relax_demoted<T: Target>(
    function: &mut MFunction<T::Class>,
    facts: &Facts,
    b: usize,
    at: usize,
    base: u32,
    slot: usize,
) {
    let Kind::Op(name) = function.blocks[b].insts[at].kind else { return };
    let Some(wanted) = T::operand_class(name, slot) else { return };
    let Some([(def_block, def_at)]) = facts.defs.get(&base).map(Vec::as_slice) else { return };
    let (Kind::Copy, [Operand::Def(_), Operand::Use(Reg::Virt(source))]) =
        (function.blocks[*def_block].insts[*def_at].kind, &function.blocks[*def_block].insts[*def_at].operands[..])
    else {
        return;
    };
    let reads = function.blocks.iter().flat_map(|block| &block.insts).flat_map(uses).filter(|u| *u == base).count();
    if reads != 1 {
        return;
    }
    if let Some(class) = T::meet(function.classes[*source as usize], wanted) {
        function.classes[base as usize] = class;
    }
}

/// `isProfitableToCommute`.
#[allow(clippy::too_many_arguments)]
fn profitable<T: Target>(
    facts: &Facts,
    function: &MFunction<T::Class>,
    b: usize,
    at: usize,
    dist: usize,
    a: u32,
    base: u32,
    c: u32,
) -> bool {
    if !facts.plainly_killed(function, b, at, c) {
        return false;
    }
    // `r0 = ADD (MOV r1), (MOV r0)`: tie the source that arrives in the
    // register the result leaves in, to drop an otherwise certain copy
    if let Some(to_a) = Facts::copied_to_phys::<T>(function, &facts.use_counts, b, at, a) {
        let from_b = Facts::copied_from_phys::<T>(function, b, at, base);
        let from_c = Facts::copied_from_phys::<T>(function, b, at, c);
        let compatible = |from: Option<RegId>| from.is_some_and(|from| T::overlaps(T::root(from), T::root(to_a)));
        let (comp_b, comp_c) = (compatible(from_b), compatible(from_c));
        if (from_b.is_none() && comp_c) || (from_b.is_some() && !comp_b && (from_c.is_none() || comp_c)) {
            return true;
        }
        if (from_c.is_none() && comp_b) || (from_c.is_some() && !comp_c && (from_b.is_none() || comp_b)) {
            return false;
        }
    }
    let (clear_c, last_def_c) = Facts::no_use_after_last_def(function, b, c, dist);
    if !clear_c {
        return false;
    }
    let (clear_b, last_def_b) = Facts::no_use_after_last_def(function, b, base, dist);
    if !clear_b {
        return true;
    }
    if Facts::rev_copy_chain(function, b, c, a) {
        return true;
    }
    if Facts::rev_copy_chain(function, b, base, a) {
        return false;
    }
    last_def_b != 0 && last_def_c != 0 && last_def_c > last_def_b
}

/// How many single-predecessor blocks [`commute_accumulator`] walks back.
const MAX_BLOCKS_BACK: usize = 4;

/// Swaps the sources of every commutable instruction that reads and writes a
/// fixed register (`P = copy first; op P, P, second`) where `second` arrives in
/// `P` and `first` does not: the `SrcRegMap` branch of `isProfitableToCommute`
/// (TwoAddressInstructionPass.cpp#L648), with the tied register named by the
/// instruction itself, so `second` is read from `P` and `first` from where
/// the instruction's second operand lives (`Target::operand_class`).
///
/// Both sources must die at the instruction and be read nowhere else, so
/// their classes may follow the swap; `second` must be copied from `P`, not
/// written since, along single-predecessor blocks (the argument copies of
/// an accumulator machine sit in the entry block); `first` must be copied
/// from a register or from one that dies there (`stored_without_a_copy`),
/// or the memory place costs a move through `P`.
///
/// DEVIATE 6502-COMMUTE: the tie is on a physical register, where the pin
/// ties virtual registers of the accumulator's class and `commute_tied`
/// swaps those. Reason: the instruction names the register itself. Exit:
/// 2026-11-30, when such instructions carry the register as a tied virtual
/// operand. Owner: 6502.
pub fn commute_accumulator<T: Target>(function: &mut MFunction<T::Class>) {
    let facts = Facts::of(function);
    let mut preds = vec![Vec::new(); function.blocks.len()];
    for (from, succs) in successors(function).iter().enumerate() {
        for &to in succs {
            // a branch to the next block is also its fall through
            if !preds[to].contains(&from) {
                preds[to].push(from);
            }
        }
    }
    for b in 0..function.blocks.len() {
        for at in 0..function.blocks[b].insts.len() {
            let inst = &function.blocks[b].insts[at];
            let Kind::Op(name) = inst.kind else { continue };
            let [Operand::Def(Reg::Phys(written)), Operand::Use(Reg::Phys(read)), Operand::Use(Reg::Virt(second))] =
                inst.operands[..]
            else {
                continue;
            };
            if !inst.commutable || written != read {
                continue;
            }
            let Some(wanted) = T::operand_class(name, 2) else { continue };
            let root = T::root(written);
            let touches = |o: &Operand| match o {
                Operand::Def(Reg::Phys(q)) | Operand::Use(Reg::Phys(q)) => T::overlaps(T::root(*q), root),
                _ => false,
            };
            // `P = first` is the nearest write of `P`; nothing between reads it
            let Some(load) = (0..at).rev().find(|&i| {
                let between = &function.blocks[b].insts[i];
                between.call || between.operands.iter().any(touches)
            }) else {
                continue;
            };
            let moved = &function.blocks[b].insts[load];
            let (Kind::Copy, [Operand::Def(Reg::Phys(into)), Operand::Use(Reg::Virt(first))]) =
                (moved.kind, &moved.operands[..])
            else {
                continue;
            };
            let first = *first;
            if *into != written
                || first == second
                || facts.use_counts[first as usize] != 1
                || facts.use_counts[second as usize] != 1
            {
                continue;
            }
            let from_first = facts.arrives_in::<T>(function, &preds, b, load, first);
            let from_second = facts.arrives_in::<T>(function, &preds, b, load, second);
            let is_p = |from: Option<RegId>| from.is_some_and(|from| T::overlaps(T::root(from), root));
            if !is_p(from_second) || is_p(from_first) || !facts.stored_without_a_copy(function, first) {
                continue;
            }
            let Some(held) = T::meet(function.classes[first as usize], wanted) else { continue };
            function.classes[first as usize] = held;
            function.classes[second as usize] = T::any();
            function.blocks[b].insts[load].operands[1] = Operand::Use(Reg::Virt(second));
            function.blocks[b].insts[at].operands[2] = Operand::Use(Reg::Virt(first));
        }
    }
}

impl Facts {
    /// Whether `reg` can be given the memory operand's place as it is: every
    /// definition is a copy, from a physical register or from a register that
    /// dies there. A value computed into the place, or a lane of a register
    /// that lives on, has to be moved there through the accumulator the other
    /// source now holds.
    fn stored_without_a_copy<C>(
        &self,
        function: &MFunction<C>,
        reg: u32,
    ) -> bool {
        let Some(defs) = self.defs.get(&reg) else { return false };
        defs.iter().all(|&(b, at)| {
            let inst = &function.blocks[b].insts[at];
            match (inst.kind, &inst.operands[..]) {
                (Kind::Copy, [Operand::Def(_), Operand::Use(Reg::Phys(_))]) => true,
                (Kind::Copy, [Operand::Def(_), Operand::Use(Reg::Virt(source))]) => {
                    self.plainly_killed(function, b, at, *source)
                }
                _ => false,
            }
        })
    }

    /// The physical register `reg` is a copy of, through copies, as read at
    /// instruction `at` of block `b`: every copy before `at`, and nothing
    /// writing that register from the first copy to `at` along single
    /// predecessors (`getMappedReg(reg, SrcRegMap)`, past one block).
    fn arrives_in<T: Target>(
        &self,
        function: &MFunction<T::Class>,
        preds: &[Vec<usize>],
        b: usize,
        at: usize,
        reg: u32,
    ) -> Option<RegId> {
        let mut reg = reg;
        for _ in 0..MAX_DATA_FLOW_EDGE {
            let [(def_block, def_at)] = self.defs.get(&reg)?[..] else { return None };
            if def_block == b && def_at >= at {
                return None;
            }
            let inst = &function.blocks[def_block].insts[def_at];
            let (Kind::Copy, [Operand::Def(_), Operand::Use(source)]) = (inst.kind, &inst.operands[..]) else {
                return None;
            };
            match *source {
                Reg::Virt(next) => reg = next,
                Reg::Phys(phys) => {
                    return Self::untouched::<T>(function, preds, (def_block, def_at), (b, at), phys).then_some(phys);
                }
            }
        }
        None
    }

    /// Whether no instruction from `from` to `to` (exclusive) writes `phys`,
    /// `to` being reached from `from` along single predecessors.
    fn untouched<T: Target>(
        function: &MFunction<T::Class>,
        preds: &[Vec<usize>],
        from: (usize, usize),
        to: (usize, usize),
        phys: RegId,
    ) -> bool {
        let root = T::root(phys);
        let writes = |insts: &[Inst]| {
            insts
                .iter()
                .any(
                    |inst| (inst.call && T::caller_saved().iter().any(|&one| T::overlaps(one, root)))
                        || inst.operands.iter().any(|o| {
                            matches!(
                                o,
                                Operand::Def(Reg::Phys(q)) if T::overlaps(T::root(*q), root)
                            )
                        }),
                )
        };
        let ((from_block, from_at), (mut block, at)) = (from, to);
        if block == from_block {
            return from_at < at && !writes(&function.blocks[block].insts[from_at + 1..at]);
        }
        if writes(&function.blocks[block].insts[..at]) {
            return false;
        }
        for _ in 0..MAX_BLOCKS_BACK {
            let [pred] = preds[block][..] else { return false };
            if pred == from_block {
                return !writes(&function.blocks[pred].insts[from_at + 1..]);
            }
            if writes(&function.blocks[pred].insts) {
                return false;
            }
            block = pred;
        }
        false
    }
}
