//! A phi of two constants that a branch chooses between becomes arithmetic of
//! the branch's condition: gcc's `pass_phiopt` (`conditional_replacement`,
//! `two_value_replacement`, and match.pd's `cond ? c : 0` as `-cond & c` through
//! `match_simplify_replacement`, tree-ssa-phiopt.cc) and LLVM's
//! `FoldTwoEntryPHINode` (SimplifyCFG). gcc runs it at -O1 and above
//! (`-fssa-phiopt`, opts.cc:609, passes.def:96, :230, :252, :354).
//!
//! `br c, T, F` where each of `T` and `F` is empty or the other side is the
//! join `J`, and `J`'s phis each take a constant from either side. Each phi is
//! the form that costs the least on the target, against the branch it
//! replaces (`OperationCosts`: the branch, a jump from the arm that is not the
//! fall-through, a copy per phi, against the condition held as a value (`set`)
//! once and an extend and the operations per phi):
//! - `F + zext c` where the constants differ by one, `F + sext c` by minus one;
//! - `(zext c << k) + F` where they differ by a power of two;
//! - `zext c * (T - F) + F` where the target prices that multiply low;
//! - `(sext c & (T - F)) + F` otherwise.
//! There is no conditional move on a 486, so a select is a branch anyway (the
//! backend's `selects`); this is what the branch is worth when it is not one.
//! LLVM's `two-entry-phi-node-folding-threshold` (4) is the same test with
//! its own unit.

use llrm_mir::context::{ConstantKind, Context, mask, signed};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::passes::{Analyses, FunctionPass, PreservedAnalyses, Unit};
use llrm_mir::target::{Machine, OperationCosts};
use llrm_mir::types::TypeId;

use crate::edges;
use crate::lcssa::arms;

/// `size`: priced in bytes (-Os), as the target states them.
pub struct PhiOpt {
    pub size: bool,
}

impl FunctionPass for PhiOpt {
    fn name(&self) -> &'static str {
        "phiopt"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        let target = outer.target();
        let costs = if self.size { target.size_costs() } else { target.costs() };
        // The target prices a multiply in time, not in bytes.
        let scaled = (!self.size).then_some(target);
        if !can_win(&costs) {
            return PreservedAnalyses::all();
        }
        let mut changed = false;
        while let Some(plan) = planned(unit.context, unit.function, &costs, scaled) {
            applied(unit.context, unit.function, plan);
            changed = true;
        }
        if !changed {
            return PreservedAnalyses::all();
        }
        llrm_analysis::cfg::_unreachable(unit.context, unit.function);
        crate::cfg::merged(unit.function);
        PreservedAnalyses::none()
    }
}

/// Whether any form can cost no more than the branch: the cheapest, one phi of
/// an extended condition, against the loosest branch, a diamond around one
/// copy. Another phi adds a copy to the branch's price and an extend to the
/// form's, so where an extend costs more than a copy one is the best case. A
/// target where none can win skips the walk of the body.
fn can_win(costs: &OperationCosts) -> bool {
    costs.extend <= costs.r#move || costs.set + costs.extend <= costs.branch + costs.branch / 2 + costs.r#move
}

/// How one phi's two constants come from the condition.
enum Form {
    /// `F + zext c`, `F + sext c`: the constants differ by one, either way.
    Step { sign: bool },
    /// `(zext c << shift) + F`.
    Shifted { shift: u32 },
    /// `(sext c & difference) + F`.
    Masked { difference: i128 },
    /// `zext c * difference + F`, where the target makes the multiply of a few
    /// instructions (a `lea` for 3, 5 and 9).
    Scaled { difference: i128 },
}

struct Rewrite {
    phi: InstId,
    ty: TypeId,
    /// The constant the condition's false side gives.
    otherwise: i128,
    form: Form,
}

struct Plan {
    block: BlockId,
    join: BlockId,
    condition: Operand,
    rewrites: Vec<Rewrite>,
}

struct Sides {
    condition: Operand,
    join: BlockId,
    /// The blocks `join`'s phis name for the condition's true and false sides.
    yes: BlockId,
    no: BlockId,
    diamond: bool,
}

/// `block`'s branch on a condition between two sides that end in `join`, each
/// side an empty block of its own or `join` itself, and the way each side
/// reaches it.
fn sides(
    function: &Function,
    block: BlockId,
) -> Option<Sides> {
    let last = function.terminator(block)?;
    let instruction = function.instruction(last);
    let [condition, Operand::Block(yes), Operand::Block(no)] = instruction.operands[..] else { return None };
    if instruction.opcode != Opcode::Br || yes == no {
        return None;
    }
    // Where an empty block of one jump, reached from `block` alone, leads.
    let empty = |side: BlockId| {
        let [only] = function.block(side).instructions()[..] else { return None };
        let jump = function.instruction(only);
        let [Operand::Block(to)] = jump.operands[..] else { return None };
        (jump.opcode == Opcode::Br && function.predecessors(side) == [block]).then_some(to)
    };
    let (join, diamond) = match (empty(yes), empty(no)) {
        (Some(a), Some(b)) if a == b => (a, true),
        (Some(a), None) if a == no => (a, false),
        (None, Some(b)) if b == yes => (b, false),
        _ => return None,
    };
    if join == block {
        return None;
    }
    // Each side as the block a phi of `join` names for it.
    let from = |side: BlockId| if side == join { block } else { side };
    let mut preds = function.predecessors(join);
    preds.sort();
    let mut expected = vec![from(yes), from(no)];
    expected.sort();
    (preds == expected).then_some(Sides { condition, join, yes: from(yes), no: from(no), diamond })
}

fn planned(
    context: &mut Context,
    function: &Function,
    costs: &OperationCosts,
    scaled: Option<&dyn Machine>,
) -> Option<Plan> {
    for block in function.layout().to_vec() {
        let Some(Sides { condition, join, yes, no, diamond }) = sides(function, block) else { continue };
        let phis = edges::phis(function, join);
        if phis.is_empty() {
            continue;
        }
        let mut rewrites = Vec::new();
        let (mut after, mut before) = (costs.set, costs.branch + if diamond { costs.branch / 2 } else { 0 });
        for &phi in &phis {
            let Some((rewrite, cost)) = rewritten(context, function, costs, scaled, phi, yes, no) else {
                rewrites.clear();
                break;
            };
            after += cost;
            before += costs.r#move;
            rewrites.push(rewrite);
        }
        if !rewrites.is_empty() && after <= before {
            return Some(Plan { block, join, condition, rewrites });
        }
    }
    None
}

/// `phi`, whose constants from `yes` and `no` are known, as the cheapest form,
/// and what it costs.
fn rewritten(
    context: &mut Context,
    function: &Function,
    costs: &OperationCosts,
    scaled: Option<&dyn Machine>,
    phi: InstId,
    yes: BlockId,
    no: BlockId,
) -> Option<(Rewrite, i64)> {
    let ty = function.instruction(phi).ty;
    let width = context.types.int_bits(ty).filter(|&bits| bits > 1)?;
    let incoming = arms(function, phi);
    let [(a, from_a), (b, from_b)] = incoming[..] else { return None };
    let constant = |operand: Operand| match operand {
        Operand::Constant(id) => match context.get(id).kind {
            ConstantKind::Int(bits) => Some(bits),
            _ => None,
        },
        _ => None,
    };
    let (value_yes, value_no) = if (from_a, from_b) == (yes, no) {
        (constant(a)?, constant(b)?)
    } else if (from_a, from_b) == (no, yes) {
        (constant(b)?, constant(a)?)
    } else {
        return None;
    };
    let otherwise = signed(value_no, width);
    let difference = signed(value_yes.wrapping_sub(value_no) & mask(width), width);
    let kept = i64::from(otherwise != 0) * costs.add;
    let candidates = [
        (difference == 1).then(|| (Form::Step { sign: false }, costs.extend + kept)),
        (difference == -1).then(|| (Form::Step { sign: true }, costs.extend + kept)),
        (difference > 1 && difference & (difference - 1) == 0).then(|| {
            (Form::Shifted { shift: difference.trailing_zeros() }, costs.extend + costs.shift + kept)
        }),
        scaled.filter(|_| difference > 1).map(|target| {
            let factor = i64::try_from(difference).unwrap_or(i64::MAX);
            (Form::Scaled { difference }, costs.extend + target.multiply_by(factor) + kept)
        }),
        Some((Form::Masked { difference }, costs.extend + 2 * costs.add + kept)),
    ];
    let (form, cost) = candidates.into_iter().flatten().min_by_key(|&(_, cost)| cost)?;
    Some((Rewrite { phi, ty, otherwise, form }, cost))
}

fn applied(
    context: &mut Context,
    function: &mut Function,
    plan: Plan,
) {
    let last = function.terminator(plan.block).expect("a terminator");
    let void = function.instruction(last).ty;
    let at = Position::Before(last);
    for Rewrite { phi, ty, otherwise, form } in plan.rewrites {
        let mut number = |function: &mut Function, op: BinaryOp, left: Operand, right: i128| {
            let right = Operand::Constant(context.int(ty, right));
            placed(function, Opcode::Binary(op), ty, vec![left, right], at)
        };
        let extended = |function: &mut Function, kind: CastOp| {
            placed(function, Opcode::Cast(kind), ty, vec![plan.condition], at)
        };
        let result = match form {
            Form::Step { sign } => {
                let one = extended(function, if sign { CastOp::SExt } else { CastOp::ZExt });
                if otherwise == 0 { one } else { number(function, BinaryOp::Add, one, otherwise) }
            }
            Form::Shifted { shift } => {
                let one = extended(function, CastOp::ZExt);
                let moved = number(function, BinaryOp::Shl, one, i128::from(shift));
                if otherwise == 0 { moved } else { number(function, BinaryOp::Add, moved, otherwise) }
            }
            Form::Scaled { difference } => {
                let one = extended(function, CastOp::ZExt);
                let scaled = number(function, BinaryOp::Mul, one, difference);
                if otherwise == 0 { scaled } else { number(function, BinaryOp::Add, scaled, otherwise) }
            }
            Form::Masked { difference } => {
                let all = extended(function, CastOp::SExt);
                let masked = number(function, BinaryOp::And, all, difference);
                if otherwise == 0 { masked } else { number(function, BinaryOp::Add, masked, otherwise) }
            }
        };
        let value = function.instruction(phi).result.expect("a phi's value");
        function.replace_value(value, result);
        function.erase(phi).expect("a replaced phi");
    }
    let jump =
        function.create_instruction(Opcode::Br, void, vec![Operand::Block(plan.join)], Flags::default(), None);
    function.insert(jump, at).expect("a placed terminator");
    function.erase(last).expect("a terminator defines nothing");
}

fn placed(
    function: &mut Function,
    opcode: Opcode,
    ty: TypeId,
    operands: Vec<Operand>,
    at: Position,
) -> Operand {
    let inst = function.create_instruction(opcode, ty, operands, Flags::default(), None);
    function.insert(inst, at).expect("a placed position");
    Operand::Value(function.instruction(inst).result.expect("a value"))
}

#[cfg(test)]
#[path = "phiopt_tests.rs"]
mod tests;
