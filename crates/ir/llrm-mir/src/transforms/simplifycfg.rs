//! The control flow graph made smaller, to a fixed point: LLVM's
//! SimplifyCFG. A branch on a constant takes its one edge, blocks nothing
//! reaches go, a block with one predecessor that only it leaves for joins
//! it, and a block that only jumps on is bypassed.

use std::collections::BTreeSet;

use crate::context::{Constant, ConstantKind, Context};
use crate::dominators::DominatorTree;
use crate::edit::Position;
use crate::loops::LoopInfo;
use crate::module::{BlockId, Function, InstId, Operand};
use crate::opcode::{Flags, Opcode};
use crate::passes::{Analyses, FunctionPass, PreservedAnalyses, Unit};

pub struct SimplifyCfg;

impl FunctionPass for SimplifyCfg {
    fn name(&self) -> &'static str {
        "simplifycfg"
    }

    fn run(&mut self, unit: &mut Unit, _: &mut Analyses) -> PreservedAnalyses {
        let mut changed = false;
        while folded_branches(unit.context, unit.function) | removed_unreachable(unit.context, unit.function) | merged(unit.function) | bypassed(unit.function) {
            changed = true;
        }
        if changed { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

fn terminator(function: &Function, block: BlockId) -> InstId {
    function.terminator(block).expect("a terminated block")
}

fn phis(function: &Function, block: BlockId) -> Vec<InstId> {
    function.block(block).instructions().iter().copied().take_while(|&one| function.instruction(one).opcode == Opcode::Phi).collect()
}

/// `block` no longer an edge into `into`: its phi inputs go.
fn drop_edge(function: &mut Function, from: BlockId, into: BlockId) {
    for phi in phis(function, into) {
        let operands: Vec<Operand> =
            function.instruction(phi).operands.chunks(2).filter(|pair| pair[1] != Operand::Block(from)).flatten().copied().collect();
        function.set_operands(phi, operands);
    }
}

/// A conditional branch on a constant, or to one block both ways, jumps.
fn folded_branches(context: &Context, function: &mut Function) -> bool {
    let mut changed = false;
    for block in function.layout().to_vec() {
        let branch = terminator(function, block);
        let instruction = function.instruction(branch);
        let [condition, Operand::Block(taken), Operand::Block(otherwise)] = instruction.operands[..] else { continue };
        if instruction.opcode != Opcode::Br {
            continue;
        }
        let (kept, dropped) = match condition {
            _ if taken == otherwise => (taken, None),
            Operand::Constant(id) => match context.get(id).kind {
                ConstantKind::Int(0) => (otherwise, Some(taken)),
                ConstantKind::Int(_) => (taken, Some(otherwise)),
                _ => continue,
            },
            _ => continue,
        };
        if let Some(dropped) = dropped {
            drop_edge(function, block, dropped);
        } else {
            // One of the two edges into `kept` goes; its phis name `block` once.
            for phi in phis(function, kept) {
                let mut seen = false;
                let operands: Vec<Operand> = function
                    .instruction(phi)
                    .operands
                    .chunks(2)
                    .filter(|pair| pair[1] != Operand::Block(block) || !std::mem::replace(&mut seen, true))
                    .flatten()
                    .copied()
                    .collect();
                function.set_operands(phi, operands);
            }
        }
        let jump = function.create_instruction(Opcode::Br, function.instruction(branch).ty, vec![Operand::Block(kept)], Flags::default(), None);
        function.insert(jump, Position::Before(branch)).expect("a placed branch");
        function.erase(branch).expect("a branch has no value");
        changed = true;
    }
    changed
}

/// Blocks the entry does not reach, gone.
fn removed_unreachable(context: &mut Context, function: &mut Function) -> bool {
    let entry = function.entry().expect("a defined function");
    let mut reached = BTreeSet::new();
    let mut work = vec![entry];
    while let Some(block) = work.pop() {
        if reached.insert(block) {
            work.extend(function.successors(block));
        }
    }
    let dead: Vec<BlockId> = function.layout().iter().copied().filter(|block| !reached.contains(block)).collect();
    for &block in &dead {
        for successor in function.successors(block) {
            if reached.contains(&successor) {
                drop_edge(function, block, successor);
            }
        }
    }
    // Values defined there are used only there, or by phis on dropped edges.
    for &block in &dead {
        for inst in function.block(block).instructions().to_vec().into_iter().rev() {
            if let Some(result) = function.instruction(inst).result {
                let poison = context.constant(Constant { ty: function.value(result).ty, kind: ConstantKind::Poison });
                function.replace_all_uses_with(result, Operand::Constant(poison));
            }
            function.erase(inst).expect("its uses were replaced");
        }
    }
    for &block in &dead {
        function.erase_block(block).expect("an emptied block nothing names");
    }
    !dead.is_empty()
}

/// A block whose one predecessor leaves only for it, joined to that
/// predecessor.
fn merged(function: &mut Function) -> bool {
    let entry = function.entry().expect("a defined function");
    for block in function.layout().to_vec() {
        let predecessors = function.predecessors(block);
        let [single] = predecessors[..] else { continue };
        if block == entry || single == block || function.successors(single) != vec![block] || function.instruction(terminator(function, single)).opcode != Opcode::Br {
            continue;
        }
        // A phi with one input is that input.
        for phi in phis(function, block) {
            let input = function.instruction(phi).operands[0];
            function.replace_all_uses_with(function.instruction(phi).result.expect("a phi's value"), input);
            function.erase(phi).expect("its uses were replaced");
        }
        let jump = terminator(function, single);
        for inst in function.block(block).instructions().to_vec() {
            function.move_to(inst, Position::Before(jump)).expect("a placed instruction");
        }
        function.erase(jump).expect("a branch has no value");
        // Phis after `block` now come from `single`.
        function.replace_block_uses_with(block, single);
        function.erase_block(block).expect("an emptied block nothing names");
        return true;
    }
    false
}

/// A block holding only a jump, its predecessors sent straight on, where
/// the target's phis can tell them apart.
fn bypassed(function: &mut Function) -> bool {
    let entry = function.entry().expect("a defined function");
    for block in function.layout().to_vec() {
        let jump = terminator(function, block);
        let [Operand::Block(target)] = function.instruction(jump).operands[..] else { continue };
        if block == entry || target == block || function.block(block).instructions().len() != 1 {
            continue;
        }
        let predecessors = function.predecessors(block);
        let target_predecessors = function.predecessors(target);
        // A predecessor already an edge into the target would need its phi
        // inputs to agree; LLVM's `CanPropagatePredecessorsForPHIs`.
        let overlap = predecessors.iter().any(|one| target_predecessors.contains(one));
        let has_phis = !phis(function, target).is_empty();
        if predecessors.is_empty() || overlap && has_phis {
            continue;
        }
        // A loop's preheader stays, as SimplifyCFG keeps canonical loops
        // (`NeedCanonicalLoop`); so does a loop's exit before phis, whose
        // copies would otherwise run inside the loop, as CodeGenPrepare's
        // `isMergingEmptyBlockProfitable` keeps it.
        let loops = LoopInfo::new(function, &DominatorTree::new(function));
        if loops.loop_of(target).is_some_and(|one| one.header == target)
            || has_phis && predecessors.iter().any(|&one| loops.leaves(one, target))
        {
            continue;
        }
        for phi in phis(function, target) {
            let operands = function.instruction(phi).operands.clone();
            let mut rewritten = Vec::new();
            for pair in operands.chunks(2) {
                if pair[1] == Operand::Block(block) {
                    rewritten.extend(predecessors.iter().flat_map(|&from| [pair[0], Operand::Block(from)]));
                } else {
                    rewritten.extend_from_slice(pair);
                }
            }
            function.set_operands(phi, rewritten);
        }
        for predecessor in &predecessors {
            let branch = terminator(function, *predecessor);
            for (at, operand) in function.instruction(branch).operands.clone().into_iter().enumerate() {
                if operand == Operand::Block(block) {
                    function.set_operand(branch, at, Operand::Block(target));
                }
            }
        }
        function.erase(jump).expect("a branch has no value");
        function.erase_block(block).expect("an emptied block nothing names");
        return true;
    }
    false
}
