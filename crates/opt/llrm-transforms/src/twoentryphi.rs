//! A two-entry phi that a branch chooses between becomes a `select`, the
//! arms' cheap code hoisted into the branch's block: the `foldTwoEntryPHINode`
//! step of SimplifyCFG.
//!
//! # LLVM structure
//! | LLVM | here | |
//! |---|---|---|
//! | [`foldTwoEntryPHINode`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L3834) | `folded` | MIRROR; D1, D2, D5 |
//! | [`GetIfCondition`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/BasicBlockUtils.cpp#L1784) | `if_condition` | MIRROR |
//! | [`dominatesMergePoint`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L456) | `dominates_merge_point` | MIRROR; D3 |
//! | [`computeSpeculationCost`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L434) | `instruction_cost` at `SizeAndLatency` | MIRROR |
//! | [`hoistAllInstructionsInto`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L3401) | `hoisted_into` | MIRROR; D4 |
//! | `two-entry-phi-node-folding-threshold` (4), `max-speculation-depth` (10), `speculate-one-expensive-inst` | `THRESHOLD`, `MAX_DEPTH`, on | MIRROR |
//!
//! The call site is
//! [`simplifyOnce`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9353):
//! a block whose first instruction is a phi of two entries.
//!
//! - D1. No branch weights or `!unpredictable` in the MIR: the test that leaves
//!   a predictably taken arm alone, and the extra budget of
//!   `SpeculateUnpredictables` (off at every slot but two later ones), are not
//!   ported.
//! - D2. The new select is not named after the phi (`takeName`): MIR has no
//!   rename.
//! - D3. The `with.overflow` + `extractvalue` pair is priced as two
//!   instructions, not one (`ZeroCostInstructions`).
//! - D4. A hoisted instruction keeps the metadata kinds LLVM keeps
//!   (`dropUBImplyingAttrsAndMetadata`) and loses the rest; there are no debug
//!   records to drop.
//! - D5. Where the target has no conditional move (`Tti::has_conditional_move`)
//!   the selects the fold makes are priced like the hoisted code, against the
//!   same budget. The pin charges none: a select is one instruction there.
//!   Reason: here each is a branch (`selects.rs`), so gcd's `if (a > b) a -= b;
//!   else b -= a;` became two selects and a loop of 239 clocks, 47 bytes. Exit:
//!   none; the select's cost is the target's.
//!
//! # Example
//! The phi that a branch chooses between becomes a select in the branch's
//! block:
//!
//! ```text
//! entry: br i1 %c, label %then, label %end
//! then:  br label %end
//! end:   %p = phi i32 [ 1, %then ], [ 2, %entry ]
//! ; becomes, in entry: %p = select i1 %c, i32 1, i32 2
//! ```

use llrm_analysis::occurrence::arms;
use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::IdSet;
use llrm_mir::edit::Position;
use llrm_mir::instructioncost::{instruction_cost, select_cost, select_logic};
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{FloatPredicate, IntPredicate, Opcode};
use llrm_mir::passes::Outer;
use llrm_mir::tti::{Cost, CostKind, Tti};

use crate::edges;

/// `two-entry-phi-node-folding-threshold`, in `TCC_Basic`.
const THRESHOLD: i64 = 4;
/// `max-speculation-depth`.
const MAX_DEPTH: usize = 10;
/// The metadata `Instruction::dropUBImplyingAttrsAndMetadata` keeps.
const KEPT_METADATA: [&str; 8] =
    ["annotation", "range", "nonnull", "align", "fpmath", "prof", "mem.cache_hint", "nofpclass"];

/// What speculating code costs and which code may be speculated.
pub struct Speculation<'a> {
    pub tti: &'a dyn Tti,
    pub layout: &'a DataLayout,
    pub outer: &'a Outer,
}

/// `br c, T, F` and the arms it chooses between.
struct IfCondition {
    branch: InstId,
    if_true: BlockId,
    if_false: BlockId,
}

enum Ending {
    Jump,
    Branch { inst: InstId, on_true: BlockId, on_false: BlockId },
    Other,
}

fn ending(
    function: &Function,
    block: BlockId,
) -> Ending {
    let Some(last) = function.terminator(block) else { return Ending::Other };
    match (&function.instruction(last).opcode, &function.instruction(last).operands[..]) {
        (Opcode::Br, &[Operand::Block(_)]) => Ending::Jump,
        (Opcode::Br, &[_, Operand::Block(on_true), Operand::Block(on_false)]) => {
            Ending::Branch { inst: last, on_true, on_false }
        }
        _ => Ending::Other,
    }
}

/// `GetIfCondition` for the two-entry phi `phi` of `block`.
fn if_condition(
    function: &Function,
    block: BlockId,
    phi: InstId,
) -> Option<IfCondition> {
    let [(_, mut first), (_, mut second)] = arms(function, phi)[..] else { return None };
    let (mut first_end, mut second_end) = (ending(function, first), ending(function, second));
    if matches!(second_end, Ending::Branch { .. }) {
        // Both conditional: no "if statement".
        if matches!(first_end, Ending::Branch { .. }) {
            return None;
        }
        std::mem::swap(&mut first, &mut second);
        std::mem::swap(&mut first_end, &mut second_end);
    }
    if !matches!(second_end, Ending::Jump) {
        return None;
    }
    match first_end {
        Ending::Branch { inst, on_true, on_false } => {
            // The condition must dominate the block: the other arm has no other
            // way in.
            if function.sole_predecessor(second).is_none() {
                return None;
            }
            let (if_true, if_false) = if on_true == block && on_false == second {
                (first, second)
            } else if on_true == second && on_false == block {
                (second, first)
            } else {
                return None;
            };
            Some(IfCondition { branch: inst, if_true, if_false })
        }
        Ending::Jump => {
            let common = single_predecessor(function, first)?;
            if single_predecessor(function, second) != Some(common) {
                return None;
            }
            let Ending::Branch { inst, on_true, .. } = ending(function, common) else { return None };
            let (if_true, if_false) = if on_true == first { (first, second) } else { (second, first) };
            Some(IfCondition { branch: inst, if_true, if_false })
        }
        Ending::Other => None,
    }
}

fn single_predecessor(
    function: &Function,
    block: BlockId,
) -> Option<BlockId> {
    let predecessors = function.predecessors(block);
    (predecessors.len() == 1).then(|| predecessors[0])
}

/// `dominatesMergePoint`: `value` is available at the branch, or is code of
/// an arm that is safe and cheap to run there (and then so are its operands).
/// `cost` is raised by what is added to `hoist`.
#[allow(clippy::too_many_arguments)]
fn dominates_merge_point(
    context: &Context,
    spec: &Speculation,
    function: &Function,
    value: Operand,
    block: BlockId,
    hoist: &mut Vec<InstId>,
    seen: &mut IdSet<InstId>,
    cost: &mut Cost,
    depth: usize,
) -> bool {
    if depth == MAX_DEPTH {
        return false;
    }
    let Operand::Value(id) = value else { return true };
    let ValueDef::Instruction(inst) = function.value(id).def else { return true };
    let Some(parent) = function.parent(inst) else { return true };
    // The condition may sit at the bottom of a weird loop.
    if parent == block {
        return false;
    }
    // Not in the conditional part of the "if statement": it dominates it.
    let jumps_here = function
        .terminator(parent)
        .is_some_and(|last| function.instruction(last).operands[..] == [Operand::Block(block)]);
    if !jumps_here {
        return true;
    }
    if seen.contains(&inst) {
        return true;
    }
    if !crate::licm::hoist::is_safe_to_speculatively_execute_for_cfg(context, spec.layout, function, spec.outer, inst) {
        return false;
    }
    *cost = *cost
        + instruction_cost(
            spec.tti,
            context,
            spec.layout,
            &spec.outer.globals,
            function,
            inst,
            CostKind::SizeAndLatency,
        );
    // Exactly one expensive instruction is let through, if it is the first.
    if cost.value > THRESHOLD * Cost::BASIC.value && (!hoist.is_empty() || depth > 0 || !cost.valid) {
        return false;
    }
    for &operand in &function.instruction(inst).operands {
        if !dominates_merge_point(context, spec, function, operand, block, hoist, seen, cost, depth + 1) {
            return false;
        }
    }
    seen.insert(inst);
    hoist.push(inst);
    true
}

/// `IsBinOpOrAndEq`: a binary operation of a comparison, or a select of a
/// constant and a comparison, where the comparison tests equality.
fn is_binary_or_and_equality(
    context: &Context,
    function: &Function,
    value: Operand,
) -> bool {
    let equality = |operand: Operand| -> Option<bool> {
        let Operand::Value(id) = operand else { return None };
        let ValueDef::Instruction(inst) = function.value(id).def else { return None };
        match function.instruction(inst).opcode {
            Opcode::ICmp(predicate) => Some(matches!(predicate, IntPredicate::Eq | IntPredicate::Ne)),
            Opcode::FCmp(predicate) => Some(matches!(
                predicate,
                FloatPredicate::Oeq | FloatPredicate::One | FloatPredicate::Ueq | FloatPredicate::Une
            )),
            _ => None,
        }
    };
    let Operand::Value(id) = value else { return false };
    let ValueDef::Instruction(inst) = function.value(id).def else { return false };
    let operation = function.instruction(inst);
    match operation.opcode {
        Opcode::Binary(_) => {
            equality(operation.operands[0]).or_else(|| equality(operation.operands[1])).unwrap_or(false)
        }
        Opcode::Select => {
            let immediate = |operand: Operand| {
                matches!(
                    operand,
                    Operand::Constant(id) if !matches!(context.get(id).kind, ConstantKind::Expr(_))
                )
            };
            let [_, a, b] = operation.operands[..] else { return false };
            if immediate(a) {
                equality(b)
            } else if immediate(b) {
                equality(a)
            } else {
                None
            }
            .unwrap_or(false)
        }
        _ => false,
    }
}

/// The two-entry phi at the top of `block` folded into a select, if a branch
/// chooses between its values and what the arms compute is cheap and safe.
/// Whether anything changed.
pub fn folded(
    context: &mut Context,
    spec: &Speculation,
    function: &mut Function,
    block: BlockId,
) -> bool {
    let Some(first) = function.block(block).first() else { return false };
    if function.instruction(first).opcode != Opcode::Phi || function.instruction(first).operands.len() != 4 {
        return false;
    }
    let Some(found) = if_condition(function, block, first) else { return false };
    let if_cond = function.instruction(found.branch).operands[0];
    // The branch folds trivially.
    if matches!(
        if_cond,
        Operand::Constant(id) if matches!(context.get(id).kind, ConstantKind::Int(_))
    ) {
        return false;
    }
    let if_blocks: Vec<BlockId> = arms(function, first)
        .into_iter()
        .map(|(_, from)| from)
        .filter(|&from| matches!(ending(function, from), Ending::Jump))
        .collect();
    // The phi itself cannot be the condition of the select that replaces it.
    if let Operand::Value(id) = if_cond
        && let ValueDef::Instruction(inst) = function.value(id).def
        && function.instruction(inst).opcode == Opcode::Phi
        && function.parent(inst) == Some(block)
    {
        return false;
    }
    // Folding one two-entry phi means folding all of the block's: no more than
    // three.
    if edges::phis(function, block).len() > 3 {
        return false;
    }

    let (mut hoist, mut seen, mut cost) = (Vec::new(), IdSet::new(), Cost::FREE);
    let mut changed = false;
    for phi in edges::phis(function, block) {
        if let Some(value) = llrm_mir::simplify::simplify_instruction(context, spec.layout, function, phi) {
            let result = function.instruction(phi).result.expect("a phi's value");
            function.replace_all_uses_with(result, value);
            function.erase(phi).expect("a phi nothing reads is erased");
            changed = true;
            continue;
        }
        for (value, _) in arms(function, phi) {
            if !dominates_merge_point(context, spec, function, value, block, &mut hoist, &mut seen, &mut cost, 0) {
                return changed;
            }
        }
    }
    // Every phi folded away.
    let Some(first) = function.block(block).first().filter(|&one| function.instruction(one).opcode == Opcode::Phi)
    else {
        return true;
    };
    // An `i1` of equality tests is for InstCombine to fold, not a select.
    let boolean = context.types.int_bits(function.instruction(first).ty) == Some(1);
    if boolean
        && (arms(function, first).iter().any(|&(value, _)| is_binary_or_and_equality(context, function, value))
            || is_binary_or_and_equality(context, function, if_cond))
    {
        return changed;
    }
    // D5. Without a conditional move each select is a branch again, so the
    // selects count against the budget besides the code they hoist.
    if !spec.tti.has_conditional_move() {
        let kind = CostKind::SizeAndLatency;
        let mut total = cost;
        for phi in edges::phis(function, block) {
            let ty = function.instruction(phi).ty;
            let on_true = arms(function, phi).into_iter().find(|&(_, from)| from == found.if_true).expect("an arm").0;
            let on_false = arms(function, phi).into_iter().find(|&(_, from)| from == found.if_false).expect("an arm").0;
            // A boolean `and`/`or` is no branch.
            if select_logic(context, ty, on_true, on_false).is_none() {
                total = total + select_cost(context, spec.tti, ty, if_cond, on_true, on_false, kind);
            }
        }
        if total.value > THRESHOLD * Cost::BASIC.value || !total.valid {
            return changed;
        }
    }
    // Unless the arms are emptied, the control flow stays.
    for &arm in &if_blocks {
        if function.iter_block(arm).take(function.block(arm).len() - 1).any(|one| !seen.contains(&one)) {
            return changed;
        }
    }

    for &arm in &if_blocks {
        hoisted_into(function, found.branch, arm);
    }
    for phi in edges::phis(function, block) {
        let operation = function.instruction(phi);
        let (ty, flags) = (operation.ty, operation.flags);
        let on_true = arms(function, phi).into_iter().find(|&(_, from)| from == found.if_true).expect("an arm").0;
        let on_false = arms(function, phi).into_iter().find(|&(_, from)| from == found.if_false).expect("an arm").0;
        let select = function.create_instruction(Opcode::Select, ty, vec![if_cond, on_true, on_false], flags, None);
        function.insert(select, Position::Before(found.branch)).expect("a new instruction");
        let result = function.instruction(phi).result.expect("a phi's value");
        let chosen = function.instruction(select).result.expect("a select's value");
        function.replace_all_uses_with(result, Operand::Value(chosen));
        function.set_operands(phi, Vec::new());
        function.erase(phi).expect("a phi nothing reads is erased");
    }
    // The arms are empty now: the branch's block jumps to the merge.
    let void = function.instruction(found.branch).ty;
    let jump = function.create_instruction(Opcode::Br, void, vec![Operand::Block(block)], Default::default(), None);
    function.insert(jump, Position::Before(found.branch)).expect("a new instruction");
    function.erase(found.branch).expect("a terminator defines nothing");
    true
}

/// `hoistAllInstructionsInto`: the instructions of `arm` before its jump,
/// moved before `before`.
fn hoisted_into(
    function: &mut Function,
    before: InstId,
    arm: BlockId,
) {
    let body = function.iter_block(arm).collect::<Vec<_>>();
    for &inst in &body[..body.len() - 1] {
        function.move_to(inst, Position::Before(before)).expect("a placed instruction");
        metadata_dropped(function, inst);
    }
}

/// `dropUBImplyingAttrsAndMetadata`: `inst` keeps the metadata kinds that
/// cannot make a speculated instruction undefined.
pub(crate) fn metadata_dropped(
    function: &mut Function,
    inst: InstId,
) {
    let kinds: Vec<String> = function
        .instruction(inst)
        .metadata
        .iter()
        .filter(|(kind, _)| !KEPT_METADATA.contains(&kind.as_str()))
        .map(|(kind, _)| kind.clone())
        .collect();
    for kind in kinds {
        function.set_metadata(inst, &kind, None);
    }
}
