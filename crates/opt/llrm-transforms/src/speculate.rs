//! A block that only a branch reaches and that jumps back to the branch's
//! other target runs before the branch, its phis in the target becoming
//! selects: the `speculativelyExecuteBB` step of SimplifyCFG.
//!
//! # LLVM structure
//! | LLVM | here | |
//! |---|---|---|
//! | [`speculativelyExecuteBB`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L3244) | `executed` | MIRROR; D1, D2, D3 |
//! | [`validateAndCostRequiredSelects`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L3130) | `selects_priced` | MIRROR; D4 |
//! | [`passingValueIsAlwaysUndefined`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9094) | `passing_value_always_undefined` | partly; D5 |
//! | `phi-node-folding-threshold` (2) | `THRESHOLD` | MIRROR |
//!
//! The call sites are in
//! [`simplifyCondBranch`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9060),
//! behind `SpeculateBlocks`. One instruction at most runs early, and a
//! value of the branch's block that only that instruction reads counts as one
//! more.
//!
//! - D1. Stores, loads that fault conditionally and `HoistCondStores` are not
//!   ported: a block with a store or a load that may trap is left alone.
//! - D2. No branch weights or `!unpredictable` (`isProfitableToSpeculate` is
//!   true), no ephemeral values (no assumption cache), no pseudo probes, no
//!   `minsize` (costs are `SizeAndLatency`).
//! - D3. A hoisted instruction keeps the metadata kinds LLVM keeps and loses
//!   the rest, as in [`twoentryphi`](crate::twoentryphi) (D4 there).
//! - D4. No constant expressions in function bodies: LLVM's price of one is not
//!   needed.
//! - D5. Only the uses LLVM lists that need no attribute are tested: a load or
//!   store through the value, a call of it, a division by it. The look through
//!   a `getelementptr`, the `noundef`/`nonnull` return and argument rules and
//!   `llvm.assume` are not ported; the MIR has no `NullPointerIsDefined`, so a
//!   null pointer is undefined. What stands between the phi and the use must be
//!   no call (`isGuaranteedToTransferExecutionToSuccessor`).
//!
//! # Example
//! The arm's one cheap instruction runs before the branch and the phi becomes a
//! select:
//!
//! ```text
//! entry: br i1 %c, label %then, label %end
//! then:  %t = add i32 %x, 1 ; br label %end
//! end:   %p = phi i32 [ %t, %then ], [ %x, %entry ]
//! ; becomes, in entry: %t = add i32 %x, 1 ; %p = select i1 %c, i32 %t, i32 %x
//! ```

use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::dense::IdMap;
use llrm_mir::edit::Position;
use llrm_mir::instructioncost::instruction_cost;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, Opcode};
use llrm_mir::tti::{CmpSel, Cost, CostKind, Ty};

use crate::twoentryphi::{Speculation, metadata_dropped};

/// `phi-node-folding-threshold`, in `TCC_Basic`.
const THRESHOLD: i64 = 2;

/// `value`, an input of `phi`, makes the program undefined where it reaches
/// a use in the phi's block.
fn passing_value_always_undefined(
    context: &Context,
    function: &Function,
    value: Operand,
    phi: InstId,
) -> bool {
    let Operand::Constant(id) = value else { return false };
    // A null, a zero or a poison.
    let null = matches!(
        context.get(id).kind,
        ConstantKind::Null | ConstantKind::Zero | ConstantKind::Poison | ConstantKind::Undef | ConstantKind::Int(0)
    );
    if !null {
        return false;
    }
    let result = function.instruction(phi).result.expect("a phi's value");
    if function.users(result).is_empty() {
        return false;
    }
    let block = function.parent(phi).expect("a placed phi");
    // The first use in the block after the phi.
    let user = function
        .users(result)
        .iter()
        .filter(|one| function.parent(one.user) == Some(block) && one.user != phi)
        .filter(|one| function.comes_before(phi, one.user))
        .min_by(|a, b| {
            if a.user == b.user {
                std::cmp::Ordering::Equal
            } else if function.comes_before(a.user, b.user) {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            }
        });
    let Some(user) = user else { return false };
    // The block's earlier uses decide first: LLVM's `find_if` stops at the
    // first use in use-list order, which is also the first that is not a
    // mismatch.
    let (inst, index) = (user.user, user.index as usize);
    if !matches!(
        function.instruction(inst).opcode,
        Opcode::GetElementPtr { .. }
            | Opcode::Ret
            | Opcode::Load { .. }
            | Opcode::Store { .. }
            | Opcode::Call(_)
            | Opcode::Invoke(_)
            | Opcode::Binary(BinaryOp::UDiv | BinaryOp::URem | BinaryOp::SDiv | BinaryOp::SRem)
    ) {
        return false;
    }
    let mut between = function.iter_after(phi).take_while(|&one| one != inst);
    if between.any(|one| matches!(
        function.instruction(one).opcode,
        Opcode::Call(_) | Opcode::Invoke(_)
    )) {
        return false;
    }
    let operation = function.instruction(inst);
    match &operation.opcode {
        Opcode::Load { volatile, .. } => !volatile,
        Opcode::Store { volatile, .. } => !volatile && index == 1,
        Opcode::Call(_) | Opcode::Invoke(_) => index == operation.operands.len() - 1,
        Opcode::Binary(_) => index == 1,
        _ => false,
    }
}

/// The cost of the selects `end`'s phis need once `then` runs early, or None
/// where a phi cannot be a select.
fn selects_priced(
    context: &Context,
    spec: &Speculation,
    function: &Function,
    block: BlockId,
    then: BlockId,
    end: BlockId,
) -> Option<Cost> {
    let mut cost = Cost::FREE;
    let mut rewritable = false;
    for phi in crate::edges::phis(function, end) {
        let arms = llrm_analysis::occurrence::arms(function, phi);
        let from = |at: BlockId| arms.iter().find(|&&(_, one)| one == at).map(|&(value, _)| value);
        let (Some(original), Some(speculated)) = (from(block), from(then)) else { return None };
        if original == speculated {
            continue;
        }
        let ty = Ty::of(&context.types, function.instruction(phi).ty)?;
        cost = cost + spec.tti.cmp_sel_cost(CmpSel::Select, ty, CostKind::SizeAndLatency);
        if passing_value_always_undefined(context, function, original, phi)
            || passing_value_always_undefined(context, function, speculated, phi)
        {
            return None;
        }
        rewritable = true;
    }
    rewritable.then_some(cost)
}

/// `then` (reached only by the branch ending `block`, and jumping to `end`,
/// the branch's other target) run before the branch. Whether it was.
pub fn executed(
    context: &Context,
    spec: &Speculation,
    function: &mut Function,
    block: BlockId,
    then: BlockId,
) -> bool {
    let Some(branch) = function.terminator(block) else { return false };
    let operands = function.instruction(branch).operands.clone();
    let [condition, Operand::Block(first), Operand::Block(second)] = operands[..] else { return false };
    let Some(end) = function.terminator(then).and_then(|last| match function.instruction(last).operands[..] {
        [Operand::Block(end)] => Some(end),
        _ => None,
    }) else {
        return false;
    };
    let invert = then != first;
    if end != if invert { first } else { second } {
        return false;
    }

    let mut speculated = 0;
    // Uses in `then` of the values of `block` that nothing else reads.
    let mut sinkable: IdMap<InstId, usize> = IdMap::new();
    let body = function.iter_block(then).collect::<Vec<_>>();
    for &inst in body[..body.len() - 1].iter().rev() {
        speculated += 1;
        if speculated > 1 {
            return false;
        }
        if !crate::licm::hoist::is_safe_to_speculatively_execute_in(context, spec.layout, function, spec.outer, inst)
            || matches!(
                function.instruction(inst).opcode,
                Opcode::Load { .. } | Opcode::Store { .. }
            )
        {
            return false;
        }
        let price = instruction_cost(
            spec.tti,
            context,
            spec.layout,
            &spec.outer.globals,
            function,
            inst,
            CostKind::SizeAndLatency,
        );
        if !price.valid || price.value > THRESHOLD {
            return false;
        }
        // An operand of `block` that only `then` reads stays behind otherwise.
        for &operand in &function.instruction(inst).operands {
            let Operand::Value(id) = operand else { continue };
            let ValueDef::Instruction(def) = function.value(id).def else { continue };
            if function.parent(def) != Some(block)
                || !llrm_mir::memory::only_value(context, spec.outer.callees(), function, def)
            {
                continue;
            }
            let seen = sinkable.get(&def).copied().unwrap_or(0);
            sinkable.insert(def, seen + 1);
        }
    }
    for (def, uses) in sinkable.iter() {
        let result = function.instruction(def).result.expect("a value used as an operand");
        if function.users(result).has_n_uses(*uses) {
            speculated += 1;
            if speculated > 1 {
                return false;
            }
        }
    }

    let Some(cost) = selects_priced(context, spec, function, block, then, end) else { return false };
    if cost.value > THRESHOLD * Cost::BASIC.value || !cost.valid {
        return false;
    }

    for &inst in &body[..body.len() - 1] {
        metadata_dropped(function, inst);
        function.move_to(inst, Position::Before(branch)).expect("a placed instruction");
    }
    for phi in crate::edges::phis(function, end) {
        let arms = llrm_analysis::occurrence::arms(function, phi);
        let at = |from: BlockId| arms.iter().position(|&(_, one)| one == from).expect("an arm");
        let (original_at, then_at) = (at(block), at(then));
        let (original, speculated) = (arms[original_at].0, arms[then_at].0);
        if original == speculated {
            continue;
        }
        let (on_true, on_false) = if invert { (original, speculated) } else { (speculated, original) };
        let operation = function.instruction(phi);
        let (ty, flags) = (operation.ty, operation.flags);
        let select = function.create_instruction(Opcode::Select, ty, vec![condition, on_true, on_false], flags, None);
        function.insert(select, Position::Before(branch)).expect("a new instruction");
        let chosen = Operand::Value(function.instruction(select).result.expect("a select's value"));
        function.set_operand(phi, 2 * original_at, chosen);
        function.set_operand(phi, 2 * then_at, chosen);
    }
    true
}
