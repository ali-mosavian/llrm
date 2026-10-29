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
                out.push(T::spill_load(class, temporary, slot).ok_or_else(refuse)?);
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

/// The register to spill for `failed`: of the registers crowding it that share
/// a candidate with it, the cheapest to spill that is not itself a spill
/// temporary.
fn victim<T: Target>(
    function: &MFunction<T::Class>,
    failed: &Uncolourable,
    temporaries: &HashSet<u32>,
) -> Option<u32> {
    let wanted = T::candidates(function.classes[failed.vreg as usize]);
    let cost = costs(function);
    failed
        .crowd
        .iter()
        .copied()
        .filter(|u| !temporaries.contains(u))
        .filter(|u| T::candidates(function.classes[*u as usize]).iter().any(|root| wanted.contains(root)))
        .min_by_key(|u| (cost[*u as usize], std::cmp::Reverse(*u)))
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
        let slot = first_slot + slots.len() as i32;
        slots.push((slot, function.classes[v as usize]));
        temporaries.extend(spill::<T>(function, v, slot)?);
    }
}
