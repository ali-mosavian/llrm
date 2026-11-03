//! The pre-RA machine scheduler for arm64: each region of a block ordered by
//! the shared bottom-up pass (`MachineScheduler`, row 65), so a loop's new
//! value is made after the last use of the old one it replaces and the
//! allocator gives it that register.
//!
//! # How it works
//! A region is a run of instructions between the ones that stay put: a branch,
//! a call, a barrier, an access with side effects, anything naming a physical
//! register other than the zero register. Within it an edge runs from a value's
//! last def to its readers, from its readers to the next def, between two defs,
//! on the flags, and on memory (a store before every later access, a load
//! before every later store). The shared [`bottom_up`] picks the order with
//! the pressure of the two register files followed in a region of more than
//! half the general registers.
//!
//! Copies get the pin's weak edges (`CopyConstrain`), which keep a copy's local
//! uses beside it so the allocator can coalesce it.
//!
//! It runs after the coalescer, as the pin does (`Target::schedule` in
//! `allocate_greedy`): the function it orders has the joined values renamed and
//! their copies gone, and the coalescer runs again on the result. The phis
//! copy through a temporary per phi (`PHIElimination`) so the new value of a
//! loop-carried register can be made in the old one's.
//!
//! DEVIATE A64-MS1: the pin schedules arm64 bidirectionally on the default
//! cpu's model (`AArch64Subtarget::overrideSchedPolicy`: `OnlyTopDown` and
//! `OnlyBottomUp` off, latency and resources); this is the bottom-up pass the
//! x86 targets run, with no latency or resource. `-misched-prera-direction=
//! bottomup` gives the pin the same movs on `recmany` (10, bidirectional 10).
//! Exit: bidirectional `GenericScheduler`, 2026-11-20.
//!
//! # See also
//! `llrm_codegen::backend::machinesched` (the pass), `lir_view` (the
//! hook), `dag::emit` (the phi copies).

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use llrm_codegen::backend::machinesched::{Graph, Node, Tracking, bottom_up_region, copy_constrain_nodes};
use llrm_codegen::backend::regpressure::{Model, Region};
use llrm_spike_mf::mfunc::{Inst, Kind, MFunction, Operand, Reg};

use crate::lir_view::Class;

/// The pseudo value the status flags are, for edges.
const FLAGS: u32 = u32::MAX;

/// The registers the allocator hands out of each file: the pressure limits.
const GPR_LIMIT: u32 = 25;
const FPR_LIMIT: u32 = 32;

fn virtuals(inst: &Inst) -> (Vec<u32>, Vec<u32>) {
    let (mut defs, mut uses) = (Vec::new(), Vec::new());
    for operand in &inst.operands {
        match operand {
            Operand::Def(Reg::Virt(n)) => defs.push(*n),
            Operand::Use(Reg::Virt(n)) => uses.push(*n),
            _ => {}
        }
    }
    (defs, uses)
}

/// Whether `inst` stays where it is and nothing moves across it.
fn boundary(inst: &Inst) -> bool {
    inst.terminator
        || inst.barrier
        || inst.call
        || inst.side_effects
        || inst.early_clobber
        || inst.operands.iter().any(|operand| {
            matches!(
                operand,
                Operand::Def(Reg::Phys(id)) | Operand::Use(Reg::Phys(id)) if *id != crate::reg::XZR && *id != crate::reg::SP
            )
        })
}

/// `function` as the coalescer leaves it: each value is the one that stands
/// for it (of the class both could take) and a copy of a value to itself is
/// gone, as the pin has deleted it before it schedules.
fn joined(
    function: &MFunction<Class>,
    survivor: &dyn Fn(u32) -> u32,
) -> MFunction<Class> {
    let mut seen = function.clone();
    let known = function.classes.len() as u32;
    // A value joined into a register of the machine stays as it is.
    let stands = |n: u32| match survivor(n + 1) {
        one if one >= 1 && one <= known => one - 1,
        _ => n,
    };
    for n in 0..known {
        let to = stands(n);
        if to != n {
            let (one, other) = (seen.classes[to as usize], seen.classes[n as usize]);
            seen.classes[to as usize] = match (one, other) {
                (Class::Fpr, Class::Quad) | (Class::Quad, Class::Fpr) => Class::Quad,
                _ => one,
            };
        }
    }
    for block in seen.blocks.iter_mut() {
        for inst in block.insts.iter_mut() {
            for operand in inst.operands.iter_mut() {
                match operand {
                    Operand::Def(Reg::Virt(n)) | Operand::Use(Reg::Virt(n)) => *n = stands(*n),
                    _ => {}
                }
            }
        }
        block
            .insts
            .retain(
                |inst| !matches!(
                    (inst.kind, inst.operands.as_slice()),
                    (Kind::Copy, [Operand::Def(dst), Operand::Use(src)]) if dst == src
                ),
            );
    }
    seen
}

/// The (destination, source) of a copy of one virtual register to another.
fn copy_of(inst: &Inst) -> Option<(u32, u32)> {
    match (inst.kind, inst.operands.as_slice()) {
        (Kind::Copy, [Operand::Def(Reg::Virt(dst)), Operand::Use(Reg::Virt(src))]) if dst != src => Some((*dst, *src)),
        _ => None,
    }
}

/// The edges of a region, by position in it (`ScheduleDAGInstrs`).
fn graph(insts: &[Inst]) -> Graph {
    let n = insts.len();
    let mut preds: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut def_at: HashMap<u32, usize> = HashMap::new();
    let mut readers: HashMap<u32, Vec<usize>> = HashMap::new();
    let (mut store, mut loads): (Option<usize>, Vec<usize>) = (None, Vec::new());
    for (later, inst) in insts.iter().enumerate() {
        let (mut defs, mut uses) = virtuals(inst);
        if inst.reads_flags {
            uses.push(FLAGS);
        }
        if inst.sets_flags {
            defs.push(FLAGS);
        }
        let mut edges: Vec<usize> = Vec::new();
        for value in &uses {
            edges.extend(def_at.get(value));
        }
        for value in &defs {
            edges.extend(def_at.get(value));
            edges.extend(readers.get(value).into_iter().flatten());
        }
        if inst.may_load || inst.may_store {
            edges.extend(store);
            if inst.may_store {
                edges.extend(&loads);
            }
        }
        edges.retain(|&earlier| earlier != later);
        edges.sort_unstable();
        edges.dedup();
        preds[later] = edges;
        for value in &uses {
            readers.entry(*value).or_default().push(later);
        }
        for value in &defs {
            def_at.insert(*value, later);
            readers.remove(value);
        }
        if inst.may_store {
            store = Some(later);
            loads.clear();
        } else if inst.may_load {
            loads.push(later);
        }
    }
    Graph::from_preds(preds)
}

/// The blocks a block can go to: its branch targets and, unless it ends in a
/// jump nothing falls out of, the next block.
fn successors<C>(
    function: &MFunction<C>,
    at: usize,
) -> Vec<usize> {
    let block = &function.blocks[at];
    let mut out: Vec<usize> = block
        .insts
        .iter()
        .flat_map(|inst| inst.operands.iter())
        .filter_map(|operand| match operand {
            Operand::Block(to) => function.blocks.iter().position(|one| one.id == *to),
            _ => None,
        })
        .collect();
    if !block.insts.last().is_some_and(|last| last.barrier) && at + 1 < function.blocks.len() {
        out.push(at + 1);
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// The virtual registers live out of each block.
fn live_out(function: &MFunction<Class>) -> Vec<HashSet<u32>> {
    let count = function.blocks.len();
    let mut gen_kill: Vec<(HashSet<u32>, HashSet<u32>)> = Vec::with_capacity(count);
    for block in &function.blocks {
        let (mut read, mut written) = (HashSet::new(), HashSet::new());
        for inst in &block.insts {
            let (defs, uses) = virtuals(inst);
            read.extend(uses.into_iter().filter(|value| !written.contains(value)));
            written.extend(defs);
        }
        gen_kill.push((read, written));
    }
    let succs: Vec<Vec<usize>> = (0..count).map(|at| successors(function, at)).collect();
    let mut live_in: Vec<HashSet<u32>> = gen_kill.iter().map(|(read, _)| read.clone()).collect();
    let mut out: Vec<HashSet<u32>> = vec![HashSet::new(); count];
    let mut changed = true;
    while changed {
        changed = false;
        for at in (0..count).rev() {
            let mut now: HashSet<u32> = HashSet::new();
            for &next in &succs[at] {
                now.extend(live_in[next].iter().copied());
            }
            let mut entering: HashSet<u32> = gen_kill[at].0.clone();
            entering.extend(now.iter().copied().filter(|value| !gen_kill[at].1.contains(value)));
            if now != out[at] || entering != live_in[at] {
                out[at] = now;
                live_in[at] = entering;
                changed = true;
            }
        }
    }
    out
}

/// The pressure model of the function's registers: the general file is set 0,
/// the vector file set 1.
fn model(function: &MFunction<Class>) -> Model {
    let mut model = Model::new(vec![GPR_LIMIT, FPR_LIMIT]);
    let sets: [Rc<[usize]>; 2] = [Rc::from(vec![0]), Rc::from(vec![1])];
    for (value, class) in function.classes.iter().enumerate() {
        model.count(value as u32, 1, sets[usize::from(*class != Class::Gpr)].clone());
    }
    model
}

/// `function` with each region of each block ordered; whether anything moved.
pub fn schedule(
    function: &mut MFunction<Class>,
    survivor: &dyn Fn(u32) -> u32,
) -> bool {
    if function.blocks.iter().all(|block| block.insts.len() < 2) {
        return false;
    }
    let mut seen = joined(function, survivor);
    let live = live_out(&seen);
    let model = model(&seen);
    for (at, block) in seen.blocks.iter_mut().enumerate() {
        let mut regions: Vec<std::ops::Range<usize>> = Vec::new();
        let mut start = None;
        for (index, inst) in block.insts.iter().enumerate() {
            if boundary(inst) {
                if let Some(first) = start.take() {
                    regions.push(first..index);
                }
            } else if start.is_none() {
                start = Some(index);
            }
        }
        if let Some(first) = start {
            regions.push(first..block.insts.len());
        }
        if regions.iter().all(|region| region.len() < 2) {
            continue;
        }
        let mut now: HashSet<u32> = live[at].clone();
        let mut out = block.insts.clone();
        let mut end = block.insts.len();
        let step = |now: &mut HashSet<u32>, inst: &Inst| {
            let (defs, uses) = virtuals(inst);
            for value in defs {
                now.remove(&value);
            }
            now.extend(uses);
        };
        for region in regions.iter().rev() {
            for inst in block.insts[region.end..end].iter().rev() {
                step(&mut now, inst);
            }
            let slice = &block.insts[region.clone()];
            if slice.len() >= 2 {
                let nodes: Vec<(Vec<u32>, Vec<u32>)> = slice
                    .iter()
                    .map(|inst| {
                        let (defs, uses) = virtuals(inst);
                        (uses, defs)
                    })
                    .collect();
                let region_model =
                    Region::from_nodes(nodes.iter().map(|(uses, defs)| (uses.as_slice(), defs.as_slice())));
                let tracking = (slice.len() > GPR_LIMIT as usize / 2).then(|| {
                    let mut live_out: Vec<u32> = now.iter().copied().collect();
                    live_out.sort_unstable();
                    let tied: HashSet<u32> = slice
                        .iter()
                        .flat_map(|inst| {
                            inst.ties
                                .iter()
                                .filter_map(
                                    move |&(def, _)| match inst.operands[def] {
                                        Operand::Def(Reg::Virt(n)) => Some(n),
                                        _ => None,
                                    },
                                )
                        })
                        .collect();
                    let live_thru = live_out.iter().copied().filter(|value| !tied.contains(value)).collect();
                    Tracking { model: &model, live_out, live_thru }
                });
                let mut deps = graph(slice);
                let nodes: Vec<Node> = nodes
                    .iter()
                    .zip(slice)
                    .map(|((uses, defs), inst)| Node { uses, defines: defs, copy: copy_of(inst) })
                    .collect();
                copy_constrain_nodes(&nodes, &mut deps, &|value| now.contains(&value));
                let order = bottom_up_region(
                    slice.len(),
                    &deps,
                    tracking.as_ref().map(|tracking| (tracking, &region_model)),
                    |node| format!("{:?}", slice[node].kind),
                );
                if order.iter().enumerate().any(|(place, &node)| place != node) {
                    for (place, &node) in order.iter().enumerate() {
                        out[region.start + place] = slice[node].clone();
                    }
                }
            }
            for inst in slice.iter().rev() {
                step(&mut now, inst);
            }
            end = region.start;
        }
        block.insts = out;
    }
    *function = seen;
    true
}
