//! Loop-invariant code motion, as LLVM's LICM hoists: an instruction whose
//! operands its loop does not define, safe to run whether or not the loop
//! would, and reading no memory the loop may write, moves to the loop's
//! preheader. Inner loops go first, so what leaves one can leave the next.

use std::collections::{BTreeSet, HashMap};

use crate::context::ConstantKind;
use crate::edit::Position;
use crate::loops::Loop;
use crate::memory;
use crate::module::{BlockId, Function, InstId, Operand, ValueDef};
use crate::opcode::{BinaryOp, Opcode};
use crate::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, Unit};
use crate::valuetracking;

pub struct Licm;

impl FunctionPass for Licm {
    fn name(&self) -> &'static str {
        "licm"
    }

    fn run(&mut self, unit: &mut Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        let tree = analyses.get::<Dominators>(unit.context, unit.layout, unit.function);
        let loops = analyses.get::<Loops>(unit.context, unit.layout, unit.function);
        let mut children: HashMap<BlockId, Vec<BlockId>> = HashMap::new();
        for &block in unit.function.layout() {
            if let Some(parent) = tree.immediate_dominator(block).filter(|_| tree.is_reachable(block)) {
                children.entry(parent).or_default().push(block);
            }
        }
        let mut changed = false;
        // Inner loops come first.
        for one in loops.loops.iter().rev() {
            let Some(preheader) = preheader(unit.function, one) else { continue };
            let writes = one.blocks.iter().flat_map(|&block| unit.function.block(block).instructions().to_vec()).any(|inst| memory::of(unit.context, unit.callees, unit.function, inst).writes);
            let before = unit.function.terminator(preheader).expect("a terminator");
            // Down the dominator tree, so an operand hoists before its user.
            let mut stack = vec![one.header];
            while let Some(block) = stack.pop() {
                for inst in unit.function.block(block).instructions().to_vec() {
                    if invariant(unit, &one.blocks, inst) && safe(unit, inst, writes) {
                        unit.function.move_to(inst, Position::Before(before)).expect("a placed instruction");
                        changed = true;
                    }
                }
                stack.extend(children.get(&block).into_iter().flatten().filter(|child| one.blocks.contains(child)));
            }
        }
        // Blocks and edges are as they were.
        if changed { PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>() } else { PreservedAnalyses::all() }
    }
}

/// The loop's one way in from outside, if it is a block that only enters.
fn preheader(function: &Function, one: &Loop) -> Option<BlockId> {
    let outside: Vec<BlockId> = function.predecessors(one.header).into_iter().filter(|block| !one.blocks.contains(block)).collect();
    match outside[..] {
        [block] if function.successors(block) == [one.header] => Some(block),
        _ => None,
    }
}

/// Whether nothing `inst` reads is defined in the loop of `blocks`.
fn invariant(unit: &Unit, blocks: &BTreeSet<BlockId>, inst: InstId) -> bool {
    let function = &*unit.function;
    function.instruction(inst).operands.iter().all(|operand| match *operand {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Argument(_) => true,
            ValueDef::Instruction(def) => function.parent(def).is_some_and(|block| !blocks.contains(&block)),
        },
        Operand::Constant(_) => true,
        Operand::Block(_) => false,
    })
}

/// Whether `inst` may run where the loop would not have run it, in a loop
/// that `writes` memory or not: LLVM's `isSafeToSpeculativelyExecute`.
fn safe(unit: &Unit, inst: InstId, writes: bool) -> bool {
    let function = &*unit.function;
    let instruction = function.instruction(inst);
    match &instruction.opcode {
        Opcode::Binary(BinaryOp::UDiv | BinaryOp::URem | BinaryOp::SDiv | BinaryOp::SRem) => {
            // A constant divisor other than 0, and than -1 where it may overflow.
            let Operand::Constant(id) = instruction.operands[1] else { return false };
            let ConstantKind::Int(bits) = unit.context.get(id).kind else { return false };
            let width = unit.context.types.int_bits(instruction.ty).unwrap_or(128);
            let all_ones = if width >= 128 { u128::MAX } else { (1 << width) - 1 };
            bits != 0 && (matches!(instruction.opcode, Opcode::Binary(BinaryOp::UDiv | BinaryOp::URem)) || bits != all_ones)
        }
        Opcode::Binary(_) | Opcode::Cast(_) | Opcode::ICmp(_) | Opcode::FCmp(_) | Opcode::GetElementPtr { .. } | Opcode::Select | Opcode::FNeg
        | Opcode::ExtractValue(_) => true,
        Opcode::Load { volatile: false, .. } => {
            let pointer = instruction.operands[0];
            let bytes = unit.layout.store_size(&unit.context.types, instruction.ty);
            valuetracking::dereferenceable(unit.context, unit.layout, function, pointer, bytes)
                && (!writes || memory::invariant(unit.context, unit.layout, function, pointer))
        }
        _ => false,
    }
}
