//! Allocas only loaded and stored, whole, made SSA values: LLVM's
//! `PromoteMemToReg`. A phi goes where the stores' iterated dominance
//! frontier meets a block the value is live into, so SSA is pruned.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::context::{Constant, ConstantKind, Context};
use crate::dominators::DominatorTree;
use crate::edit::Position;
use crate::module::{BlockId, Function, InstId, Operand, ValueId};
use crate::opcode::{Flags, Opcode};
use crate::passes::{Analyses, Dominators, FunctionPass, PreservedAnalyses, Unit};
use crate::types::TypeId;

pub struct Mem2Reg;

impl FunctionPass for Mem2Reg {
    fn name(&self) -> &'static str {
        "mem2reg"
    }

    fn run(&mut self, unit: &mut Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        let allocas = promotable(unit.context, unit.function);
        if allocas.is_empty() {
            return PreservedAnalyses::all();
        }
        let tree = analyses.get::<Dominators>(unit.context, unit.layout, unit.function);
        promote(unit, &tree, &allocas);
        PreservedAnalyses::none().preserve::<Dominators>()
    }
}

/// Each alloca whose every use is a load of its type or a store of one into
/// it, neither volatile, with the type it holds.
fn promotable(context: &Context, function: &Function) -> Vec<(InstId, ValueId, TypeId)> {
    let mut out = Vec::new();
    for (_, inst) in function.walk() {
        let instruction = function.instruction(inst);
        let Opcode::Alloca { allocated, .. } = instruction.opcode else { continue };
        let Some(address) = instruction.result else { continue };
        let whole = function.users(address).iter().all(|one| {
            let user = function.instruction(one.user);
            match user.opcode {
                Opcode::Load { volatile: false, .. } => user.ty == allocated,
                Opcode::Store { volatile: false, .. } => {
                    one.index == 1 && function.operand_type(context, user.operands[0]) == Some(allocated)
                }
                _ => false,
            }
        });
        let single = instruction.operands.first().is_none_or(|&count| match count {
            Operand::Constant(id) => context.get(id).kind == ConstantKind::Int(1),
            _ => false,
        });
        if whole && single {
            out.push((inst, address, allocated));
        }
    }
    out
}

fn promote(unit: &mut Unit, tree: &DominatorTree, allocas: &[(InstId, ValueId, TypeId)]) {
    let function = &mut *unit.function;
    let slots: HashMap<ValueId, usize> = allocas.iter().enumerate().map(|(at, (_, address, _))| (*address, at)).collect();
    let frontiers = frontiers(function, tree);
    // Each phi placed: its block and slot, and the block each input comes from.
    let mut phis: HashMap<(BlockId, usize), (InstId, Vec<BlockId>)> = HashMap::new();
    for (slot, &(_, address, ty)) in allocas.iter().enumerate() {
        let (defining, live) = defining_and_live(function, tree, address);
        let mut work: Vec<BlockId> = defining.iter().copied().collect();
        let mut placed = BTreeSet::new();
        while let Some(block) = work.pop() {
            for &frontier in frontiers.get(&block).into_iter().flatten() {
                if !live.contains(&frontier) || !placed.insert(frontier) {
                    continue;
                }
                let predecessors = function.predecessors(frontier);
                let poison = unit.context.constant(Constant { ty, kind: ConstantKind::Poison });
                let operands = predecessors.iter().flat_map(|&from| [Operand::Constant(poison), Operand::Block(from)]).collect();
                let phi = function.create_instruction(Opcode::Phi, ty, operands, Flags::default(), None);
                let first = function.block(frontier).instructions().first().copied();
                function.insert(phi, first.map_or(Position::End(frontier), Position::Before)).expect("a placed block");
                phis.insert((frontier, slot), (phi, predecessors));
                if !defining.contains(&frontier) {
                    work.push(frontier);
                }
            }
        }
    }
    // Rename down the dominator tree, each slot's value as it stands.
    let entry = function.entry().expect("a defined function");
    let children = children(function, tree);
    let initial: Vec<Operand> =
        allocas.iter().map(|&(_, _, ty)| Operand::Constant(unit.context.constant(Constant { ty, kind: ConstantKind::Poison }))).collect();
    let mut dead = Vec::new();
    let mut stack = vec![(entry, initial)];
    while let Some((block, mut current)) = stack.pop() {
        for slot in 0..allocas.len() {
            if let Some((phi, _)) = phis.get(&(block, slot)) {
                current[slot] = Operand::Value(function.instruction(*phi).result.expect("a phi's value"));
            }
        }
        for inst in function.block(block).instructions().to_vec() {
            let instruction = function.instruction(inst);
            match instruction.opcode {
                Opcode::Load { .. } => {
                    if let Operand::Value(address) = instruction.operands[0]
                        && let Some(&slot) = slots.get(&address)
                    {
                        function.replace_all_uses_with(instruction.result.expect("a load's value"), current[slot]);
                        dead.push(inst);
                    }
                }
                Opcode::Store { .. } => {
                    if let Operand::Value(address) = instruction.operands[1]
                        && let Some(&slot) = slots.get(&address)
                    {
                        current[slot] = instruction.operands[0];
                        dead.push(inst);
                    }
                }
                _ => {}
            }
        }
        for successor in function.successors(block) {
            for slot in 0..allocas.len() {
                let Some((phi, from)) = phis.get(&(successor, slot)) else { continue };
                for (at, _) in from.iter().enumerate().filter(|(_, one)| **one == block) {
                    function.set_operand(*phi, 2 * at, current[slot]);
                }
            }
        }
        for &child in children.get(&block).into_iter().flatten() {
            stack.push((child, current.clone()));
        }
    }
    // What unreachable blocks read of a slot is poison.
    for (_, inst) in function.walk().collect::<Vec<_>>() {
        let instruction = function.instruction(inst);
        if tree.is_reachable(function.parent(inst).expect("placed")) || dead.contains(&inst) {
            continue;
        }
        match instruction.opcode {
            Opcode::Load { .. } if matches!(instruction.operands[0], Operand::Value(one) if slots.contains_key(&one)) => {
                let poison = unit.context.constant(Constant { ty: instruction.ty, kind: ConstantKind::Poison });
                function.replace_all_uses_with(instruction.result.expect("a load's value"), Operand::Constant(poison));
                dead.push(inst);
            }
            Opcode::Store { .. } if matches!(instruction.operands[1], Operand::Value(one) if slots.contains_key(&one)) => dead.push(inst),
            _ => {}
        }
    }
    for inst in dead.into_iter().chain(allocas.iter().map(|(inst, ..)| *inst)) {
        function.erase(inst).expect("a promoted access has no users left");
    }
}

/// The blocks that store to `address`, and those it is live into: that read
/// it before any store of their own, and the predecessors that pass it on.
fn defining_and_live(function: &Function, tree: &DominatorTree, address: ValueId) -> (BTreeSet<BlockId>, BTreeSet<BlockId>) {
    let mut first: BTreeMap<BlockId, bool> = BTreeMap::new(); // a block's first access: a load?
    let mut defining = BTreeSet::new();
    for &block in function.layout() {
        if !tree.is_reachable(block) {
            continue;
        }
        for &inst in function.block(block).instructions() {
            let instruction = function.instruction(inst);
            let (load, store) = match instruction.opcode {
                Opcode::Load { .. } => (instruction.operands[0] == Operand::Value(address), false),
                Opcode::Store { .. } => (false, instruction.operands[1] == Operand::Value(address)),
                _ => (false, false),
            };
            if store {
                defining.insert(block);
            }
            if load || store {
                first.entry(block).or_insert(load);
            }
        }
    }
    let mut live = BTreeSet::new();
    let mut work: Vec<BlockId> = first.iter().filter(|(_, load)| **load).map(|(block, _)| *block).collect();
    while let Some(block) = work.pop() {
        if !live.insert(block) {
            continue;
        }
        for predecessor in function.predecessors(block) {
            if tree.is_reachable(predecessor) && !defining.contains(&predecessor) {
                work.push(predecessor);
            }
        }
    }
    (defining, live)
}

/// Each reachable block's dominance frontier, by Cooper, Harvey and Kennedy.
fn frontiers(function: &Function, tree: &DominatorTree) -> HashMap<BlockId, BTreeSet<BlockId>> {
    let mut out: HashMap<BlockId, BTreeSet<BlockId>> = HashMap::new();
    for &block in function.layout() {
        let predecessors: Vec<BlockId> = function.predecessors(block).into_iter().filter(|one| tree.is_reachable(*one)).collect();
        if !tree.is_reachable(block) || predecessors.len() < 2 {
            continue;
        }
        let dominator = tree.immediate_dominator(block);
        for mut runner in predecessors {
            while Some(runner) != dominator {
                out.entry(runner).or_default().insert(block);
                match tree.immediate_dominator(runner) {
                    Some(next) if next != runner => runner = next,
                    _ => break,
                }
            }
        }
    }
    out
}

fn children(function: &Function, tree: &DominatorTree) -> HashMap<BlockId, Vec<BlockId>> {
    let mut out: HashMap<BlockId, Vec<BlockId>> = HashMap::new();
    for &block in function.layout() {
        if let Some(parent) = tree.immediate_dominator(block).filter(|parent| *parent != block && tree.is_reachable(block)) {
            out.entry(parent).or_default().push(block);
        }
    }
    out
}
