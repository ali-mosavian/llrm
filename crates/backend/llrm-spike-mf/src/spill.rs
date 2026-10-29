//! Spilling around [`GraphColoring`]: a register nothing fits is kept in a
//! frame slot, stored after each definition and loaded before each use into a
//! short-lived register of its own.

use std::collections::HashSet;

use crate::mfunc::{Inst, Kind, MFunction, Operand, Reg};
use crate::regalloc::{Assignment, GraphColoring, Target, Uncolourable, successors};

/// How much more a use inside a loop costs per level of nesting.
const LOOP_WEIGHT: u64 = 10;

/// The loop depth of each block: the blocks from a branch's target back to the
/// branch, for every branch that goes backwards.
fn loop_depths<C>(function: &MFunction<C>) -> Vec<u32> {
    let mut depth = vec![0u32; function.blocks.len()];
    for (at, next) in successors(function).iter().enumerate() {
        for head in next.iter().filter(|head| **head <= at) {
            depth[*head..=at].iter_mut().for_each(|d| *d += 1);
        }
    }
    depth
}

/// What spilling each register would cost: its definitions and uses, each
/// weighted by the loop it is in.
fn costs<C>(function: &MFunction<C>) -> Vec<u64> {
    let mut cost = vec![0u64; function.classes.len()];
    for (block, depth) in function.blocks.iter().zip(loop_depths(function)) {
        let weight = LOOP_WEIGHT.pow(depth.min(4));
        for operand in block.insts.iter().flat_map(|inst| &inst.operands) {
            if let Operand::Def(Reg::Virt(v)) | Operand::Use(Reg::Virt(v)) = operand {
                cost[*v as usize] += weight;
            }
        }
    }
    cost
}

fn mentions(
    inst: &Inst,
    v: u32,
) -> (bool, bool) {
    let has = |wanted: fn(&Operand) -> Option<Reg>| inst.operands.iter().any(|o| wanted(o) == Some(Reg::Virt(v)));
    (
        has(|o| if let Operand::Use(r) = o { Some(*r) } else { None }),
        has(|o| if let Operand::Def(r) = o { Some(*r) } else { None }),
    )
}

/// Keep `v` in `slot`: each instruction that reads it loads it first, each that
/// writes it stores afterwards, through a new register of `v`'s class. A copy
/// to or from `v` becomes the load or the store itself. Returns the new
/// registers.
fn spill<T: Target>(
    function: &mut MFunction<T::Class>,
    v: u32,
    slot: i32,
) -> Result<Vec<u32>, String> {
    let class = function.classes[v as usize];
    let refuse = || format!("the target cannot spill v{v} ({class:?})");
    let mut made = Vec::new();
    for at in 0..function.blocks.len() {
        let mut out = Vec::with_capacity(function.blocks[at].insts.len());
        for mut inst in std::mem::take(&mut function.blocks[at].insts) {
            let (reads, writes) = mentions(&inst, v);
            if !reads && !writes {
                out.push(inst);
                continue;
            }
            if inst.kind == Kind::Copy
                && let [Operand::Def(to), Operand::Use(from)] = inst.operands[..]
                && to != from
            {
                if from == Reg::Virt(v) {
                    out.push(T::spill_load(class, to, slot).ok_or_else(refuse)?);
                } else {
                    out.push(T::spill_store(class, from, slot).ok_or_else(refuse)?);
                }
                continue;
            }
            let temporary = Reg::Virt(function.classes.len() as u32);
            function.classes.push(class);
            made.push(temporary_index(temporary));
            for operand in &mut inst.operands {
                if let Operand::Def(r) | Operand::Use(r) = operand
                    && *r == Reg::Virt(v)
                {
                    *r = temporary;
                }
            }
            if reads {
                let load = T::spill_load(class, temporary, slot).ok_or_else(refuse)?;
                // a load may set the condition codes: it goes before the
                // compare a flag reader depends on, not between them
                let at = if inst.reads_flags { flag_sequence_start(&out) } else { out.len() };
                out.insert(at, load);
            }
            out.push(inst);
            if writes {
                out.push(T::spill_store(class, temporary, slot).ok_or_else(refuse)?);
            }
        }
        function.blocks[at].insts = out;
    }
    Ok(made)
}

fn temporary_index(register: Reg) -> u32 {
    match register {
        Reg::Virt(v) => v,
        Reg::Phys(_) => unreachable!("a temporary is virtual"),
    }
}

/// Where the sequence that produced the flags a reader is about to consume
/// begins: the run of flag setters and copies into physical registers (the
/// accumulator loads of a multi-step compare) ending at `insts`' end.
fn flag_sequence_start(insts: &[Inst]) -> usize {
    let copies_to_physical =
        |inst: &Inst| inst.kind == Kind::Copy && matches!(inst.operands.first(), Some(Operand::Def(Reg::Phys(_))));
    let mut start = insts.len();
    while start > 0 && !insts[start - 1].terminator && (insts[start - 1].sets_flags || copies_to_physical(&insts[start - 1])) {
        start -= 1;
    }
    start
}

/// Whether the condition codes are read at or after `at` before an instruction
/// sets them again: an instruction inserted there that sets them would change
/// what that reader sees.
fn flags_live_at(
    insts: &[Inst],
    at: usize,
) -> bool {
    for inst in &insts[at..] {
        if inst.reads_flags {
            return true;
        }
        if inst.sets_flags {
            return false;
        }
    }
    false
}

/// `isTriviallyReMaterializable`: the block and index of the one instruction
/// that defines `v` from nothing but constants and incoming-argument slots
/// (immutable), with no side effect, so it can run again wherever `v` is read.
fn remat_source<C>(
    function: &MFunction<C>,
    v: u32,
) -> Option<(usize, usize)> {
    let mut defs = function.blocks.iter().enumerate().flat_map(|(at, block)| {
        block.insts.iter().enumerate().filter(move |(_, inst)| mentions(inst, v).1).map(move |(i, _)| (at, i))
    });
    let (block, index) = defs.next()?;
    if defs.next().is_some() || function.blocks.iter().any(|b| b.phis.iter().any(|phi| phi.result == Reg::Virt(v))) {
        return None;
    }
    let inst = &function.blocks[block].insts[index];
    let invariant = |inst: &Inst| {
        !inst.may_load
            || inst.operands.iter().any(|o| matches!(o, Operand::Frame(slot) if function.fixed.contains_key(slot)))
    };
    let defines_only_v = inst.operands.iter().filter(|o| matches!(o, Operand::Def(_))).count() == 1;
    let reads_nothing = !inst.operands.iter().any(|o| matches!(o, Operand::Use(_)));
    (inst.kind != Kind::Copy
        && !inst.call
        && !inst.terminator
        && !inst.may_store
        && !inst.side_effects
        && !inst.early_clobber
        && inst.ties.is_empty()
        && defines_only_v
        && reads_nothing
        && invariant(inst))
    .then_some((block, index))
}

/// Run `v`'s defining instruction again at each read instead of keeping it in
/// a register or a slot (`InlineSpiller::reMaterializeFor`); the definition
/// goes. `None` when `v` cannot be, or when a read sits where the condition
/// codes are live and the instruction sets them. Returns the new registers.
fn rematerialize<C: Copy>(
    function: &mut MFunction<C>,
    v: u32,
) -> Option<Vec<u32>> {
    let (def_block, def_index) = remat_source(function, v)?;
    let source = function.blocks[def_block].insts[def_index].clone();
    for (at, block) in function.blocks.iter().enumerate() {
        for (i, inst) in block.insts.iter().enumerate() {
            let reads = mentions(inst, v).0;
            if reads && source.sets_flags && flags_live_at(&block.insts, i) && !(at == def_block && i == def_index) {
                return None;
            }
        }
    }
    let class = function.classes[v as usize];
    let again = |destination: Reg| {
        let mut copy = source.clone();
        for operand in &mut copy.operands {
            if let Operand::Def(r) = operand {
                *r = destination;
            }
        }
        copy
    };
    let mut made = Vec::new();
    for at in 0..function.blocks.len() {
        let mut out = Vec::with_capacity(function.blocks[at].insts.len());
        for mut inst in std::mem::take(&mut function.blocks[at].insts) {
            let (reads, writes) = mentions(&inst, v);
            if writes {
                continue;
            }
            if !reads {
                out.push(inst);
                continue;
            }
            if inst.kind == Kind::Copy
                && let [Operand::Def(to), Operand::Use(from)] = inst.operands[..]
                && from == Reg::Virt(v)
                && to != from
            {
                out.push(again(to));
                continue;
            }
            let temporary = Reg::Virt(function.classes.len() as u32);
            function.classes.push(class);
            made.push(temporary_index(temporary));
            for operand in &mut inst.operands {
                if let Operand::Use(r) = operand
                    && *r == Reg::Virt(v)
                {
                    *r = temporary;
                }
            }
            out.push(again(temporary));
            out.push(inst);
        }
        function.blocks[at].insts = out;
    }
    Some(made)
}

/// Whether every definition and use of `v` is in one run of instructions of
/// one block: spilling it leaves the same run, so frees nothing.
fn already_minimal<C>(
    function: &MFunction<C>,
    v: u32,
) -> bool {
    let mut places = function
        .blocks
        .iter()
        .enumerate()
        .flat_map(
            |(at, block)| block
                .insts
                .iter()
                .enumerate()
                .filter(move |(_, inst)| mentions(inst, v) != (false, false))
                .map(move |(i, _)| (at, i)),
        );
    let Some((block, first)) = places.next() else { return true };
    let mut last = first;
    for (at, i) in places {
        if at != block || i != last + 1 {
            return false;
        }
        last = i;
    }
    true
}

/// The register to spill for `failed`: the cheapest that spilling can free
/// (not a spill temporary, not already as short as a range gets) of those
/// holding a register `failed` could have used and `failed` itself; failing
/// that, of the crowd.
fn victim<T: Target>(
    function: &MFunction<T::Class>,
    failed: &Uncolourable,
    temporaries: &HashSet<u32>,
) -> Option<u32> {
    let cost = costs(function);
    let cheapest = |from: &[u32]| {
        from.iter()
            .copied()
            .filter(|u| !temporaries.contains(u) && !already_minimal(function, *u))
            .min_by_key(|u| (cost[*u as usize], std::cmp::Reverse(*u)))
    };
    let near: Vec<u32> = failed.blockers.iter().copied().chain([failed.vreg]).collect();
    cheapest(&near).or_else(|| cheapest(&failed.crowd))
}

/// Allocate `function`, spilling until it fits; slot numbers start at
/// `first_slot`. Returns the assignment and each slot with its register's
/// class. The function must be out of phis.
pub fn allocate_spilling<T: Target>(
    function: &mut MFunction<T::Class>,
    first_slot: i32,
) -> Result<(Assignment, Vec<(i32, T::Class)>), String> {
    let mut slots = Vec::new();
    let mut temporaries = HashSet::new();
    loop {
        let failed = match GraphColoring::<T>::default().colour(function) {
            Ok(assignment) => return Ok((assignment, slots)),
            Err(failed) => failed,
        };
        let v = victim::<T>(function, &failed, &temporaries).ok_or_else(|| {
            format!(
                "no register for v{} ({:?}) and nothing to spill",
                failed.vreg, function.classes[failed.vreg as usize]
            )
        })?;
        if let Some(made) = rematerialize(function, v) {
            temporaries.extend(made);
            continue;
        }
        let slot = first_slot + slots.len() as i32;
        slots.push((slot, function.classes[v as usize]));
        temporaries.extend(spill::<T>(function, v, slot)?);
    }
}
