//! Puts every natural loop in loop-closed SSA form, so loop passes can change a
//! loop without rewriting the readers of its values elsewhere.
//!
//! # How it works
//! A value defined in a loop and read outside it is read through a phi in the
//! loop's exit block. Later loop passes can then change the loop's body, copy
//! it or move its exits without rewriting readers outside it.
//!
//! Loops are taken innermost first, so an inner exit phi is an ordinary
//! definition of the enclosing loop. For one loop:
//!
//! - With a single exit block whose predecessors are exactly the exiting
//!   blocks, a value is closed when every block that reads it is dominated by
//!   the exit and its definition dominates every exiting block. Each such value
//!   gets a phi in the exit with one input per exiting block, and every outside
//!   reader the exit dominates is rewritten to read it. A phi that reads the
//!   value on an edge counts at that edge's source block. If the exit has other
//!   predecessors, the loop is left for `loopsimplify`.
//! - With several exit blocks, [`crate::lcssamerges`] makes a phi at each and
//!   merges them where the exits' paths join.
//!
//! Phis are only added, as `formLCSSA` does: a phi whose inputs are all one
//! value stays for CodeGenPrepare's phi fold. The new phi is named after its
//! value with `.lcssa`. The pass is idempotent.
//!
//! The helpers `arms`, `from_arms`, `operations`, `definitions`, `exit_phi` and
//! `place_phi` read and build phis for many other loop passes.
//!
//! # Example
//! Before:
//!
//! ```mir
//! define i16 @f(i16 %seed, i16 %n) {
//! b0:
//!   br label %b1
//!
//! b1:
//!   %carried = phi i16 [ %seed, %b0 ], [ %stepped, %b2 ]
//!   %more = icmp ult i16 %carried, %n
//!   br i1 %more, label %b2, label %b3
//!
//! b2:
//!   %stepped = add i16 %carried, 1
//!   br label %b1
//!
//! b3:
//!   %answer = add i16 %carried, 7
//!   ret i16 %answer
//! }
//! ```
//!
//! After:
//!
//! ```mir
//! b3:
//!   %carried.lcssa = phi i16 [ %carried, %b1 ]
//!   %answer = add i16 %carried.lcssa, 7
//!   ret i16 %answer
//! ; ... b0 to b2 unchanged
//! ```
//!
//! `%carried` is read in `b3`, outside the loop, so the exit block gets a phi
//! of it and the reader reads the phi.
//!
//! From unit test `test_a_loop_value_used_after_the_exit_gets_an_exit_phi` in
//! `lcssa_tests.rs`.
//!
//! # Runs
//! In every body's pipeline after `loopsimplify`, unless `lcssa` is turned off,
//! and again after header copying. The unroller calls it first.
//!
//! # Keeps
//! Dominators and loops stay valid; all other analyses are invalidated when
//! something changed.
//!
//! # See also
//! LLVM's LCSSA (LCSSA.cpp, `formLCSSA`).

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::cfg;
use llrm_analysis::cfg::Around;
use llrm_analysis::graph::loops::Loop;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueId};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, Unit};

use crate::lcssamerges;

pub struct LoopClosedSSA;

impl FunctionPass for LoopClosedSSA {
    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn name(&self) -> &'static str {
        "lcssa"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let shape = analyses.get::<cfg::Shape>(unit.context, unit.layout, unit.function);
        match closed_in(unit.function, &shape) {
            Ok(true) => PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>(),
            Ok(false) => PreservedAnalyses::all(),
            Err(error) => panic!("lcssa: {error}"),
        }
    }
}

/// `function` with every supported natural loop in closed SSA form; whether
/// anything changed.
pub fn closed(function: &mut Function) -> Result<bool, String> {
    let shape = cfg::Shape::of(function);
    closed_in(function, &shape)
}

/// [`closed`] over the `shape` the caller holds: the loops are found once.
pub fn closed_in(
    function: &mut Function,
    shape: &cfg::Shape,
) -> Result<bool, String> {
    // The edges leaving each loop, found once: closing adds phis, never edges.
    let exits = shape.loops.iter().map(|loop_| loop_.exits(function)).collect::<Vec<_>>();
    formed_with(function, shape, &exits)
}

/// `formLCSSA` of every loop: the exit phis a loop lacks are added, and none
/// is taken away, as `closed` does first for the phis no loop closes at.
pub fn formed(function: &mut Function) -> Result<bool, String> {
    let shape = cfg::Shape::of(function);
    let exits = shape.loops.iter().map(|loop_| loop_.exits(function)).collect::<Vec<_>>();
    formed_with(function, &shape, &exits)
}

fn formed_with(
    function: &mut Function,
    shape: &cfg::Shape,
    exits: &[Vec<(i64, i64)>],
) -> Result<bool, String> {
    let mut changed = false;
    // Loops come inner first. Closing an inner loop first makes its exit
    // value an ordinary definition in an enclosing loop.
    // The dominators and the graph, found once: closing adds phis, no block or
    // edge.
    let mut flow = lcssamerges::Flow::with_dominance(shape.dominance.clone());
    for (loop_, exiting) in shape.loops.iter().zip(exits) {
        changed |= _closed_loop(function, loop_, exiting, &mut flow)?;
    }
    Ok(changed)
}

/// `block`'s instructions other than its phis, in order.
pub(crate) fn operations(
    function: &Function,
    block: BlockId,
) -> Vec<InstId> {
    function.iter_block(block).filter(|&one| function.instruction(one).opcode != Opcode::Phi).collect()
}

pub(crate) use llrm_analysis::occurrence::{arms, from_arms};

/// The values defined in `blocks`, each with its block.
pub(crate) fn definitions(
    function: &Function,
    blocks: &llrm_mir::forest::Body,
) -> BTreeMap<ValueId, i64> {
    let mut defined = BTreeMap::new();
    for &at in blocks {
        let block = cfg::block(at);
        for inst in function.iter_block(block) {
            if let Some(value) = function.instruction(inst).result {
                defined.insert(value, cfg::id(block));
            }
        }
    }
    defined
}

/// A read of a loop's value from outside it: the block of the reader, and for
/// a phi the block its input comes from.
pub(crate) struct Read {
    pub home: i64,
    pub from: Option<i64>,
}

/// The values defined in `body` that a reader outside it reads, each with its
/// block and the reads, by value. Found from the uses of what the loop defines
/// (LLVM's formLCSSA), not by reading every instruction outside for each loop.
pub(crate) fn outside_reads(
    function: &Function,
    body: &llrm_mir::forest::Body,
) -> Vec<(ValueId, i64, Vec<Read>)> {
    let mut found = Vec::new();
    for &at in body {
        let block = cfg::block(at);
        for inst in function.iter_block(block) {
            let Some(value) = function.instruction(inst).result else { continue };
            let mut reads = Vec::new();
            for one in function.users(value) {
                let Some(home) = function.parent(one.user).map(cfg::id) else { continue };
                if body.contains(&home) {
                    continue;
                }
                let user = function.instruction(one.user);
                let from = if user.opcode == Opcode::Phi {
                    let Operand::Block(from) = user.operands[one.index as usize + 1] else { continue };
                    Some(cfg::id(from))
                } else {
                    None
                };
                reads.push(Read { home, from });
            }
            if !reads.is_empty() {
                found.push((value, cfg::id(block), reads));
            }
        }
    }
    // By value, as a map of them was walked: the phis are made in this order.
    found.sort_unstable_by_key(|(value, ..)| *value);
    found
}

/// A new, unplaced phi of `value`'s type and name, without inputs yet.
pub(crate) fn exit_phi(
    function: &mut Function,
    value: ValueId,
) -> InstId {
    let data = function.value(value);
    let name = data.name.as_ref().map(|name| format!("{name}.lcssa"));
    function.create_instruction(Opcode::Phi, data.ty, Vec::new(), Flags::default(), name.as_deref())
}

/// Places `phi` after `block`'s phis.
pub(crate) fn place_phi(
    function: &mut Function,
    block: BlockId,
    phi: InstId,
) -> Result<(), String> {
    let first = operations(function, block).first().copied();
    function.insert(phi, first.map_or(Position::End(block), Position::Before))
}

pub fn _closed_loop(
    function: &mut Function,
    loop_: &Loop,
    exiting: &[(i64, i64)],
    flow: &mut lcssamerges::Flow,
) -> Result<bool, String> {
    let exits = exiting.iter().map(|&(_, target)| target).collect::<BTreeSet<_>>();
    if exits.len() > 1 {
        return lcssamerges::closed(function, loop_, exiting, flow);
    }
    let dominance = &mut flow.dominance;
    let Some(&exit_at) = exits.first() else {
        return Ok(false);
    };
    let sources = exiting.iter().map(|&(source, _)| source).collect::<BTreeSet<_>>();
    if cfg::predecessors_of(function, exit_at) != sources {
        return Ok(false);
    }

    // Phi inputs are used on their incoming edge.  A phi in the dedicated
    // exit is already the LCSSA boundary, so only downstream phis count here.
    // Found from the uses of what the loop defines (LLVM's formLCSSA), keeping
    // only the values something outside reads, not every value the loop makes.
    let mut escaping: Vec<(ValueId, i64, BTreeSet<i64>)> = Vec::new();
    for (value, home, reads) in outside_reads(function, &loop_.body) {
        let sites = reads
            .iter()
            .filter_map(|read| match read.from {
                Some(_) if read.home == exit_at => None,
                Some(from) => Some(from),
                None => Some(read.home),
            })
            .collect::<BTreeSet<_>>();
        if !sites.is_empty() {
            escaping.push((value, home, sites));
        }
    }
    if escaping.is_empty() {
        return Ok(false);
    }

    let dominance = &*dominance.get_or_insert_with(|| cfg::Dominance::of(function));
    let crossing = escaping
        .iter()
        .filter(|(_, home, sites)| {
            sites.iter().all(|&site| dominance.dominates(function, exit_at, site))
                && sources.iter().all(|&source| dominance.dominates(function, *home, source))
        })
        .map(|(value, ..)| *value)
        .collect::<Vec<_>>();
    if crossing.is_empty() {
        return Ok(false);
    }

    let edges_in = function.edge_predecessors(cfg::block(exit_at));
    let mut swap: BTreeMap<ValueId, Operand> = BTreeMap::new();
    let mut phis = Vec::new();
    for &value in &crossing {
        let phi = exit_phi(function, value);
        let incoming = edges_in.iter().map(|&source| (Operand::Value(value), source)).collect::<Vec<_>>();
        function.set_operands(phi, from_arms(&incoming));
        swap.insert(value, Operand::Value(function.instruction(phi).result.expect("a phi's value")));
        phis.push(phi);
    }

    // Every use of a crossing value outside the loop that the exit dominates
    // reads its phi.
    let mut rewrites = Vec::new();
    for (&value, &replacement) in &swap {
        for one in function.users(value) {
            let Some(home) = function.parent(one.user).map(cfg::id) else { continue };
            if loop_.body.contains(&home) {
                continue;
            }
            let user = function.instruction(one.user);
            let reads_it = if user.opcode == Opcode::Phi {
                home != exit_at
                    && matches!(
                        user.operands[one.index as usize + 1],
                        Operand::Block(from) if dominance.dominates(function, exit_at, cfg::id(from))
                    )
            } else {
                dominance.dominates(function, exit_at, home)
            };
            if reads_it {
                rewrites.push((one.user, one.index as usize, replacement));
            }
        }
    }
    for (user, index, operand) in rewrites {
        function.set_operand(user, index, operand);
    }
    for phi in phis {
        place_phi(function, cfg::block(exit_at), phi)?;
    }
    Ok(true)
}

#[cfg(test)]
#[path = "lcssa_tests.rs"]
pub mod tests;
