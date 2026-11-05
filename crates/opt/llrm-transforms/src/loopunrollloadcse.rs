//! `loadCSE`
//! ([LoopUnroll.cpp:282](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopUnroll.cpp#L282))
//! and `getMatchingValue` (L260): after a partial unroll, a simple load whose
//! address has the SCEV of an earlier load that dominates it and that nothing
//! may have written since is that load's value.
//!
//! The dominator tree of the loop is walked from the header with a scope per
//! node, the loads available at it keyed by the SCEV of their address. Each
//! entry carries the generation of memory when it was made: a block with
//! more than one predecessor and every instruction that may write memory
//! start a new one. A load of the same generation is the available value; one
//! of an older generation is it where MemorySSA's clobbering access of the
//! load dominates the access of the available one.
//!
//! # Deviations
//! - LC1 MemorySSA is made over a copy of the whole function (LLVM: over the
//!   loop, `MemorySSA(*L, AA, DT)`), once, and only when two loads of the loop
//!   share an address SCEV: without such a pair nothing can match. It is not
//!   updated as loads go, as LLVM's is not. Exit: a loop-scoped build.
//! - LC2 The SCEVs are made before the walk, by the pass's `ScalarEvolution`
//!   (the one `simplifyLoopIVs` asked), since the walk changes the function and
//!   LLVM's holds its expressions current.
//!
//! # Used by
//! [`crate::loopunrollpass::unroll_loops`], where the loop is still a loop.

use llrm_analysis::memoryssa::Accesses;
use llrm_analysis::mssa::view::Translated;
use llrm_analysis::mssa::{Mssa, shadow};
use llrm_analysis::scev::Cx;
use llrm_analysis::scev::expr::Scev;
use llrm_mir::module::{BlockId, InstId, Operand};
use llrm_mir::opcode::Opcode;
use llrm_support::hash::HashMap;

use crate::earlycse::Scoped;
use crate::loopunrollpass::{Analyzed, Run};

/// `LoadValue`: the load that made the value available, and the generation of
/// memory it was loaded in.
#[derive(Clone, Copy)]
struct LoadValue {
    def: InstId,
    generation: u32,
}

/// A step of the walk of the dominator tree.
enum Step {
    Enter(BlockId, u32),
    Leave,
}

/// `loadCSE(L, DT, SE, LI, BAA, GetMSSA)` of the loop headed by `header`;
/// whether a load was replaced.
pub(crate) fn load_cse(
    run: &mut Run,
    analyzed: &Analyzed,
    header: BlockId,
) -> bool {
    let Some(lp) = analyzed.loops.get_loop_for(header) else { return false };
    if lp.get_header() != header {
        return false;
    }
    let blocks = lp.get_blocks().to_vec();

    // The SCEV of each simple load's address (LC2).
    let mut keys: HashMap<InstId, Scev> = HashMap::default();
    {
        // The pass's expressions, which `simplifyLoopIVs` just asked: the pin
        // keeps one `ScalarEvolution` for both.
        let se = &mut run.scev;
        let cx = Cx {
            context: &*run.context,
            globals: run.globals,
            metadata: run.metadata,
            function: &*run.function,
            layout: run.layout,
            loops: &analyzed.loops,
            tree: &analyzed.tree,
            assumptions: &analyzed.assumptions,
        };
        for &block in &blocks {
            for inst in run.function.iter_block(block) {
                let operation = run.function.instruction(inst);
                if !matches!(operation.opcode, Opcode::Load { volatile: false, .. }) {
                    continue;
                }
                let pointer = operation.operands[0];
                if se.is_value_scevable(&cx, pointer) {
                    keys.insert(inst, se.get_scev(&cx, pointer));
                }
            }
        }
    }
    // Two loads of one address are all there is to match (LC1).
    let mut seen: HashMap<Scev, u32> = HashMap::default();
    for key in keys.values() {
        *seen.entry(*key).or_insert(0) += 1;
    }
    if !seen.values().any(|&count| count > 1) {
        return false;
    }

    // MemorySSA of the function as it stands (LC1).
    let before = run.function.fork();
    let shape = llrm_analysis::cfg::Shape::of(&before);
    let view = llrm_analysis::memory::Unit::within(&*run.context, run.layout, &before, run.outer).with_shape(&shape);
    let accesses = Accesses::lazy();
    let mssa = Mssa::build(&before, &shape.dominance, &shadow::effect_of_in(&accesses, &view));
    let memory = Translated::new(&view, &accesses, mssa);

    let mut available: Scoped<Scev, LoadValue> = Scoped::new();
    let mut changed = false;
    let mut walk = vec![Step::Enter(header, 0)];
    while let Some(step) = walk.pop() {
        let (block, entry_generation) = match step {
            Step::Leave => {
                available.leave();
                continue;
            }
            Step::Enter(block, generation) => (block, generation),
        };
        available.enter();
        let mut generation = entry_generation;
        // "If this block has a single predecessor, then the predecessor is the
        // parent of the domtree node and all of the live out memory values are
        // still current in this block."
        let mut predecessors: Vec<BlockId> = run.function.predecessors(block).into_iter().collect();
        predecessors.sort_unstable();
        predecessors.dedup();
        if predecessors.len() != 1 {
            generation += 1;
        }
        for inst in run.function.iter_block(block).collect::<Vec<_>>() {
            if run.function.is_erased(inst) {
                continue;
            }
            let Some(&key) = keys.get(&inst) else {
                if may_write(run, inst) {
                    generation += 1;
                }
                continue;
            };
            let matching =
                available.lookup(&key).copied().and_then(|value| matching_value(run, &memory, value, inst, generation));
            match matching {
                Some(value) => {
                    let result = run.function.instruction(inst).result.expect("a load has a value");
                    if crate::looprotate::replacement_preserves_lcssa_form(
                        &analyzed.loops,
                        run.function,
                        inst,
                        Operand::Value(value),
                    ) {
                        run.function.replace_all_uses_with(result, Operand::Value(value));
                        run.function.erase(inst).expect("a load with no uses");
                        changed = true;
                    }
                }
                None => available.insert(key, LoadValue { def: inst, generation }),
            }
        }
        walk.push(Step::Leave);
        for child in analyzed.tree.children(Some(block)).into_iter().rev() {
            if lp.contains(child) {
                walk.push(Step::Enter(child, generation));
            }
        }
    }
    changed
}

/// `getMatchingValue`: the value of `available`'s load for `load`, if it is
/// of the same type and nothing wrote memory between them.
fn matching_value(
    run: &Run,
    memory: &Translated<'_>,
    available: LoadValue,
    load: InstId,
    generation: u32,
) -> Option<llrm_mir::module::ValueId> {
    let earlier = run.function.instruction(available.def).result?;
    let later = run.function.instruction(load).result?;
    if run.function.value(earlier).ty != run.function.value(later).ty {
        return None;
    }
    if available.generation != generation {
        let earlier_access = memory.memory_access(available.def)?;
        let later_def = memory.clobbering_access(load)?;
        if !memory.dominates(later_def, earlier_access) {
            return None;
        }
    }
    Some(earlier)
}

/// `Instruction::mayWriteToMemory`.
fn may_write(
    run: &Run,
    inst: InstId,
) -> bool {
    match run.function.instruction(inst).opcode {
        Opcode::Store { .. } => true,
        Opcode::Load { volatile, .. } => volatile,
        Opcode::Call(_) | Opcode::Invoke(_) => {
            llrm_mir::memory::of(run.context, run.callees, run.function, inst).writes
        }
        _ => false,
    }
}
