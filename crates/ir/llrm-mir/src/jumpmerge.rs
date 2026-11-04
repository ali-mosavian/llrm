//! Whether a branch on `a && b` or `a || b` of compares stays one condition
//! (a `cmp` and a `ccmp` or `setcc`s, then one branch) or splits into a branch
//! each: SelectionDAG's `shouldKeepJumpConditionsTogether`.
//!
//! # How it works
//! The target gives a budget (`getJumpConditionMergingParams`, [`Tti`]
//! `jump_condition_merging_params`). The right operand's instructions that the
//! left does not already need, and nothing outside the right uses, are priced
//! by latency; the condition stays together while they add up to no more than
//! the budget. A branch with a likely early-out edge (`and` toward the false
//! edge, `or` toward the true one is the case that needs both sides, the other
//! is the early out) is adjusted by the target's biases.
//!
//! Two users: the DAG builder (`llrm-codegen`) and the arm64 selector, which
//! has no DAG. The target's numbers are the target's: this holds the rule.
//!
//! DEVIATE 1: the edge probabilities come from the caller as `likely`, a
//! closure that is asked only when a bias is set.

use crate::datalayout::DataLayout;
use crate::instructioncost::instruction_cost;
use crate::module::{Function, GlobalValue, InstId, Operand, ValueDef};
use crate::opcode::Opcode;
use crate::tti::{Cost, CostKind, Tti};

/// A logical `and` or `or`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Logic {
    And,
    Or,
}

/// `TargetLoweringBase::CondMergingParams`: the budget, and its change where
/// the early-out edge is likely (`likely_bias`) or the merged side is
/// (`unlikely_bias`). A negative `base_cost` always splits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CondMerging {
    pub base_cost: i64,
    pub likely_bias: i64,
    pub unlikely_bias: i64,
}

impl CondMerging {
    /// `{-1, -1, -1}`: never keep together.
    pub const SPLIT: CondMerging = CondMerging { base_cost: -1, likely_bias: -1, unlikely_bias: -1 };
}

/// What `jump_condition_merging_params` is asked about.
pub struct JumpMergeQuery<'a> {
    pub context: &'a crate::context::Context,
    pub function: &'a Function,
    pub logic: Logic,
    pub lhs: Operand,
    pub rhs: Operand,
}

/// `collectInstructionDeps`: the instructions `operand` depends on, in the
/// order found, to the pin's recursion depth. False where the count is
/// incomplete.
fn collect_deps(
    function: &Function,
    deps: &mut Vec<InstId>,
    operand: Operand,
    necessary: Option<&Vec<InstId>>,
    depth: usize,
) -> bool {
    // `SelectionDAG::MaxRecursionDepth`
    if depth >= 6 {
        return false;
    }
    let Operand::Value(value) = operand else { return true };
    let ValueDef::Instruction(inst) = function.value(value).def else { return true };
    if necessary.is_some_and(|necessary| necessary.contains(&inst)) || deps.contains(&inst) {
        return true;
    }
    deps.push(inst);
    for next in function.instruction(inst).operands.iter() {
        if !collect_deps(function, deps, *next, necessary, depth + 1) {
            return false;
        }
    }
    // An `alloca` of one element keeps no count operand here; the pin's has
    // one (`i32 1`), which one level down is the depth that ends the count.
    if matches!(function.instruction(inst).opcode, Opcode::Alloca { .. })
        && function.instruction(inst).operands.is_empty()
        && depth + 1 >= 6
    {
        return false;
    }
    true
}

/// `shouldKeepJumpConditionsTogether` of the branch `branch` whose condition
/// `cond_inst` joins `lhs` and `rhs`.
#[allow(clippy::too_many_arguments)]
pub fn should_keep_together(
    tti: &dyn Tti,
    context: &crate::context::Context,
    layout: &DataLayout,
    globals: &[GlobalValue],
    function: &Function,
    cond_inst: InstId,
    logic: Logic,
    (lhs, rhs): (Operand, Operand),
    params: CondMerging,
    likely: &mut dyn FnMut() -> Option<bool>,
) -> bool {
    if params.base_cost < 0 {
        return false;
    }
    let mut thresh = params.base_cost;
    let biased = params.likely_bias != 0 || params.unlikely_bias != 0;
    if biased && let Some(likely) = likely() {
        if logic == if likely { Logic::And } else { Logic::Or } {
            // Both sides are likely to be computed.
            thresh += params.likely_bias;
        } else {
            // An early out is likely.
            if params.unlikely_bias < 0 {
                return false;
            }
            thresh -= params.unlikely_bias;
        }
    }
    if thresh <= 0 {
        return false;
    }
    let defining = |operand: Operand| match operand {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(inst) => Some(inst),
            _ => None,
        },
        _ => None,
    };
    // What the left side needs, and what only the right adds.
    let mut left: Vec<InstId> = Vec::new();
    collect_deps(function, &mut left, lhs, None, 0);
    let mut right: Vec<InstId> = Vec::new();
    if !collect_deps(function, &mut right, rhs, Some(&left), 0) {
        return false;
    }
    if let Some(inst) = defining(rhs)
        && !left.contains(&inst)
        && !right.contains(&inst)
    {
        right.push(inst);
    }
    // Prune what unrelated instructions also use.
    for _ in 0..6 {
        let drop = right
            .iter()
            .position(
                |inst| {
                    let Some(result) = function.instruction(*inst).result else { return false };
                    function.users(result).iter().any(|one| one.user != cond_inst && !right.contains(&one.user))
                },
            );
        match drop {
            Some(at) => {
                right.remove(at);
            }
            None => break,
        }
    }
    let mut total = Cost::FREE;
    for inst in &right {
        total = total + instruction_cost(tti, context, layout, globals, function, *inst, CostKind::Latency);
        if !total.valid || total.value > thresh {
            return false;
        }
    }
    true
}
