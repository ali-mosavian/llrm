//! Loop strength reduction, as LLVM's LSR: an address or integer a loop
//! computes from its counters with two operations or more each iteration
//! becomes a recurrence of its own, one add per iteration, where that
//! leaves no more values live in the loop than before.

use std::collections::BTreeSet;

use crate::context::Constant;
use crate::edit::Position;
use crate::loops::Loop;
use crate::module::{BlockId, Function, InstId, Operand, ValueDef, ValueId};
use crate::opcode::{BinaryOp, Flags, Opcode};
use crate::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, ScalarEvolution, Unit};
use crate::scalarevolution::{Evolution, Linear, Recurrence};
use crate::types::TypeId;

pub struct LoopReduce;

impl FunctionPass for LoopReduce {
    fn name(&self) -> &'static str {
        "loop-reduce"
    }

    fn run(&mut self, unit: &mut Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        let loops = analyses.get::<Loops>(unit.context, unit.layout, unit.function);
        let evolution = analyses.get::<ScalarEvolution>(unit.context, unit.layout, unit.function);
        let mut changed = false;
        for one in loops.loops.iter().rev() {
            let Some((preheader, latch)) = edges(unit.function, one) else { continue };
            let innermost = |block: BlockId| loops.loop_of(block).is_some_and(|inner| inner.header == one.header);
            let insts: Vec<InstId> = unit.function.layout().iter().filter(|&&block| innermost(block)).flat_map(|&block| unit.function.block(block).instructions().to_vec()).collect();
            for inst in insts {
                if unit.function.is_erased(inst) {
                    continue;
                }
                changed |= reduced(unit, &evolution, one, (preheader, latch), inst);
            }
        }
        // Blocks and edges are as they were.
        if changed { PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>() } else { PreservedAnalyses::all() }
    }
}

/// The loop's preheader and its one latch, when those are its only ways in.
fn edges(function: &Function, one: &Loop) -> Option<(BlockId, BlockId)> {
    let [latch] = one.latches[..] else { return None };
    let outside: Vec<BlockId> = function.predecessors(one.header).into_iter().filter(|block| !one.blocks.contains(block)).collect();
    match outside[..] {
        [preheader] if function.successors(preheader) == [one.header] && function.predecessors(one.header).len() == 2 => Some((preheader, latch)),
        _ => None,
    }
}

/// The operations of this loop that compute only what `root` does, root
/// included, and the values they read that are not among them.
fn chain(function: &Function, evolution: &Evolution, one: &Loop, root: InstId, index: Option<ValueId>) -> (BTreeSet<InstId>, Vec<ValueId>) {
    let mut exclusive = BTreeSet::from([root]);
    let mut pending: Vec<ValueId> = match index {
        Some(index) => vec![index],
        None => function.instruction(root).operands.iter().filter_map(|&operand| value_of(operand)).collect(),
    };
    let mut leaves = Vec::new();
    while let Some(value) = pending.pop() {
        let node = match function.value(value).def {
            ValueDef::Instruction(inst)
                if function.instruction(inst).opcode != Opcode::Phi
                    && evolution.of(value).is_some_and(|found| found.header == one.header)
                    && function.users(value).iter().all(|one_use| exclusive.contains(&one_use.user)) =>
            {
                Some(inst)
            }
            _ => None,
        };
        match node {
            Some(inst) if exclusive.insert(inst) => pending.extend(function.instruction(inst).operands.iter().filter_map(|&operand| value_of(operand))),
            Some(_) => {}
            None if !leaves.contains(&value) => leaves.push(value),
            None => {}
        }
    }
    (exclusive, leaves)
}

fn value_of(operand: Operand) -> Option<ValueId> {
    match operand {
        Operand::Value(value) => Some(value),
        _ => None,
    }
}

/// Whether `value` stops being live in the loop once `exclusive` is gone:
/// nothing else in the loop reads it, and nothing after the loop does
/// before it is defined again.
fn freed(function: &Function, one: &Loop, exclusive: &BTreeSet<InstId>, value: ValueId) -> bool {
    let def = match function.value(value).def {
        ValueDef::Instruction(inst) => function.parent(inst),
        ValueDef::Argument(_) => None,
    };
    let mut after = BTreeSet::new();
    let mut work: Vec<BlockId> = one.blocks.iter().flat_map(|&block| function.successors(block)).filter(|block| !one.blocks.contains(block)).collect();
    while let Some(block) = work.pop() {
        if Some(block) != def && after.insert(block) {
            work.extend(function.successors(block));
        }
    }
    function.users(value).iter().filter(|one_use| !exclusive.contains(&one_use.user)).all(|one_use| {
        let instruction = function.instruction(one_use.user);
        let block = match instruction.opcode {
            // A phi reads its input at the end of the edge's source.
            Opcode::Phi => match instruction.operands[one_use.index as usize + 1] {
                Operand::Block(from) => Some(from),
                _ => None,
            },
            _ => function.parent(one_use.user),
        };
        block.is_some_and(|block| !one.blocks.contains(&block) && !after.contains(&block))
    })
}

/// Whether replacing `exclusive` by a recurrence stepping by `step` saves an
/// operation each iteration and adds no value live in the loop.
fn profitable(function: &Function, one: &Loop, (exclusive, leaves): &(BTreeSet<InstId>, Vec<ValueId>), dying: usize, step: &Linear) -> bool {
    let added = 1 + usize::from(!step.terms.is_empty());
    let freed = leaves.iter().filter(|&&leaf| freed(function, one, exclusive, leaf)).count();
    dying >= 2 && added <= freed
}

/// `inst` rewritten over a recurrence of its own, if that saves work.
fn reduced(unit: &mut Unit, evolution: &Evolution, one: &Loop, edges: (BlockId, BlockId), inst: InstId) -> bool {
    let function = &*unit.function;
    let instruction = function.instruction(inst);
    match instruction.opcode {
        // An address one index moves: a byte offset stepping on its own.
        Opcode::GetElementPtr { source } => {
            let indices: Vec<Option<i128>> = instruction.operands[1..].iter().map(|&operand| constant(unit, operand)).collect();
            let (offset, variable) = unit.layout.collect_offset(&unit.context.types, source, &indices);
            let [(position, scale)] = variable[..] else { return false };
            let (base, index) = (instruction.operands[0], instruction.operands[1 + position]);
            let Operand::Value(index_value) = index else { return false };
            let Some(found) = evolution.of(index_value).filter(|found| found.header == one.header) else { return false };
            if !invariant(function, one, base) {
                return false;
            }
            let (exclusive, leaves) = chain(function, evolution, one, inst, Some(index_value));
            let leaves: Vec<ValueId> = leaves.into_iter().filter(|&leaf| Operand::Value(leaf) != base).collect();
            // The address stays, its index and scaling go.
            let dying = exclusive.len() - 1 + usize::from(scale != 1);
            if !profitable(function, one, &(exclusive, leaves), dying, &found.step.scaled(scale as u128, 128)) {
                return false;
            }
            let ty = function.value(index_value).ty;
            let width = unit.context.types.int_bits(ty).expect("an integer index");
            let bytes = Recurrence {
                header: found.header,
                start: found.start.scaled(scale as u128, width).shifted(offset as u128, width),
                step: found.step.scaled(scale as u128, width),
            };
            let (result_ty, flags) = (instruction.ty, instruction.flags);
            let stepped = recurrence(unit, &bytes, ty, edges);
            let i8 = unit.context.types.int(8);
            let address = unit.function.create_instruction(Opcode::GetElementPtr { source: i8 }, result_ty, vec![base, stepped], flags, None);
            unit.function.insert(address, Position::Before(inst)).expect("a placed instruction");
            replaced(unit.function, inst, Operand::Value(unit.function.instruction(address).result.expect("an address")));
            true
        }
        // An integer used by anything but more of the same.
        Opcode::Binary(BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Shl) => {
            let value = instruction.result.expect("a value");
            let Some(found) = evolution.of(value).filter(|found| found.header == one.header).cloned() else { return false };
            let users = function.users(value);
            let inside = users.iter().all(|one_use| function.parent(one_use.user).is_some_and(|block| one.blocks.contains(&block)));
            let ends = users.iter().any(|one_use| function.instruction(one_use.user).result.is_none_or(|result| evolution.of(result).is_none()));
            if !inside || !ends {
                return false;
            }
            let found_chain = chain(function, evolution, one, inst, None);
            let dying = found_chain.0.len();
            if !profitable(function, one, &found_chain, dying, &found.step) {
                return false;
            }
            let ty = instruction.ty;
            let stepped = recurrence(unit, &found, ty, edges);
            replaced(unit.function, inst, stepped);
            true
        }
        _ => false,
    }
}

fn constant(unit: &Unit, operand: Operand) -> Option<i128> {
    let Operand::Constant(id) = operand else { return None };
    let constant = unit.context.get(id);
    let crate::context::ConstantKind::Int(bits) = constant.kind else { return None };
    Some(crate::context::signed(bits, unit.context.types.int_bits(constant.ty)?))
}

fn invariant(function: &Function, one: &Loop, operand: Operand) -> bool {
    match operand {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(inst) => function.parent(inst).is_some_and(|block| !one.blocks.contains(&block)),
            ValueDef::Argument(_) => true,
        },
        _ => true,
    }
}

fn replaced(function: &mut Function, inst: InstId, with: Operand) {
    let result = function.instruction(inst).result.expect("a value");
    function.replace_all_uses_with(result, with);
    function.erase(inst).expect("its uses were replaced");
}

/// A header phi starting at `found`'s start and stepped by the latch.
fn recurrence(unit: &mut Unit, found: &Recurrence, ty: TypeId, (preheader, latch): (BlockId, BlockId)) -> Operand {
    let entry = unit.function.terminator(preheader).expect("a terminator");
    let start = expanded(unit, &found.start, ty, entry);
    let step = expanded(unit, &found.step, ty, entry);
    let poison = unit.context.constant(Constant { ty, kind: crate::context::ConstantKind::Poison });
    let operands = vec![start, Operand::Block(preheader), Operand::Constant(poison), Operand::Block(latch)];
    let phi = unit.function.create_instruction(Opcode::Phi, ty, operands, Flags::default(), None);
    let header = found.header;
    let first = unit.function.block(header).instructions()[0];
    unit.function.insert(phi, Position::Before(first)).expect("a placed block");
    let value = Operand::Value(unit.function.instruction(phi).result.expect("a phi's value"));
    let back = unit.function.terminator(latch).expect("a terminator");
    let next = unit.function.create_instruction(Opcode::Binary(BinaryOp::Add), ty, vec![value, step], Flags::default(), None);
    unit.function.insert(next, Position::Before(back)).expect("a placed instruction");
    unit.function.set_operand(phi, 2, Operand::Value(unit.function.instruction(next).result.expect("a sum")));
    value
}

/// `linear` computed before `before`.
fn expanded(unit: &mut Unit, linear: &Linear, ty: TypeId, before: InstId) -> Operand {
    let mut sum: Option<Operand> = None;
    let emit = |unit: &mut Unit, op: BinaryOp, a: Operand, b: Operand| {
        let inst = unit.function.create_instruction(Opcode::Binary(op), ty, vec![a, b], Flags::default(), None);
        unit.function.insert(inst, Position::Before(before)).expect("a placed instruction");
        Operand::Value(unit.function.instruction(inst).result.expect("a value"))
    };
    for &(value, coefficient) in &linear.terms {
        let term = match coefficient {
            1 => Operand::Value(value),
            _ if coefficient.is_power_of_two() => {
                let amount = Operand::Constant(unit.context.int(ty, i128::from(coefficient.trailing_zeros())));
                emit(unit, BinaryOp::Shl, Operand::Value(value), amount)
            }
            _ => {
                let factor = Operand::Constant(unit.context.int(ty, coefficient as i128));
                emit(unit, BinaryOp::Mul, Operand::Value(value), factor)
            }
        };
        sum = Some(match sum {
            None => term,
            Some(before) => emit(unit, BinaryOp::Add, before, term),
        });
    }
    let constant = Operand::Constant(unit.context.int(ty, linear.constant as i128));
    match sum {
        None => constant,
        Some(sum) if linear.constant == 0 => sum,
        Some(sum) => emit(unit, BinaryOp::Add, sum, constant),
    }
}
