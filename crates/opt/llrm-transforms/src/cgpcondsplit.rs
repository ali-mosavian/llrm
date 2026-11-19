//! A branch on `a || b` (or `a && b`) whose `b` needs dear work only it reads,
//! made as two branches with that work after the first.
//!
//! Pass: `CodeGenPrepare::splitBranchCondition`, with the `Sink.cpp` rule for
//! what moves: a single-use instruction, and a load nothing between it and the
//! split can write over.
//!
//! ```text
//!  %b = <dear cone>                     br %a, %t, %side
//!  %c = select %a, true, %b       side: %b = <dear cone>
//!  br %c, %t, %f                        br %b, %t, %f
//! ```
//!
//! Applied where the cone costs more than twice a branch in the target's
//! prices: on x87 `fld`, `fadd` and `fstp` are 18 clocks against 3 for the
//! branch, paid on every trip though `a` decides one in 25.
//!
//! DEVIATE CB1 (owner x87-b; exit 2026-11-24: the pin's DAG keeps the cone in
//! the first block, and `splitBranchCondition` takes only an `or`/`and`
//! instruction): `SimplifyCFG`'s `FoldBranchToCommonDest` made the select
//! from two branches, and the cone, made before the first branch in the
//! source, stayed in front of both. gcc's `tree-ssa-sink` moves it behind the
//! first test (sfm16_dbg -O1: 400 `fadd` against 384, +291 clocks).
//!
//! # Example
//! A branch on `select %a, true, %b` whose `%b` is a dear cone only it reads
//! becomes two branches, the cone after the first.
//!
//! ```text
//!  %b = <dear cone>                     br %a, %t, %side
//!  %c = select %a, true, %b       side: %b = <dear cone>
//!  br %c, %t, %f                        br %b, %t, %f
//! ```

use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};
use llrm_mir::passes::{self, Analyses, FunctionPass, PreservedAnalyses};
use llrm_mir::types::Type;

use crate::edges::predecessor_added;

/// What the cone and the branch cost on the target, as `OperationCosts` states.
#[derive(Clone, Copy, Debug)]
pub struct Prices {
    pub float_add: i64,
    pub float_multiply: i64,
    pub float_divide: i64,
    pub load: i64,
    pub float_load: i64,
    pub branch: i64,
}

/// Every branch on a logical `or`/`and` whose second operand's cone is dear,
/// split. Whether any was.
pub fn conditions_split(
    context: &Context,
    function: &mut Function,
    prices: Prices,
) -> bool {
    if prices.branch <= 0 {
        return false;
    }
    let mut changed = false;
    // A split block is one the scan has not seen: its own branch is a plain
    // one, so a second round finds no more than the first left.
    while let Some(site) = find(context, function, prices) {
        split(function, site);
        changed = true;
    }
    changed
}

struct Site {
    home: BlockId,
    branch: InstId,
    select: InstId,
    first: Operand,
    second: Operand,
    cone: Vec<InstId>,
    or: bool,
}

fn find(
    context: &Context,
    function: &Function,
    prices: Prices,
) -> Option<Site> {
    for &home in function.layout() {
        let Some(branch) = function.terminator(home) else { continue };
        if function.instruction(branch).opcode != Opcode::Br {
            continue;
        }
        let [Operand::Value(condition), Operand::Block(on_true), Operand::Block(on_false)] =
            function.instruction(branch).operands[..]
        else {
            continue;
        };
        let ValueDef::Instruction(select) = function.value(condition).def else { continue };
        if on_true == on_false
            || function.instruction(select).opcode != Opcode::Select
            || function.parent(select) != Some(home)
            || !function.users(condition).has_one_use()
        {
            continue;
        }
        let [first, yes, no] = function.instruction(select).operands[..] else { continue };
        let is = |operand: Operand, bit: u128| {
            matches!(
                operand,
                Operand::Constant(id) if context.get(id).kind == ConstantKind::Int(bit)
            )
        };
        let (second, or) = match (is(yes, 1), is(no, 0)) {
            (true, _) => (no, true),
            (_, true) => (yes, false),
            _ => continue,
        };
        let Operand::Value(value) = second else { continue };
        let ValueDef::Instruction(root) = function.value(value).def else { continue };
        if function.parent(root) != Some(home) || !function.users(value).has_one_use() {
            continue;
        }
        let cone = cone_of(function, home, root);
        if cost(context, function, &cone, prices) > 2 * prices.branch {
            return Some(Site { home, branch, select, first, second, cone, or });
        }
    }
    None
}

/// What the cone costs: each float operation and load at its price.
fn cost(
    context: &Context,
    function: &Function,
    cone: &[InstId],
    prices: Prices,
) -> i64 {
    cone.iter()
        .map(|&inst| {
            let one = function.instruction(inst);
            match one.opcode {
                Opcode::Binary(BinaryOp::FAdd | BinaryOp::FSub) => prices.float_add,
                Opcode::Binary(BinaryOp::FMul) => prices.float_multiply,
                Opcode::Binary(BinaryOp::FDiv) => prices.float_divide,
                Opcode::Load { .. } if matches!(context.types.get(one.ty), Type::Float(_)) => prices.float_load,
                Opcode::Load { .. } => prices.load,
                _ => 0,
            }
        })
        .sum()
}

/// `root` and what only it reads, in block order: a single-use instruction
/// that is pure, or a load no store or call after it in the block can write
/// over (`Sink.cpp`'s `SawStore`). What cannot move stays, and the cone reads
/// it from the first block.
fn cone_of(
    function: &Function,
    home: BlockId,
    root: InstId,
) -> Vec<InstId> {
    let order: Vec<InstId> = function.iter_block(home).collect();
    let position = |inst: InstId| order.iter().position(|&other| other == inst);
    let writes = |inst: InstId| {
        matches!(
            function.instruction(inst).opcode,
            Opcode::Store { .. } | Opcode::Call(_) | Opcode::Invoke(_)
        )
    };
    let movable = |inst: InstId| {
        let one = function.instruction(inst);
        match one.opcode {
            Opcode::Load { volatile, .. } => {
                !volatile && position(inst).is_some_and(|at| !order[at + 1..].iter().any(|&later| writes(later)))
            }
            Opcode::Cast(_) | Opcode::ICmp(_) | Opcode::FCmp(_) | Opcode::Select | Opcode::GetElementPtr { .. } => true,
            Opcode::Binary(op) => {
                !matches!(
                    op,
                    BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::URem | BinaryOp::SRem | BinaryOp::FRem
                )
            }
            _ => false,
        }
    };
    if !movable(root) {
        return Vec::new();
    }
    let mut found = vec![root];
    let mut at = 0;
    while at < found.len() {
        for operand in function.instruction(found[at]).operands.clone() {
            let Operand::Value(value) = operand else { continue };
            let ValueDef::Instruction(def) = function.value(value).def else { continue };
            if function.parent(def) == Some(home)
                && function.users(value).has_one_use()
                && movable(def)
                && !found.contains(&def)
            {
                found.push(def);
            }
        }
        at += 1;
    }
    order.into_iter().filter(|inst| found.contains(inst)).collect()
}

fn split(
    function: &mut Function,
    site: Site,
) {
    let Site { home, branch, select, first, second, cone, or } = site;
    let [_, Operand::Block(on_true), Operand::Block(on_false)] = function.instruction(branch).operands[..] else {
        unreachable!("a conditional branch")
    };
    let void = function.instruction(branch).ty;
    let side = function.create_block(None);
    function.insert_block(side, Some(home)).expect("a new block");
    for moved in cone {
        function.move_to(moved, Position::End(side)).expect("a placed instruction");
    }
    // `a` decides one way: that edge leaves `home`; the other goes to `side`,
    // which decides on `b`.
    let (kept, rest) = if or { (on_true, on_false) } else { (on_false, on_true) };
    let (home_true, home_false) = if or { (on_true, side) } else { (side, on_false) };
    function.set_operand(branch, 0, first);
    function.set_operand(branch, 1, Operand::Block(home_true));
    function.set_operand(branch, 2, Operand::Block(home_false));
    let again = function.create_instruction(
        Opcode::Br,
        void,
        vec![second, Operand::Block(on_true), Operand::Block(on_false)],
        Flags::default(),
        None,
    );
    function.insert(again, Position::End(side)).expect("an unplaced instruction");
    // The block `a` decided now has two predecessors, the other one a new one.
    predecessor_added(function, kept, side, home);
    for phi in crate::edges::phis(function, rest) {
        let operands = function.instruction(phi).operands.clone();
        for (at, operand) in operands.iter().enumerate() {
            if *operand == Operand::Block(home) {
                function.set_operand(phi, at, Operand::Block(side));
            }
        }
    }
    function.erase(select).expect("nothing reads the select now");
}

/// [`conditions_split`] as a pass, after `cgpaddr`, at every level that prices
/// clocks (not -Os).
pub struct CondSplit;

impl FunctionPass for CondSplit {
    fn name(&self) -> &'static str {
        "cgpcondsplit"
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let costs = analyses.outer().target().costs();
        let prices = Prices {
            float_add: costs.float_add,
            float_multiply: costs.float_multiply,
            float_divide: costs.float_divide,
            load: costs.load,
            float_load: costs.float_load,
            branch: costs.branch,
        };
        if conditions_split(unit.context, unit.function, prices) {
            PreservedAnalyses::none()
        } else {
            PreservedAnalyses::all()
        }
    }
}

#[cfg(test)]
#[path = "cgpcondsplit_tests.rs"]
mod tests;
