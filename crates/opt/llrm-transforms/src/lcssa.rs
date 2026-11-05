//! Puts every natural loop in loop-closed SSA form, so loop passes can change a
//! loop without rewriting the readers of its values elsewhere.
//!
//! # How it works
//! A value defined in a loop and read outside it is read through a phi in the
//! loop's exit block. Later loop passes can then change the loop's body, copy
//! it or move its exits without rewriting readers outside it.
//!
//! This is `LCSSA.cpp`. Loops are taken innermost first
//! (`formLCSSARecursively`), so an inner exit phi is an ordinary definition of
//! the enclosing loop. For one loop (`formLCSSA`):
//!
//! - The blocks that dominate an exit and sit directly in the loop are found by
//!   walking immediate dominators up from the exits.
//! - Each value they define, with a use but not a lone one in its own block, is
//!   a candidate.
//! - A candidate's uses outside the loop (a phi's counting at its incoming
//!   block) are rewritten. A use in an unreachable block becomes poison.
//! - Each exit block its definition dominates gets a phi, named after the value
//!   with `.lcssa`, placed after the block's phis (L4), with one input per predecessor.
//!   An input from a block outside the loop is itself rewritten.
//! - A use in an exit block reads that block's phi, a lone new phi is read
//!   directly, and any other use goes through `SsaUpdater`, which places a phi
//!   where the exits' paths join.
//! - A phi that nothing reads is erased again.
//!
//! Phis are only added, as `formLCSSA` does: a phi whose inputs are all one
//! value stays for CodeGenPrepare's phi fold. The pass is idempotent.
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
//! # Deviations
//! - L1 Lifetime markers are ordinary users (2026-11-04, w2, exit 2026-11-12):
//!   the pin drops them when one crosses the loop boundary, but a pass has no
//!   callee table to know them by. A marker names an alloca, which sits in the
//!   entry block, never in a loop.
//! - L2 No `ScalarEvolution` and no debug records: the pin caches the SCEV of a
//!   new phi and moves `dbg.value`s; neither is kept here (S1 of `ssaupdater`).
//! - L3 A phi's inputs come in predecessor id order, not use-list order.
//! - L4 An exit phi goes after the block's phis, in value-id order (2026-11-05, w2,
//!   exit 2026-11-12, row QR-1): the pin puts it first. The register allocator adds
//!   spills for some orders of an exit block's phis where the pin's code size is
//!   constant; the old order is the one that does not (x86 jpeg Os, 6502 grep Os).
//!
//! # See also
//! LLVM's
//! [`LCSSA.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LCSSA.cpp).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::Rc;

use llrm_analysis::cfg;
use llrm_analysis::ssaupdater::SsaUpdater;
use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::edit::Position;
use llrm_mir::loopinfo::{Loop, LoopId, LoopInfo};
use llrm_mir::module::{BlockId, Function, InstId, Operand, Use, ValueId};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, Unit};
use llrm_mir::types::Type;

pub struct LoopClosedSSA;

impl FunctionPass for LoopClosedSSA {
    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn name(&self) -> &'static str {
        "lcssa"
    }

    /// `LCSSAPass::run`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LCSSA.cpp#L557>
    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let tree = analyses.get::<Dominators>(unit.context, unit.layout, unit.function);
        let info = analyses.get::<Loops>(unit.context, unit.layout, unit.function);
        match form_on_all_loops(unit.context, unit.layout, unit.function, &tree, &info) {
            Ok(true) => PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>(),
            Ok(false) => PreservedAnalyses::all(),
            Err(error) => panic!("lcssa: {error}"),
        }
    }
}

/// `function` with every natural loop in closed SSA form; whether anything
/// changed.
pub fn closed(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
) -> Result<bool, String> {
    let tree = DomTree::of(function);
    let info = LoopInfo::analyze(&*function, || DomTree::of(function));
    form_on_all_loops(context, layout, function, &tree, &info)
}

/// `formLCSSAOnAllLoops`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LCSSA.cpp#L477>
fn form_on_all_loops(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    tree: &DomTree,
    info: &LoopInfo,
) -> Result<bool, String> {
    let mut form = Form { context, layout, function, tree, info, exit_blocks: HashMap::new(), placed: HashMap::new() };
    let mut changed = false;
    for top in info.top_level_loops() {
        changed |= form.recursively(top)?;
    }
    Ok(changed)
}

/// `LoopExitBlocksTy` and the analyses one run reads.
struct Form<'a> {
    context: &'a Context,
    layout: &'a DataLayout,
    function: &'a mut Function,
    tree: &'a DomTree,
    info: &'a LoopInfo,
    /// The loops' exit blocks, found once each: closing adds phis, no edge.
    exit_blocks: HashMap<LoopId, Rc<[BlockId]>>,
    /// The phis this run added, with the value each closes (L4).
    placed: HashMap<InstId, ValueId>,
}

impl Form<'_> {
    /// `formLCSSARecursivelyImpl`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LCSSA.cpp#L455>
    fn recursively(
        &mut self,
        loop_: Loop,
    ) -> Result<bool, String> {
        let mut changed = false;
        for sub in loop_.get_sub_loops() {
            changed |= self.recursively(sub)?;
        }
        Ok(changed | self.form_loop(loop_)?)
    }

    /// The exit blocks of `loop_`, from the cache.
    fn exits_of(
        &mut self,
        loop_: Loop,
    ) -> Rc<[BlockId]> {
        let function = &*self.function;
        Rc::clone(self.exit_blocks.entry(loop_.id()).or_insert_with(|| loop_.get_exit_blocks(function).into()))
    }

    /// `formLCSSAImpl`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LCSSA.cpp#L382>
    fn form_loop(
        &mut self,
        loop_: Loop,
    ) -> Result<bool, String> {
        let exits = self.exits_of(loop_);
        if exits.is_empty() {
            return Ok(false);
        }
        let dominating = self.blocks_dominating_exits(loop_, &exits);
        let mut worklist = Vec::new();
        for block in dominating {
            if self.info.get_loop_for(block) != Some(loop_) {
                continue;
            }
            for inst in self.function.iter_block(block) {
                let Some(value) = self.function.instruction(inst).result else { continue };
                let mut users = self.function.users(value).iter();
                let Some(first) = users.next() else { continue };
                if users.next().is_none()
                    && self.function.parent(first.user) == Some(block)
                    && self.function.instruction(first.user).opcode != Opcode::Phi
                {
                    continue;
                }
                if matches!(
                    self.context.types.get(self.function.value(value).ty),
                    Type::Token
                ) {
                    continue;
                }
                worklist.push(inst);
            }
        }
        self.for_instructions(worklist)
    }

    /// `computeBlocksDominatingExits`, in the order the pin's set holds them.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LCSSA.cpp#L340>
    fn blocks_dominating_exits(
        &self,
        loop_: Loop,
        exits: &[BlockId],
    ) -> Vec<BlockId> {
        let mut found = Vec::new();
        let mut seen = HashSet::new();
        let mut worklist = exits.to_vec();
        while let Some(block) = worklist.pop() {
            if loop_.get_header() == block {
                continue;
            }
            let Some(idom) = self.tree.immediate_dominator(block) else { continue };
            if !loop_.contains(idom) {
                continue;
            }
            if seen.insert(idom) {
                found.push(idom);
                worklist.push(idom);
            }
        }
        found
    }

    /// `formLCSSAForInstructionsImpl`: for each instruction of `worklist`, from
    /// the back, phis in the exits and its outside uses rewritten to read them.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LCSSA.cpp#L82>
    fn for_instructions(
        &mut self,
        mut worklist: Vec<InstId>,
    ) -> Result<bool, String> {
        let info = self.info;
        let mut changed = false;
        let mut local_to_remove: Vec<InstId> = Vec::new();
        // `PredIteratorCache`.
        let mut pred_cache: HashMap<BlockId, Rc<[BlockId]>> = HashMap::new();
        while let Some(inst) = worklist.pop() {
            let value = self.function.instruction(inst).result.expect("a worklist instruction has a value");
            let inst_block = self.function.parent(inst).expect("a placed instruction");
            let loop_ = info.get_loop_for(inst_block).expect("an instruction of a block in a loop");
            let exits = self.exits_of(loop_);
            if exits.is_empty() {
                continue;
            }
            let ty = self.function.value(value).ty;

            let mut to_rewrite: Vec<Use> = Vec::new();
            for one in self.function.users(value).iter().copied().collect::<Vec<_>>() {
                let mut user_block = self.function.parent(one.user).expect("a placed user");
                // Skip uses in unreachable blocks.
                if !self.tree.is_reachable_from_entry(user_block) {
                    let poison = Operand::Constant(self.context.constant(Constant { ty, kind: ConstantKind::Poison }));
                    self.function.set_operand(one.user, one.index as usize, poison);
                    continue;
                }
                // A use in a phi occurs in the respective predecessor block.
                if self.function.instruction(one.user).opcode == Opcode::Phi {
                    user_block = self.incoming_block(one);
                }
                if inst_block != user_block && !loop_.contains(user_block) {
                    to_rewrite.push(one);
                }
            }
            if to_rewrite.is_empty() {
                continue;
            }

            // An invoke's result is not available along its unwind edge.
            let dom_block = match self.function.instruction(inst).opcode {
                Opcode::Invoke(_) => {
                    let operands = &self.function.instruction(inst).operands;
                    let Operand::Block(normal) = operands[operands.len() - 3] else { unreachable!("a normal dest") };
                    normal
                }
                _ => inst_block,
            };

            let mut added: Vec<InstId> = Vec::new();
            let mut post_process: Vec<InstId> = Vec::new();
            let name = self.function.value(value).name.clone();
            let mut updater = SsaUpdater::new(ty, name.as_deref());

            // Phis in all of the exit blocks the value dominates.
            for &exit in exits.iter() {
                if !self.tree.dominates(dom_block, exit) {
                    continue;
                }
                if updater.has_value_for_block(exit) {
                    continue;
                }
                let preds =
                    Rc::clone(pred_cache.entry(exit).or_insert_with(|| self.function.edge_predecessors(exit).into()));
                let phi = exit_phi(self.function, value);
                let incoming = preds.iter().map(|&pred| (Operand::Value(value), pred)).collect::<Vec<_>>();
                self.function.set_operands(phi, from_arms(&incoming));
                insert_in_value_order(self.function, exit, phi, value, &self.placed)?;
                self.placed.insert(phi, value);
                // An exit with a predecessor outside the loop: that input is
                // rewritten in terms of a different phi.
                for (at, &pred) in preds.iter().enumerate() {
                    if !loop_.contains(pred) {
                        to_rewrite.push(Use { user: phi, index: 2 * at as u32 });
                    }
                }
                added.push(phi);
                let made = Operand::Value(self.function.instruction(phi).result.expect("a phi's value"));
                updater.add_available_value(exit, made);
                // An exit that is the header of a disjoint loop: its phi may
                // need closing for that loop.
                if let Some(other) = info.get_loop_for(exit)
                    && !loop_.contains_loop(Some(other))
                {
                    post_process.push(phi);
                }
            }

            for one in to_rewrite {
                let mut user_block = self.function.parent(one.user).expect("a placed user");
                if self.function.instruction(one.user).opcode == Opcode::Phi {
                    user_block = self.incoming_block(one);
                }
                // A use in an exit block reads the phi placed there.
                let front = added.iter().copied().find(|&one| self.function.parent(one) == Some(user_block));
                if let Some(front) = front
                    && exits.contains(&user_block)
                {
                    let phi = self.function.instruction(front).result.expect("a phi's value");
                    self.function.set_operand(one.user, one.index as usize, Operand::Value(phi));
                    continue;
                }
                // A lone phi dominates every use.
                if let [only] = added[..] {
                    let phi = self.function.instruction(only).result.expect("a phi's value");
                    self.function.set_operand(one.user, one.index as usize, Operand::Value(phi));
                    continue;
                }
                updater.rewrite_use(self.context, self.layout, self.function, one);
            }

            // SSAUpdater may have placed phis inside other loops.
            for &placed in updater.inserted_phis() {
                let block = self.function.parent(placed).expect("a placed phi");
                if let Some(other) = info.get_loop_for(block)
                    && !loop_.contains_loop(Some(other))
                {
                    post_process.push(placed);
                }
            }
            for phi in post_process {
                let used = self.function.instruction(phi).result.is_some_and(|v| !self.function.users(v).is_empty());
                if used {
                    worklist.push(phi);
                }
            }
            // Phis nothing was rewritten to read.
            for phi in added {
                let value = self.function.instruction(phi).result.expect("a phi's value");
                if self.function.users(value).is_empty() && !local_to_remove.contains(&phi) {
                    local_to_remove.push(phi);
                }
            }
            changed = true;
        }
        // The use check again: a later phi may read one.
        for phi in local_to_remove {
            let value = self.function.instruction(phi).result.expect("a phi's value");
            if self.function.users(value).is_empty() {
                self.function.erase(phi)?;
            }
        }
        Ok(changed)
    }

    /// `PHINode::getIncomingBlock(Use)`.
    fn incoming_block(
        &self,
        one: Use,
    ) -> BlockId {
        let Operand::Block(from) = self.function.instruction(one.user).operands[one.index as usize + 1] else {
            unreachable!("a phi's block operand")
        };
        from
    }
}

/// Places `phi` for `value` after the block's phis and the phis this run added
/// for lower values (DEVIATE L4: the pin puts it first in the block).
fn insert_in_value_order(
    function: &mut Function,
    block: BlockId,
    phi: InstId,
    value: ValueId,
    added: &HashMap<InstId, ValueId>,
) -> Result<(), String> {
    let mut first_other = None;
    for inst in function.iter_block(block) {
        if function.instruction(inst).opcode != Opcode::Phi {
            first_other = Some(inst);
            break;
        }
        match added.get(&inst) {
            Some(&of) if of > value => {
                first_other = Some(inst);
                break;
            }
            _ => {}
        }
    }
    function.insert(phi, first_other.map_or(Position::End(block), Position::Before))
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

#[cfg(test)]
#[path = "lcssa_tests.rs"]
pub mod tests;

#[cfg(test)]
#[path = "lcssa_multi_exit_tests.rs"]
mod multi_exit_tests;
