//! A phi of two constants that a branch chooses between becomes arithmetic of
//! the branch's condition, where that costs no more than the branch.
//!
//! # How it works
//! The shape is `br c, T, F` where each of `T` and `F` is an empty block of
//! one jump (reached from the branch alone) or the join `J` itself, both
//! leading to `J`, whose predecessors are exactly those two sides. Every phi
//! of `J` must take a constant integer (wider than one bit) from each side;
//! otherwise the branch is left whole.
//!
//! Each phi is rewritten in the form that costs least on the target. With
//! `t` the constant from `T`, `f` the constant from `F` and `d = t - f`:
//!
//! | condition on `d` | form | cost |
//! |---|---|---|
//! | `d = 1` | `f + zext c` | extend |
//! | `d = -1` | `f + sext c` | extend |
//! | `d` a power of two above 1 | `(zext c << k) + f` | extend + shift |
//! | `d > 1`, time-priced | `zext c * d + f` | extend + multiply by `d` |
//! | any | `(sext c & d) + f` | extend + 2 adds |
//!
//! The `+ f` term is dropped when `f` is 0, else it costs an add. The sum of
//! the forms and one condition held as a value (`set`) is compared with the
//! branch it replaces (`OperationCosts::branch`, plus half again for a diamond,
//! the jump from the arm that is not the fall-through) and one copy (`move`)
//! per phi. The rewrite is made when it is no dearer; all phis of a join
//! go together or none. The multiply form is offered only where prices are
//! in time: at `-Os` the pass is priced in bytes (`size_costs`) and the
//! multiply is not offered.
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   b["<b>b</b><br/><small>br c, T, F</small>"]:::mid
//!   t["<b>T</b><br/><small>br J</small>"]:::mid
//!   f["<b>F</b><br/><small>br J</small>"]:::mid
//!   j["<b>J</b><br/><small>p = phi [4, T] [0, F]</small>"]:::mid
//!   b -->|"c"| t
//!   b -->|"not c"| f
//!   t --> j
//!   f --> j
//! ```
//!
//! *Before: a branch chooses between two constants*
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   b["<b>b</b><br/><small>x = zext c to i16<br/>y = shl x, 2<br/>p is y</small>"]:::mid
//!   j["<b>J</b><br/><small>the uses of p</small>"]:::mid
//!   b --> j
//! ```
//!
//! *After: arithmetic of the condition, no branch*
//!
//! The 486 has no conditional move, so a select is a branch anyway (the
//! backend's `selects`); this pass is for what the branch is worth when it
//! is not one. `can_win` skips the walk of a body when no form can cost no
//! more than the loosest branch on the target.
//!
//! # Example
//! Before:
//! ```mir
//! define ... @pipeline.body(i16 %0) ... {
//! b1:
//!   %1 = icmp ne i16 %0, 0
//!   br i1 %1, label %b3, label %b2
//!
//! b2:
//!   br label %b4
//!
//! b3:
//!   br label %b4
//!
//! b4:
//!   %2 = phi i16 [ 0, %b2 ], [ 4, %b3 ]
//!   ret i16 %2
//! }
//! ```
//! After:
//! ```mir
//! define ... @pipeline.body(i16 %0) ... {
//! b1:
//!   %1 = icmp ne i16 %0, 0
//!   %2 = zext i1 %1 to i16
//!   %3 = shl i16 %2, 2
//!   ret i16 %3
//! }
//! ```
//! The phi of `4` and `0` is `c != 0` extended and shifted left by 2, with no
//! branch.
//!
//! From `--dump` of `pick` (`if (c) x = 4; else x = 0; return x;`) at -O2 -m16
//! -march=i386, stages `dead` and `phiopt`.
//!
//! # Runs
//! In [`crate::pipeline`]'s per-body fixed point, after `decide`. On at every
//! level above -O0, as gcc runs `-fssa-phiopt` at -O1 and above. After a
//! rewrite the empty sides are removed and blocks merged. The measured effect
//! on the 486 and P5 is recorded in `docs/reference/levels.md`.
//!
//! # Needs
//! The target's `OperationCosts` (`branch`, `move`, `extend`, `set`, `add`,
//! `shift`) and `Machine::multiply_by`.
//!
//! # See also
//! gcc's `pass_phiopt` (`conditional_replacement`, `two_value_replacement`,
//! and `match.pd`'s `cond ? c : 0` as `-cond & c`), enabled at -O1 by
//! [`opts.cc:609`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/opts.cc#L609);
//! LLVM's `FoldTwoEntryPHINode` in SimplifyCFG, whose
//! `two-entry-phi-node-folding-threshold` (4) is the same test in its own unit.

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
        let mut body = function.iter_block(side);
        let (Some(only), None) = (body.next(), body.next()) else { return None };
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
    let mut preds: Vec<BlockId> = function.predecessors(join).into();
    preds.sort();
    let mut expected = vec![from(yes), from(no)];
    expected.sort();
    (preds == expected).then_some(Sides { condition, join, yes: from(yes), no: from(no), diamond })
}

fn planned(
    context: &Context,
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
    context: &Context,
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
        (difference > 1 && difference & (difference - 1) == 0)
            .then(|| (Form::Shifted { shift: difference.trailing_zeros() }, costs.extend + costs.shift + kept)),
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
    context: &Context,
    function: &mut Function,
    plan: Plan,
) {
    let last = function.terminator(plan.block).expect("a terminator");
    let void = function.instruction(last).ty;
    let at = Position::Before(last);
    for Rewrite { phi, ty, otherwise, form } in plan.rewrites {
        let number = |function: &mut Function, op: BinaryOp, left: Operand, right: i128| {
            let right = Operand::Constant(context.int(ty, right));
            placed(function, Opcode::Binary(op), ty, vec![left, right], at)
        };
        let extended =
            |function: &mut Function, kind: CastOp| placed(function, Opcode::Cast(kind), ty, vec![plan.condition], at);
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
    let jump = function.create_instruction(Opcode::Br, void, vec![Operand::Block(plan.join)], Flags::default(), None);
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
