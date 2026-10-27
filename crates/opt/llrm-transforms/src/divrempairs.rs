//! DivRemPairs: a division and the remainder of the same operands share one
//! division, `x % y` being `x - (x / y) * y`.
//!
//! # How it works
//! A translation of LLVM's
//! [`DivRemPairs.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/DivRemPairs.cpp)
//! at d1106deb71cc, `optimizeDivRem` (:178) over `getWorklist` (:121): every
//! `sdiv`/`udiv` and `srem`/`urem` (and the expanded `x - (x / y) * y`,
//! `matchExpandedRem`) is keyed by signedness and operands, and each remainder
//! with a division of its key is a pair. A remainder the division does not
//! dominate in a different block, with no block dominating the other, has the
//! division hoisted to their common predecessor when the remainder's block
//! leads only to the division's; then the remainder is decomposed to the
//! multiply and subtract of the division, the division moved before it if it
//! came after, `exact` and the like dropped from the division, and an operand
//! that may be `undef` frozen.
//!
//! # Deviations
//! - D1: `TTI.hasDivRemOp` is false: no target here has a combined
//!   divide-remainder instruction (arm64's default), so the recompose and
//!   hoist-both arms are not translated. Exit: a `Tti::has_div_rem_op` and
//!   those arms, with the first target that answers true.
//!
//! # Keeps
//! The blocks and edges: dominators and loops are preserved.
//!
//! # Example
//! ```text
//! %q = sdiv i64 %x, %y
//! %r = srem i64 %x, %y        ; becomes  %m = mul i64 %q, %y
//!                             ;          %r = sub i64 %x, %m
//! ```
//!
//! # Used by
//! `pipeline`: `divrempairs@1667` (OptimizePM, after the late combines) and
//! `divrempairs@2373` (LateFPM).

use std::collections::HashMap;
use std::rc::Rc;

use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, Unit};
use llrm_mir::valuetracking::guarantee::{is_guaranteed_not_to_be_undef, is_guaranteed_to_transfer_execution_to_successor};
use llrm_mir::valuetracking::Query;

pub struct DivRemPairs;

/// `DivRemMapKey`: signedness, dividend, divisor.
type Key = (bool, Operand, Operand);

fn binary(
    unit: &Unit,
    inst: InstId,
) -> Option<(BinaryOp, Operand, Operand)> {
    let instruction = unit.function.instruction(inst);
    match instruction.opcode {
        Opcode::Binary(op) => Some((op, instruction.operands[0], instruction.operands[1])),
        _ => None,
    }
}

fn defining(
    unit: &Unit,
    operand: Operand,
) -> Option<InstId> {
    match operand {
        Operand::Value(value) => match unit.function.value(value).def {
            llrm_mir::module::ValueDef::Instruction(def) => Some(def),
            _ => None,
        },
        _ => None,
    }
}

/// `matchExpandedRem`: `x - ((x / y) * y)`, the multiply either way round.
fn match_expanded_rem(
    unit: &Unit,
    inst: InstId,
) -> Option<Key> {
    let (BinaryOp::Sub, dividend, rounded) = binary(unit, inst)? else { return None };
    let (BinaryOp::Mul, left, right) = binary(unit, defining(unit, rounded)?)? else { return None };
    for (division, divisor) in [(left, right), (right, left)] {
        let Some(div) = defining(unit, division) else { continue };
        let Some((op @ (BinaryOp::SDiv | BinaryOp::UDiv), x, y)) = binary(unit, div) else { continue };
        if x == dividend && y == divisor {
            return Some((op == BinaryOp::SDiv, dividend, divisor));
        }
    }
    None
}

/// `getWorklist`: the pairs of a division and a remainder (or an expanded
/// one) of one key, in the remainders' order.
fn worklist(unit: &Unit) -> Vec<(InstId, InstId)> {
    let mut divisions: HashMap<Key, InstId> = HashMap::new();
    let mut remainders: Vec<(Key, InstId)> = Vec::new();
    let mut at: HashMap<Key, usize> = HashMap::new();
    let mut remember = |key: Key, inst: InstId, remainders: &mut Vec<(Key, InstId)>| match at.get(&key) {
        Some(&index) => remainders[index].1 = inst,
        None => {
            at.insert(key, remainders.len());
            remainders.push((key, inst));
        }
    };
    for block in unit.function.layout().to_vec() {
        for inst in unit.function.iter_block(block) {
            match binary(unit, inst) {
                Some((BinaryOp::SDiv, x, y)) => {
                    divisions.insert((true, x, y), inst);
                }
                Some((BinaryOp::UDiv, x, y)) => {
                    divisions.insert((false, x, y), inst);
                }
                Some((BinaryOp::SRem, x, y)) => remember((true, x, y), inst, &mut remainders),
                Some((BinaryOp::URem, x, y)) => remember((false, x, y), inst, &mut remainders),
                _ => {
                    if let Some(key) = match_expanded_rem(unit, inst) {
                        remember(key, inst, &mut remainders);
                    }
                }
            }
        }
    }
    remainders.into_iter().filter_map(|(key, rem)| divisions.get(&key).map(|&div| (div, rem))).collect()
}

/// The facts the value tracking asks, at `cxt`.
fn query<'a>(
    unit: &'a Unit,
    globals: &'a [llrm_mir::module::GlobalValue],
    dt: Option<&'a llrm_mir::domtree::DomTree>,
    cxt: Option<InstId>,
) -> Query<'a> {
    Query {
        context: &*unit.context,
        globals,
        metadata: unit.metadata,
        function: &*unit.function,
        layout: unit.layout,
        dt,
        assumptions: None,
        dom_conditions: None,
        cond_context: None,
        cxt,
    }
}

fn is_expanded(
    unit: &Unit,
    rem: InstId,
) -> bool {
    !matches!(binary(unit, rem), Some((BinaryOp::SRem | BinaryOp::URem, ..)))
}

impl FunctionPass for DivRemPairs {
    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn name(&self) -> &'static str {
        "divrempairs"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("div-rem-pairs")
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = Rc::clone(analyses.outer());
        let dt = analyses.get::<Dominators>(&*unit.context, unit.layout, &*unit.function);
        let mut changed = false;
        for (div, rem) in worklist(unit) {
            let expanded = is_expanded(unit, rem);
            let signed = binary(unit, div).is_some_and(|(op, ..)| op == BinaryOp::SDiv);
            let div_block = unit.function.parent(div).expect("placed");
            let rem_block = unit.function.parent(rem).expect("placed");
            let mut div_dominates = dt.instruction_dominates(unit.function, div, rem);
            if !div_dominates && !dt.instruction_dominates(unit.function, rem, div) {
                // Neither dominates the other: hoist the division to the block
                // the remainder's only comes from, when that block leads to
                // nothing else.
                let safe_to_hoist = |unit: &Unit, inst: InstId, block: BlockId| {
                    let q = query(unit, &outer.globals, None, None);
                    unit.function
                        .iter_block(block)
                        .take_while(|&one| one != inst)
                        .all(|one| is_guaranteed_to_transfer_execution_to_successor(&q, one))
                };
                let single_successor = |unit: &Unit, block: BlockId| {
                    let successors = unit.function.successors(block);
                    (successors.len() == 1).then(|| successors[0])
                };
                let pred = if single_successor(unit, rem_block) == Some(div_block) {
                    unit.function.sole_predecessor(rem_block)
                } else {
                    None
                };
                // (MIR has no `catchswitch` to exclude.)
                let hoistable = pred.filter(|&pred| {
                    let terminator = unit.function.terminator(pred).expect("a terminator");
                    is_guaranteed_to_transfer_execution_to_successor(
                        &query(unit, &outer.globals, None, None),
                        terminator,
                    ) && safe_to_hoist(unit, rem, rem_block)
                        && safe_to_hoist(unit, div, div_block)
                        && unit.function.successors(pred).iter().all(|&b| b == div_block || b == rem_block)
                        && unit.function.predecessors(div_block).iter().all(|&b| b == rem_block || b == pred)
                });
                let Some(pred) = hoistable else { continue };
                div_dominates = true;
                let terminator = unit.function.terminator(pred).expect("a terminator");
                unit.function.move_to(div, Position::Before(terminator)).expect("a placed division");
                changed = true;
            }
            // D1: no combined divide-remainder instruction, so an expanded
            // remainder is already as cheap as it gets.
            if expanded {
                continue;
            }
            let (_, x, y) = binary(unit, rem).expect("a remainder");
            let ty = unit.function.instruction(rem).ty;
            let mul = unit.function.create_instruction(
                Opcode::Binary(BinaryOp::Mul),
                ty,
                vec![Operand::Value(unit.function.instruction(div).result.expect("a division")), y],
                Flags::default(),
                None,
            );
            let sub = unit.function.create_instruction(
                Opcode::Binary(BinaryOp::Sub),
                ty,
                vec![x, Operand::Value(unit.function.instruction(mul).result.expect("a multiply"))],
                Flags::default(),
                None,
            );
            if !div_dominates {
                unit.function.move_to(div, Position::Before(rem)).expect("a placed division");
            }
            unit.function.insert(mul, Position::After(rem)).expect("a placed multiply");
            unit.function.insert(sub, Position::After(mul)).expect("a placed subtract");
            // `exact` on the division would make `x % y` poison.
            unit.function.drop_poison_generating_annotations(div);
            // An operand that may be `undef` is frozen, so the division and the
            // multiply and subtract agree on one value.
            for (index, operand, users) in [(0, x, [(div, 0), (sub, 0)]), (1, y, [(div, 1), (mul, 1)])] {
                let _ = index;
                let sure = is_guaranteed_not_to_be_undef(&query(unit, &outer.globals, Some(&dt), Some(div)), operand, 0);
                if !sure {
                    let ty = unit.function.operand_type(unit.context, operand).expect("a typed operand");
                    let freeze =
                        unit.function.create_instruction(Opcode::Freeze, ty, vec![operand], Flags::default(), None);
                    unit.function.insert(freeze, Position::Before(div)).expect("a placed freeze");
                    let frozen = Operand::Value(unit.function.instruction(freeze).result.expect("a freeze"));
                    for (user, at) in users {
                        unit.function.set_operand(user, at, frozen);
                    }
                }
            }
            let rem_result = unit.function.instruction(rem).result.expect("a remainder");
            let replacement = Operand::Value(unit.function.instruction(sub).result.expect("a subtract"));
            unit.function.replace_value(rem_result, replacement);
            unit.function.erase(rem).expect("nothing reads it");
            let _ = signed;
            changed = true;
        }
        if changed {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}
