//! Work moved down to the one place that needs it, so the paths that skip
//! that place no longer run it: gcc's `-ftree-sink` (`pass_sink_code`),
//! LLVM's `Sink` and `MachineSink`.
//!
//! # How it works
//! A load whose every reader lies below one block `T` that its own block `B`
//! dominates moves to the top of the deepest such `T` where it still gains:
//! `T` runs no oftener than `B`, in no deeper loop, and some path from `B` to
//! an exit skips `T`. No reader may lie in a deeper loop than `T` (`hoist`
//! left it where it is), and nothing on any path from the load to `T` may
//! write what it reads (`memoryssa::Writers`). An operation (the `gep` of a
//! sunk load) moves the same way, but only when every instruction that reads
//! it has moved: alone it may part a flag setter from its compare.
//!
//! The moves are made on a copy and kept only if together they save more than
//! disturbing the allocation can cost: the work no longer run on the skipped
//! paths (each instruction's price at the frequency of `B` less that of `T`)
//! must exceed one register copy for each execution of the moved work where it
//! now runs. A 3% skip in sum_three at -m16 -O2 cost an instruction; a skip of
//! over half is what d_faces has.
//!
//! The memory accesses and the block frequencies are asked of the manager when
//! a move first needs them, so a body with nothing that passes the shape of
//! the function asks for none.
//!
//! Blocks are taken last first, and each from its end, so what a sunk
//! instruction read is next in line: the `gep` of a sunk load goes with it.
//! Blocks and edges are as they were.
//!
//! ```text
//!  b24:  %t = load ptr, ptr %a          b24:  %c = icmp ...
//!        %c = icmp ...           ==>          br %c, b28, b26
//!        br %c, b28, b26                b26:  %t = load ptr, ptr %a
//!  b26:  ... %t ...                           ... %t ...
//! ```
//!
//! d_faces's inner loop loaded eight arrays' pointers at its top and stored
//! the ones it needed after a call to the stack; a face the visibility test
//! skipped paid for all of them.
//!
//! # Example
//! Before, `%v` loaded in `b0` and read only in `b1`, which a negative `%c`
//! reaches:
//!
//! ```mir
//! b0:
//!   %v = load i32, ptr @g
//!   %cold = icmp slt i32 %c, 0
//!   br i1 %cold, label %b1, label %b2
//!
//! b1:
//!   %r = add i32 %v, 1
//!   ret i32 %r
//!
//! b2:
//!   ret i32 0
//! ```
//!
//! After, as the test asserts it:
//!
//! ```mir
//! b0:
//!   %cold = icmp slt i32 %c, 0
//!   br i1 %cold, label %b1, label %b2
//!
//! b1:
//!   %v = load i32, ptr @g
//!   %r = add i32 %v, 1
//!   ret i32 %r
//!
//! b2:
//!   ret i32 0
//! ```
//!
//! From unit test `test_a_load_read_in_one_arm_moves_into_it` in
//! `sink_tests.rs`.
//!
//! # Runs
//! Last of the function passes before `switchconv`, after `addresssink`; off
//! with `-fno-sink` and at -O0.
//!
//! # Keeps
//! Dominators and loops: no block or edge changes.

use llrm_analysis::cfg;
use llrm_analysis::memoryssa::{Accesses, Writers};
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{self, Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses};

pub struct Sink {
    pub size: bool,
}

impl FunctionPass for Sink {
    fn name(&self) -> &'static str {
        "sink"
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        if sunk(unit, analyses, self.size) {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// Edges of the function by block.
struct Edges {
    succ: IdMap<BlockId, Vec<BlockId>>,
    pred: IdMap<BlockId, Vec<BlockId>>,
}

impl Edges {
    fn of(function: &Function) -> Self {
        let mut succ: IdMap<BlockId, Vec<BlockId>> = IdMap::new();
        let mut pred: IdMap<BlockId, Vec<BlockId>> = IdMap::new();
        for block in cfg::graph(function) {
            let from = cfg::block(block.at);
            for to in block.succ {
                let to = cfg::block(to);
                succ.get_or_insert_with(from, Vec::new).push(to);
                pred.get_or_insert_with(to, Vec::new).push(from);
            }
        }
        Self { succ, pred }
    }

    /// What `from` reaches along `edges`, never entering `blocked`.
    fn reach(
        edges: &IdMap<BlockId, Vec<BlockId>>,
        from: impl IntoIterator<Item = BlockId>,
        blocked: &[BlockId],
    ) -> IdSet<BlockId> {
        let mut seen = IdSet::new();
        let mut work: Vec<BlockId> = from.into_iter().filter(|block| !blocked.contains(block)).collect();
        while let Some(block) = work.pop() {
            if !seen.insert(block) {
                continue;
            }
            work.extend(edges.get(&block).into_iter().flatten().filter(|next| !blocked.contains(next)));
        }
        seen
    }

    /// Whether some path from `from` ends without passing `through`.
    fn escapes(
        &self,
        from: BlockId,
        through: BlockId,
    ) -> bool {
        Self::reach(&self.succ, [from], &[through]).iter().any(|block| self.succ.get(&block).is_none_or(Vec::is_empty))
    }

    /// The blocks on a path from `from` to `to` that enters neither again.
    fn between(
        &self,
        from: BlockId,
        to: BlockId,
    ) -> Vec<BlockId> {
        let forward = Self::reach(&self.succ, self.succ.get(&from).cloned().unwrap_or_default(), &[from, to]);
        let backward = Self::reach(&self.pred, self.pred.get(&to).cloned().unwrap_or_default(), &[from, to]);
        forward.iter().filter(|block| backward.contains(block)).collect()
    }
}

/// Whether `inst` may be moved without reading anything but its operands, or
/// is a load: and whether it is the second.
fn _kind(
    function: &Function,
    inst: InstId,
) -> Option<bool> {
    let instruction = function.instruction(inst);
    instruction.result?;
    match instruction.opcode {
        Opcode::Load { volatile: false, .. } => Some(true),
        Opcode::GetElementPtr { .. } => Some(false),
        Opcode::Binary(_) | Opcode::Cast(_) | Opcode::ICmp(_) | Opcode::FNeg | Opcode::Freeze => Some(false),
        _ => None,
    }
}

/// Where each reader of `value` reads it: its block, or for a phi the block
/// the value arrives from.
fn _readers(
    function: &Function,
    inst: InstId,
) -> Option<Vec<BlockId>> {
    let result = function.instruction(inst).result?;
    let mut blocks = Vec::new();
    for one in function.users(result) {
        let user = function.instruction(one.user);
        let block = if user.opcode == Opcode::Phi {
            match user.operands.get(one.index as usize + 1) {
                Some(Operand::Block(from)) => *from,
                _ => return None,
            }
        } else {
            function.parent(one.user)?
        };
        blocks.push(block);
    }
    (!blocks.is_empty()).then_some(blocks)
}

/// Whether every reader of `inst` has itself moved.
fn _feeds_moved(
    function: &Function,
    inst: InstId,
    moved: &IdSet<InstId>,
) -> bool {
    function
        .instruction(inst)
        .result
        .is_some_and(|result| function.users(result).iter().all(|one| moved.contains(&one.user)))
}

pub fn sunk(
    unit: &mut passes::Unit,
    analyses: &mut Analyses,
    size: bool,
) -> bool {
    let dominators = analyses.get::<Dominators>(unit.context, unit.layout, unit.function);
    let loops = analyses.get::<Loops>(unit.context, unit.layout, unit.function);
    let outer = std::rc::Rc::clone(analyses.outer());
    // What is asked of the manager only where a move passes the shape of the
    // function: a body with nothing that can move asks for none of it, and it
    // is then no more than the pass's own cost (`cases` at -Os asked for
    // the memory accesses and the loop trips of each function, 31 M that the
    // lowering of its switch after the pass made again).
    let analyses = std::cell::RefCell::new(analyses);
    let accesses: std::cell::OnceCell<Option<std::rc::Rc<Accesses>>> = std::cell::OnceCell::new();
    let accesses = || {
        accesses
            .get_or_init(|| {
                Accesses::managed(unit.context, unit.layout, unit.function, &mut analyses.borrow_mut()).ok()
            })
            .as_deref()
    };
    let unusable = std::cell::Cell::new(false);
    let frequency: std::cell::OnceCell<Option<std::collections::BTreeMap<i64, i64>>> = std::cell::OnceCell::new();
    let frequency =
        || frequency.get_or_init(|| crate::hoist::_frequency(unit, &mut analyses.borrow_mut(), &outer, size));
    let edges = Edges::of(unit.function);
    let costs = if size { outer.target().size_costs() } else { crate::profit::costs(&outer) };
    let mut saved = 0i64;
    let mut ran = 0i64;
    // The moves are made on a copy and kept only if together they save more
    // than disturbing the allocation can cost (below).
    let mut trial = unit.function.clone();
    let mut plan: Vec<(InstId, InstId)> = Vec::new();
    let weight = |block: BlockId| frequency().as_ref().and_then(|map| map.get(&cfg::id(block)).copied());
    let blocks: Vec<BlockId> = unit.function.layout().to_vec();
    let mut sunk_set: IdSet<InstId> = IdSet::new();
    for &home in blocks.iter().rev() {
        let insts: Vec<InstId> = trial.iter_block(home).collect::<Vec<_>>();
        for (at, &inst) in insts.iter().enumerate().rev() {
            let Some(load) = _kind(&trial, inst) else { continue };
            let Some(readers) = _readers(&trial, inst) else { continue };
            if readers.contains(&home) {
                continue;
            }
            // An operation goes only with the instructions that read it:
            // alone it may take a flag setter from its compare, and saves
            // nothing a spill does not.
            if !load && !_feeds_moved(&trial, inst, &sunk_set) {
                continue;
            }
            // The deepest block every reader lies below.
            let mut meet = readers[0];
            let mut ok = true;
            for &reader in &readers[1..] {
                while !dominators.dominates(meet, reader) {
                    match dominators.immediate_dominator(meet) {
                        Some(up) => meet = up,
                        None => {
                            ok = false;
                            break;
                        }
                    }
                }
            }
            if !ok || meet == home || !dominators.dominates(home, meet) {
                continue;
            }
            let mut target = Some(meet);
            let mut place = None;
            while let Some(candidate) = target {
                if candidate == home {
                    break;
                }
                // Not nearer to a loop that reads it: that is where `hoist`
                // left it, and a move there only shuffles the allocation.
                let gains = loops.get_loop_depth(candidate) <= loops.get_loop_depth(home)
                    && readers.iter().all(|&reader| loops.get_loop_depth(reader) <= loops.get_loop_depth(candidate))
                    && edges.escapes(home, candidate)
                    && !matches!(
                        trial
                            .iter_block(candidate)
                            .map(|one| &trial.instruction(one).opcode)
                            .find(|opcode| **opcode != Opcode::Phi),
                        Some(Opcode::LandingPad { .. }) | None
                    )
                    && match (weight(candidate), weight(home)) {
                        (Some(there), Some(here)) => there <= here,
                        _ => true,
                    };
                if gains
                    && (!load
                        || match accesses() {
                            Some(accesses) => _unwritten(
                                unit,
                                &trial,
                                accesses,
                                &outer,
                                &edges,
                                home,
                                candidate,
                                &insts[at + 1..],
                                inst,
                            ),
                            None => {
                                unusable.set(true);
                                false
                            }
                        })
                {
                    place = Some(candidate);
                    break;
                }
                target = dominators.immediate_dominator(candidate);
            }
            let Some(candidate) = place else { continue };
            let first = trial
                .iter_block(candidate)
                .find(|&one| trial.instruction(one).opcode != Opcode::Phi)
                .expect("a terminated block");
            llrm_support::debug!(
                "sink",
                "b{} -> b{}: {} (frequency {:?} -> {:?})",
                cfg::id(home),
                cfg::id(candidate),
                trial.instruction(inst).opcode.mnemonic(),
                weight(home),
                weight(candidate)
            );
            if let (Some(here), Some(there), Some(price)) = (
                weight(home),
                weight(candidate),
                crate::profit::operation(unit.context, unit.layout, &trial, outer.callees(), inst, &costs),
            ) {
                saved += price * (here - there);
                ran += there;
            }
            trial.move_to(inst, Position::Before(first)).expect("a placed instruction");
            plan.push((inst, first));
            sunk_set.insert(inst);
        }
    }
    // Where the accesses could not be found nothing moves.
    if plan.is_empty() || unusable.get() {
        return false;
    }
    // What the moves save must outweigh what disturbing the allocation can
    // cost: one register copy for each execution of the moved work, where it
    // now runs (a 3% skip in sum_three at -m16 -O2 cost an instruction).
    if frequency().is_some() && saved < costs.r#move * ran {
        llrm_support::debug!("sink", "refused: saves {saved}, a copy where it runs is {}", costs.r#move * ran);
        return false;
    }
    for (inst, first) in plan {
        unit.function.move_to(inst, Position::Before(first)).expect("a placed instruction");
    }
    true
}

/// Whether nothing between the load `inst` in `home` and the top of `target`
/// may write what it reads: the rest of `home`, and the blocks on the way.
#[allow(clippy::too_many_arguments)]
fn _unwritten(
    unit: &passes::Unit,
    function: &Function,
    accesses: &Accesses,
    outer: &passes::Outer,
    edges: &Edges,
    home: BlockId,
    target: BlockId,
    rest: &[InstId],
    inst: InstId,
) -> bool {
    if llrm_mir::memory::invariant_load(unit.context, unit.layout, function, inst) {
        return true;
    }
    let Some(read) = accesses.references.get(&inst) else { return false };
    let mut between: Vec<InstId> = rest.to_vec();
    for block in edges.between(home, target) {
        between.extend(function.iter_block(block));
    }
    Writers::of(accesses, &between).spare(accesses, Some(outer.program()), read)
}

#[allow(dead_code)]
fn _defined_in(
    function: &Function,
    value: llrm_mir::module::ValueId,
) -> Option<BlockId> {
    match function.value(value).def {
        ValueDef::Instruction(def) => function.parent(def),
        ValueDef::Argument(_) => None,
    }
}

#[cfg(test)]
#[path = "sink_tests.rs"]
mod tests;
