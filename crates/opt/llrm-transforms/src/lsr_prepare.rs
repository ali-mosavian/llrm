//! What LLVM's loop strength reduction does to a loop before it collects
//! (`OptimizeMax`, `OptimizeLoopTermCond` and `FindIVUserForCond`;
//! LoopStrengthReduce.cpp at `d1106deb71cc`, l.2485-2815).
//!
//! # How it works
//! An exit test is changed to take the post-incremented value of its induction
//! variable, so the variable and its increment share one register. For each
//! exiting block that ends in a compare of an IV user: a `max` the trip count
//! was written with (`i != smax(n, 1)`) is undone to `i < n`; the test is moved
//! beside its branch (copied if it has other users); and the use is marked
//! post-increment, unless another use of a stride that divides this one could
//! share the pre-increment value. [`analyze`] decides from the expressions;
//! [`apply`] edits the function and says which tests it marked.
//!
//! # Example
//! `do { p[i] = 0; } while (++i != n)`: the test `%next == %n` is on `%next`,
//! the incremented value, and moves to just before the latch's branch.
//!
//! # Runs
//! Before the collection, from [`crate::lsr_pass`].
//!
//! # Limits
//! DEVIATE: `OptimizeShadowIV` and a vector condition (`extractelement`) are
//! not ported.

use llrm_analysis::scev::backedge::ExitCountKind;
use llrm_analysis::scev::expr::{Kind, Use as ScevUse};
use llrm_analysis::scev::{Cx, Flags as ScevFlags, ScalarEvolution};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::tti::Tti;

use crate::ivusers::IvUsers;
use crate::lsr_collect::{get_access_type, get_exact_sdiv, is_address_use};

/// A `max` in the bound of an exit test, undone: the test becomes `pred` of the
/// IV and `rhs`, and the select (and its compare) go.
#[derive(Clone, Debug)]
pub struct MaxEdit {
    pub cond: InstId,
    pub select: InstId,
    pub predicate: IntPredicate,
    pub rhs: Operand,
}

/// What to do for one exiting block.
#[derive(Clone, Debug)]
pub struct ExitEdit {
    pub exiting: BlockId,
    pub max: Option<MaxEdit>,
    /// Whether the test takes the post-incremented value.
    pub post_inc: bool,
}

/// `OptimizeMax`: the edit that undoes the max in `cond`'s bound, if it has
/// one of the shape.
fn optimize_max(
    se: &mut ScalarEvolution,
    cx: &Cx,
    l: BlockId,
    cond: InstId,
) -> Option<MaxEdit> {
    let function = cx.function;
    let instruction = function.instruction(cond);
    let Opcode::ICmp(pred) = instruction.opcode else { return None };
    if !matches!(pred, IntPredicate::Eq | IntPredicate::Ne) {
        return None;
    }
    let Operand::Value(sel_value) = instruction.operands[1] else { return None };
    let ValueDef::Instruction(select) = function.value(sel_value).def else { return None };
    if function.instruction(select).opcode != Opcode::Select || !function.users(sel_value).has_one_use() {
        return None;
    }
    let btc = se.get_backedge_taken_count(cx, l, ExitCountKind::Exact);
    if se.is_could_not_compute(btc) {
        return None;
    }
    let one = se.get_constant_of_type(cx, se.type_of(btc), 1);
    // The trip count is the backedge-taken count plus one.
    let iteration_count = se.get_add_expr(cx, vec![ScevUse::of(one), ScevUse::of(btc)], ScevFlags::default(), 0).scev;
    if iteration_count != se.get_scev(cx, Operand::Value(sel_value)) {
        return None;
    }
    // A max that matches. There is no ULE: it would be against zero.
    let (mut new_pred, max_ops) = match (se.node(btc).kind.clone(), se.node(iteration_count).kind.clone()) {
        (Kind::SMax(ops), _) => (IntPredicate::Sle, ops),
        (_, Kind::SMax(ops)) => (IntPredicate::Slt, ops),
        (_, Kind::UMax(ops)) => (IntPredicate::Ult, ops),
        _ => return None,
    };
    // More than two operands would need more checking and setup.
    if max_ops.len() != 2 {
        return None;
    }
    let (max_lhs, max_rhs) = (max_ops[0].scev, max_ops[1].scev);
    // Constants are on the left: for < and > one, for <= and >= zero.
    let true_when_equal = matches!(new_pred, IntPredicate::Sle);
    let wrong_lhs = if true_when_equal { !se.match_zero(max_lhs) } else { max_lhs != one };
    if wrong_lhs {
        return None;
    }
    // The IV is `{1,+,1}`.
    let iv = se.get_scev(cx, instruction.operands[0]);
    let (start, step, _) = se.match_affine_add_rec(iv)?;
    if se.as_constant(start).map(|c| c.sext_value()) != Some(1) || se.as_constant(step).map(|c| c.sext_value()) != Some(1)
    {
        return None;
    }
    // The right operand of the select, which the new compare takes.
    let sel = function.instruction(select);
    let is_one = |operand: Operand| matches!(operand, Operand::Constant(c) if cx.context.get(c).kind == llrm_mir::context::ConstantKind::Int(1));
    let plus_one_of = |se: &mut ScalarEvolution, operand: Operand| -> Option<Operand> {
        let Operand::Value(v) = operand else { return None };
        let ValueDef::Instruction(def) = function.value(v).def else { return None };
        let add = function.instruction(def);
        if add.opcode != Opcode::Binary(BinaryOp::Add) || !is_one(add.operands[1]) {
            return None;
        }
        (se.get_scev(cx, add.operands[0]) == max_rhs).then_some(add.operands[0])
    };
    let rhs = if true_when_equal {
        // `n + 1`, and `n`.
        plus_one_of(se, sel.operands[1]).or_else(|| plus_one_of(se, sel.operands[2]))?
    } else if se.get_scev(cx, sel.operands[1]) == max_rhs {
        sel.operands[1]
    } else if se.get_scev(cx, sel.operands[2]) == max_rhs {
        sel.operands[2]
    } else {
        se.unknown_value(max_rhs)?
    };
    if pred == IntPredicate::Eq {
        new_pred = new_pred.inverse();
    }
    Some(MaxEdit { cond, select, predicate: new_pred, rhs })
}

/// `OptimizeLoopTermCond`, read-only: the edit for each exiting block that
/// has one. None where the backedge is not an exit (a head-tested loop).
pub fn analyze(
    se: &mut ScalarEvolution,
    cx: &Cx,
    tti: &dyn Tti,
    l: BlockId,
    users: &IvUsers,
) -> Vec<ExitEdit> {
    let function = cx.function;
    let lp = cx.loops.get_loop_for(l).expect("a loop");
    let latch = lp.get_loop_latch(function).expect("a latch");
    let exiting_blocks = lp.get_exiting_blocks(function);
    // A backedge that does not exit: a head-tested loop.
    if !exiting_blocks.contains(&latch) {
        return Vec::new();
    }
    let mut edits = Vec::new();
    for exiting in exiting_blocks {
        let Some(term) = function.terminator(exiting) else { continue };
        let branch = function.instruction(term);
        if branch.opcode != Opcode::Br || branch.operands.len() != 3 {
            continue;
        }
        let Operand::Value(cond_value) = branch.operands[0] else { continue };
        let ValueDef::Instruction(cond) = function.value(cond_value).def else { continue };
        // The IV use of the condition, if it has one.
        let Some(cond_idx) = users.uses.iter().position(|u| u.user == cond) else { continue };
        // A trip count written as a max: undone first.
        let max = optimize_max(se, cx, l, cond);
        // An exiting block that dominates the latch may use the post-inc value
        // if it is not shared with other uses.
        if !cx.tree.dominates(exiting, latch) {
            edits.push(ExitEdit { exiting, max, post_inc: false });
            continue;
        }
        // In a non-latch exit, there may be pre-inc users in the blocks between.
        let mut declined = false;
        if latch != exiting {
            for (idx, other) in users.uses.iter().enumerate() {
                if idx == cond_idx {
                    continue;
                }
                let block = function.parent(other.user).expect("a placed instruction");
                if cx.tree.properly_dominates(block, exiting) {
                    continue;
                }
                // The strides might share: a quotient that could be a legal scale.
                let (Some(mut a), Some(mut b)) =
                    (IvUsers::stride(se, cx, &users.uses[cond_idx], l), IvUsers::stride(se, cx, other, l))
                else {
                    continue;
                };
                let (abits, bbits) =
                    (se.get_type_size_in_bits(cx, se.type_of(a)), se.get_type_size_in_bits(cx, se.type_of(b)));
                if abits != bbits {
                    if abits > bbits {
                        b = se.get_sign_extend_expr(cx, ScevUse::of(b), se.type_of(a), 0);
                    } else {
                        a = se.get_sign_extend_expr(cx, ScevUse::of(a), se.type_of(b), 0);
                    }
                }
                let Some(d) = get_exact_sdiv(se, cx, b, a, false).and_then(|s| se.as_constant(s)) else { continue };
                // A stride of one or minus one can be reused by non-addresses.
                if d.is_one() || d.is_all_ones() {
                    declined = true;
                    break;
                }
                // Weird situations.
                if d.significant_bits() >= 64 || d.is_signed_min() {
                    declined = true;
                    break;
                }
                // A possible scaled-address reuse.
                if is_address_use(cx, other.user, other.operand) {
                    let _access = get_access_type(cx, other.user, other.operand);
                    let scale = d.sext_value() as i64;
                    if tti.is_legal_addressing_mode(false, 0, true, scale)
                        || tti.is_legal_addressing_mode(false, 0, true, scale.wrapping_neg())
                    {
                        declined = true;
                        break;
                    }
                }
            }
        }
        edits.push(ExitEdit { exiting, max, post_inc: !declined });
    }
    edits
}

/// The edits made: the exit tests that take the post-incremented value.
pub fn apply(
    function: &mut Function,
    types: &llrm_mir::types::Types,
    header: BlockId,
    edits: &[ExitEdit],
) -> Vec<InstId> {
    let mut marked = Vec::new();
    for edit in edits {
        // `OptimizeMax`: the compare is made again, without the select.
        if let Some(max) = &edit.max {
            let old = function.instruction(max.cond).clone();
            let i1 = types.int(1);
            let new = function.create_instruction(
                Opcode::ICmp(max.predicate),
                i1,
                vec![old.operands[0], max.rhs],
                Flags::default(),
                Some("scmp"),
            );
            function.insert(new, Position::Before(max.cond)).expect("a placed compare");
            let (old_value, new_value) = (
                function.instruction(max.cond).result.expect("a compare value"),
                function.instruction(new).result.expect("a compare value"),
            );
            function.replace_all_uses_with(old_value, Operand::Value(new_value));
            let sel_cmp = match function.instruction(max.select).operands[0] {
                Operand::Value(v) => match function.value(v).def {
                    ValueDef::Instruction(def) => Some(def),
                    ValueDef::Argument(_) => None,
                },
                _ => None,
            };
            function.erase(max.cond).expect("the old compare");
            function.erase(max.select).expect("the select");
            if let Some(cmp) = sel_cmp
                && function.instruction(cmp).result.is_some_and(|v| function.users(v).is_empty())
            {
                function.erase(cmp).expect("the select's compare");
            }
        }
        if !edit.post_inc {
            continue;
        }
        let term = function.terminator(edit.exiting).expect("a terminator");
        let Operand::Value(cond_value) = function.instruction(term).operands[0] else { continue };
        let ValueDef::Instruction(mut cond) = function.value(cond_value).def else { continue };
        // The compare may be anywhere in the loop, with other users: it goes
        // beside the branch, or a copy of it does.
        if matches!(function.instruction(cond).opcode, Opcode::ICmp(_) | Opcode::FCmp(_))
            && function.next_in_block(cond) != Some(term)
        {
            if function.users(cond_value).has_one_use() {
                function.move_to(cond, Position::Before(term)).expect("a placed compare");
            } else {
                let copy = function.clone_instruction(cond);
                function.insert(copy, Position::Before(term)).expect("a placed copy");
                let copy_value = function.instruction(copy).result.expect("a compare value");
                function.set_operand(term, 0, Operand::Value(copy_value));
                let _ = header;
                cond = copy;
            }
        }
        marked.push(cond);
    }
    marked
}
