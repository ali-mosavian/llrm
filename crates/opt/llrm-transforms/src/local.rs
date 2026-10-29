//! The utilities GVN calls on a CSE: patching the surviving instruction,
//! replacing the uses a dominator tree allows, dead and duplicate-phi
//! cleanup, block merging, and the pointer-equality replacement checks.
//!
//! A translation of LLVM's
//! [`Local.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp)
//! and the parts of `Dominators.cpp`, `BasicBlockUtils.cpp`, `Loads.cpp`,
//! `Instruction.cpp` and `Attributes.cpp` it calls, at d1106deb71cc. Each
//! function names the C++ function it translates, keeps its order of checks,
//! and uses LLVM's name in snake case. Not called yet.
//!
//! # How it works
//! MIR has no `Use` object: a use is [`Use`] (user and operand index), and a
//! phi's value operand `i` comes in on the block operand `i + 1`. `RAUW` is
//! `Function::replace_value`, which also points a debug record at the new
//! value, as LLVM's metadata-as-value does.
//!
//! DEVIATE, each with the fact MIR lacks:
//! 1. `MDNode::getMostGeneric*`, `intersect`, and the merged `!prof`,
//!    `!callees`, `!captures`: MIR's nodes are untyped tuples and nothing
//!    builds a merged one. A kind both instructions carry in two different
//!    nodes is dropped, which LLVM may keep merged. Two non-distinct nodes with
//!    equal operands are one node, as LLVM uniques them.
//! 2. `!DIAssignID`, `!mmra`, `!memprof`, `!callsite`, `!callee_type`,
//!    `!alloc_token`: nothing in MIR reads them (debug info is
//!    `Function::debug_records`, not metadata). They are dropped, the arm for a
//!    kind LLVM does not know.
//! 3. `ConstantRange::unionWith` for a `range` attribute: MIR has no
//!    `ConstantRange`. Two different ranges drop the attribute.
//! 4. `addrspacecast nonnull`: `Flags` has no such bit.
//! 5. `dropPoisonGeneratingFlags` on `ctlz`, `cttz` and `abs`: MIR has none of
//!    them.
//! 6. `llvm.fake.use`: not in the intrinsic table, so no use is skipped.
//! 7. `DbgLabelInst`: a debug label is not an instruction here.
//! 8. `isRemovableAlloc`, `getFreedOperand`, `isMathLibCallNoop`: MIR has no
//!    `allockind` attribute and no `TargetLibraryInfo`, so no call is a known
//!    allocation, free or libm function.
//! 9. `experimental_guard`, `wasm_trunc_*`, `ptrauth_*`, `stacksave`,
//!    `launder_invariant_group`, `allow_runtime_check`, `allow_ubsan_check` and
//!    the constrained FP intrinsics: not in the intrinsic table.
//! 10. A non-volatile load from a constant global: a load has no atomic
//!     ordering, so every one without side effects is already dead above it.
//! 11. A lifetime marker's pointer is operand 1 (`size, ptr`). A constant
//!     pointer's uses are not tracked per function, so its markers are kept.
//! 12. `isAssumeWithEmptyBundle`: MIR has no operand bundles, so it is true.
//! 13. `mayThrow` of an invoke needs `canUnwindPastLandingPad`: an invoke may
//!     throw.
//! 14. `isIdenticalTo` and `isIdenticalToWhenDefined`: phis only, the one
//!     caller. A phi's blocks are operands, so comparing them compares blocks.
//! 15. `MergeBlockIntoPredecessor`'s `DomTreeUpdater` and `DominatorTree`,
//!     `LoopInfo`, `MemorySSAUpdater` and `MemoryDependenceResults`: MIR passes
//!     hold none of them, the caller rebuilds. The debug-only `IncomingValues`
//!     goes with them.
//! 16. `MergeBlockIntoPredecessor`'s `hasAddressTaken` (MIR has no
//!     `blockaddress`), `HasLoopOrEntryConvergenceToken` (no convergence
//!     tokens), `PTI->mayHaveSideEffects` (after `isSpecialTerminator` a
//!     terminator is a `ret`, `br`, `switch` or `unreachable`, which never
//!     writes, throws or fails to return), `takeName` (a `Function` has no
//!     block-name setter), and the `unreachable` left in the merged block (the
//!     block is erased at once).
//! 17. `PtrToAddrInst`: MIR has no `ptrtoaddr`. `getUnderlyingObjectAggressive`
//!     is `valuetracking::underlying`, which does not look through a `select`,
//!     a `phi` or a call that returns an argument: two pointers that differ
//!     only below one are not replaceable here, where LLVM may say they are.
//! 24. `MergeBlockIntoPredecessor` erases `PTI` after redirecting the uses of
//!     `BB`; here before. A `DbgMarker` hands `PTI`'s records to the next
//!     instruction or the trailing list, which `STI` adopts when it moves.
//!     `Function::erase` re-homes a record when its instruction goes
//!     (`settle_records`), and the branch must still name `BB` for that to
//!     reach `STI`: redirected first, it names its own block and the records
//!     stay on the erased branch. Asserted by
//!     `merging_leaves_the_records_of_the_old_branch_before_the_new_terminator`.
//! 25. `!llrm.descriptor` (ours, no LLVM kind): kept where both instructions
//!     carry it, dropped otherwise. LLVM's unknown-kind arm would drop it, and
//!     the merged `inttoptr` then reads as a forged selector (`forging`).
//!
//! # Example
//! Two phis with the same arms in one block. Before:
//!
//! ```mir
//! %p = phi i16 [ %x, %b1 ], [ %y, %b2 ]
//! %q = phi i16 [ %x, %b1 ], [ %y, %b2 ]
//! ```
//!
//! `eliminate_duplicate_phi_nodes` puts `%q` in the set to remove and makes it
//! read `%p`; after the caller erases it only `%p` is left.
//!
//! From unit test `a_duplicate_phi_is_replaced_by_the_first` in
//! `local_tests.rs`.
//!
//! # Used by
//! Nothing yet; `gvn` is the caller this is for.

use llrm_analysis::effects::{self, Declarations};
use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::IdSet;
use llrm_mir::domtree::DomTree;
use llrm_mir::edit::Position;
use llrm_mir::facts::Fact;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::memory::{self, Callees};
use llrm_mir::module::{
    BlockId, DESCRIPTOR_WORD, Function, GlobalKind, InstId, Instruction, MetadataId, MetadataNode, Operand, Use,
    ValueDef, ValueId,
};
use llrm_mir::opcode::{Attribute, BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::types::{Type, TypeId};
use llrm_mir::valuetracking::{self, Sizes};
use llrm_support::hash::HashMap;

use crate::edges;

/// `BasicBlockEdge`: the edge from `start` to `end`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BasicBlockEdge {
    pub start: BlockId,
    pub end: BlockId,
}

fn type_of(
    context: &Context,
    function: &Function,
    operand: Operand,
) -> TypeId {
    function.operand_type(context, operand).expect("a value or a constant")
}

fn is_ptr_or_ptr_vector(
    context: &Context,
    ty: TypeId,
) -> bool {
    match context.types.get(ty) {
        Type::Pointer(_) => true,
        Type::Vector { element, .. } => matches!(context.types.get(*element), Type::Pointer(_)),
        _ => false,
    }
}

fn pointer_address_space(
    context: &Context,
    ty: TypeId,
) -> u32 {
    match context.types.get(ty) {
        Type::Pointer(space) => *space,
        Type::Vector { element, .. } => pointer_address_space(context, *element),
        _ => 0,
    }
}

/// The block operand paired with a phi's value operand `at`:
/// `getIncomingBlock(U)`.
fn incoming_block(
    function: &Function,
    at: Use,
) -> Option<BlockId> {
    let user = function.instruction(at.user);
    if user.opcode != Opcode::Phi {
        return None;
    }
    match user.operands[at.index as usize + 1] {
        Operand::Block(block) => Some(block),
        other => unreachable!("a phi's block operand, not {other:?}"),
    }
}

/// The blocks whose terminators name `block`, once per edge:
/// `predecessors(BB)`.
fn edges_into(
    function: &Function,
    block: BlockId,
) -> Vec<BlockId> {
    function
        .block_users(block)
        .iter()
        .filter(|one| function.instruction(one.user).opcode.is_terminator())
        .filter_map(|one| function.parent(one.user))
        .collect()
}

/// The call `inst`'s intrinsic, if its callee is one.
fn intrinsic_of(
    context: &Context,
    declarations: &Declarations,
    function: &Function,
    inst: InstId,
) -> Option<Intrinsic> {
    let global = memory::callee(context, function, inst)?;
    declarations.get(global.0 as usize)?.intrinsic()
}

pub(crate) fn is_lifetime(
    context: &Context,
    declarations: &Declarations,
    function: &Function,
    inst: InstId,
) -> bool {
    matches!(
        intrinsic_of(context, declarations, function, inst),
        Some(Intrinsic::LifetimeStart | Intrinsic::LifetimeEnd)
    )
}

// ---- Dominators.cpp

/// `Instruction::comesBefore`
/// ([Instruction.cpp:335](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instruction.cpp#L335)):
/// by the position in the block.
fn comes_before(
    function: &Function,
    this: InstId,
    other: InstId,
) -> bool {
    let block = function.parent(this).expect("instructions without BB parents have no order");
    assert_eq!(function.parent(other), Some(block), "cross-BB instruction order comparison");
    function.comes_before(this, other)
}

/// `DominatorTree::dominates(const BasicBlock *, const Use &)`
/// ([Dominators.cpp:80](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Dominators.cpp#L80)):
/// whether `block` dominates the use, a phi's at the end of its incoming
/// block.
pub fn dominates_block_use(
    tree: &DomTree,
    function: &Function,
    block: BlockId,
    at: Use,
) -> bool {
    match incoming_block(function, at) {
        // A phi use using a value from a block is dominated by the end of that
        // block. Note that the phi's parent block may not be.
        Some(incoming) => tree.dominates(block, incoming),
        None => {
            let parent = function.parent(at.user).expect("a placed user");
            block != parent && tree.dominates(block, parent)
        }
    }
}

/// `DominatorTree::dominates(const BasicBlockEdge &, const BasicBlock *)`
/// ([Dominators.cpp:162](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Dominators.cpp#L162)):
/// whether the edge dominates `use_block`.
pub fn dominates_edge_block(
    tree: &DomTree,
    function: &Function,
    edge: BasicBlockEdge,
    use_block: BlockId,
) -> bool {
    // If the BB the edge ends in doesn't dominate the use BB, then the edge
    // also doesn't.
    if !tree.dominates(edge.end, use_block) {
        return false;
    }
    let predecessors = edges_into(function, edge.end);
    // Simple case: if the end BB has a single predecessor, the fact that it
    // dominates the use block implies that the edge also does.
    if predecessors.len() == 1 {
        return true;
    }
    // The end block dominates every other predecessor, or the edge, standing
    // for a block split onto it, would not dominate the end block.
    let mut is_duplicate_edge = 0;
    for block in predecessors {
        if block == edge.start {
            // Multiple edges between start and end can't dominate anything.
            is_duplicate_edge += 1;
            if is_duplicate_edge > 1 {
                return false;
            }
            continue;
        }
        if !tree.dominates(edge.end, block) {
            return false;
        }
    }
    true
}

/// `DominatorTree::dominates(const BasicBlockEdge &, const Use &)`
/// ([Dominators.cpp:213](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Dominators.cpp#L213)):
/// whether the edge dominates the use, a phi's at the end of its incoming
/// block.
pub fn dominates_edge_use(
    tree: &DomTree,
    function: &Function,
    edge: BasicBlockEdge,
    at: Use,
) -> bool {
    let incoming = incoming_block(function, at);
    // A PHI in the end of the edge is dominated by it.
    if incoming == Some(edge.start) && function.parent(at.user) == Some(edge.end) {
        return true;
    }
    // Otherwise use the edge-dominates-block query, which handles the crazy
    // critical edge cases properly.
    let use_block = incoming.unwrap_or_else(|| function.parent(at.user).expect("a placed user"));
    dominates_edge_block(tree, function, edge, use_block)
}

/// `DominatorTree::dominates(const Value *, const Use &)`
/// ([Dominators.cpp:231](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Dominators.cpp#L231)),
/// for an instruction: whether `def` dominates the use, a phi's at the end of
/// its incoming block.
pub fn dominates_instruction_use(
    tree: &DomTree,
    function: &Function,
    def: InstId,
    at: Use,
) -> bool {
    let def_block = function.parent(def).expect("a placed definition");
    // Determine the block in which the use happens. PHI nodes use their
    // operands on edges; simulate this by thinking of the use happening at the
    // end of the predecessor block.
    let use_block = incoming_block(function, at).unwrap_or_else(|| function.parent(at.user).expect("a placed user"));
    // Any unreachable use is dominated, even if Def == User.
    if !tree.is_reachable(use_block) {
        return true;
    }
    // Unreachable definitions don't dominate anything.
    if !tree.is_reachable(def_block) {
        return false;
    }
    // Invoke instructions define their return values on the edges to their
    // normal successors.
    if matches!(function.instruction(def).opcode, Opcode::Invoke(_)) {
        let operands = &function.instruction(def).operands;
        let Operand::Block(normal) = operands[operands.len() - 3] else { unreachable!("an invoke's normal block") };
        return dominates_edge_use(tree, function, BasicBlockEdge { start: def_block, end: normal }, at);
    }
    // If the def and use are in different blocks, do a simple CFG dominator
    // tree query.
    if def_block != use_block {
        return tree.dominates(def_block, use_block);
    }
    // Same block: a PHI is dominated by everything in it.
    if function.instruction(at.user).opcode == Opcode::Phi {
        return true;
    }
    comes_before(function, def, at.user)
}

// ---- Local.cpp: replaceDominatedUsesWith

/// `replaceDominatedUsesWith`, the template
/// ([Local.cpp:3238](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L3238)):
/// every use of `from` that `should_replace` accepts reads `to`; how many.
fn replace_dominated_uses_with_template(
    context: &Context,
    function: &mut Function,
    from: ValueId,
    to: Operand,
    should_replace: impl Fn(&Function, Use) -> bool,
) -> usize {
    debug_assert_eq!(function.value(from).ty, type_of(context, function, to));
    let mut count = 0;
    // `fake_use` users are not skipped: DEVIATE 6.
    for at in function.users(from).snapshot() {
        if !should_replace(&*function, at) {
            continue;
        }
        function.set_operand(at.user, at.index as usize, to);
        count += 1;
    }
    count
}

/// `replaceDominatedUsesWith(Value *, Value *, DominatorTree &, const
/// BasicBlockEdge &)`
/// ([Local.cpp:3273](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L3273)):
/// the uses of `from` the edge dominates read `to`.
pub fn replace_dominated_uses_with_edge(
    context: &Context,
    function: &mut Function,
    from: ValueId,
    to: Operand,
    tree: &DomTree,
    root: BasicBlockEdge,
) -> usize {
    let dominates = |function: &Function, at: Use| dominates_edge_use(tree, function, root, at);
    replace_dominated_uses_with_template(context, function, from, to, dominates)
}

/// `replaceDominatedUsesWith(Value *, Value *, DominatorTree &, const
/// BasicBlock *)`
/// ([Local.cpp:3280](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L3280)):
/// the uses of `from` the block dominates read `to`.
pub fn replace_dominated_uses_with_block(
    context: &Context,
    function: &mut Function,
    from: ValueId,
    to: Operand,
    tree: &DomTree,
    block: BlockId,
) -> usize {
    let dominates = |function: &Function, at: Use| dominates_block_use(tree, function, block, at);
    replace_dominated_uses_with_template(context, function, from, to, dominates)
}

/// `replaceDominatedUsesWith(Value *, Value *, DominatorTree &, const
/// Instruction *)`
/// ([Local.cpp:3287](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L3287)):
/// the uses of `from` the instruction dominates read `to`.
pub fn replace_dominated_uses_with_instruction(
    context: &Context,
    function: &mut Function,
    from: ValueId,
    to: Operand,
    tree: &DomTree,
    root: InstId,
) -> usize {
    let dominates = |function: &Function, at: Use| dominates_instruction_use(tree, function, root, at);
    replace_dominated_uses_with_template(context, function, from, to, dominates)
}

/// `replaceDominatedUsesWithIf(..., const BasicBlockEdge &, ShouldReplace)`
/// ([Local.cpp:3294](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L3294)):
/// as the edge form, where `should_replace` also accepts the use.
pub fn replace_dominated_uses_with_if_edge(
    context: &Context,
    function: &mut Function,
    from: ValueId,
    to: Operand,
    tree: &DomTree,
    root: BasicBlockEdge,
    should_replace: impl Fn(&Function, Use, Operand) -> bool,
) -> usize {
    let dominates_and_should_replace =
        |function: &Function, at: Use| dominates_edge_use(tree, function, root, at) && should_replace(function, at, to);
    replace_dominated_uses_with_template(context, function, from, to, dominates_and_should_replace)
}

/// `replaceDominatedUsesWithIf(..., const BasicBlock *, ShouldReplace)`
/// ([Local.cpp:3303](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L3303)):
/// as the block form, where `should_replace` also accepts the use.
pub fn replace_dominated_uses_with_if_block(
    context: &Context,
    function: &mut Function,
    from: ValueId,
    to: Operand,
    tree: &DomTree,
    block: BlockId,
    should_replace: impl Fn(&Function, Use, Operand) -> bool,
) -> usize {
    let dominates_and_should_replace = |function: &Function, at: Use| {
        dominates_block_use(tree, function, block, at) && should_replace(function, at, to)
    };
    replace_dominated_uses_with_template(context, function, from, to, dominates_and_should_replace)
}

/// `replaceDominatedUsesWithIf(..., const Instruction *, ShouldReplace)`
/// ([Local.cpp:3312](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L3312)):
/// as the instruction form, where `should_replace` also accepts the use.
pub fn replace_dominated_uses_with_if_instruction(
    context: &Context,
    function: &mut Function,
    from: ValueId,
    to: Operand,
    tree: &DomTree,
    root: InstId,
    should_replace: impl Fn(&Function, Use, Operand) -> bool,
) -> usize {
    let dominates_and_should_replace = |function: &Function, at: Use| {
        dominates_instruction_use(tree, function, root, at) && should_replace(function, at, to)
    };
    replace_dominated_uses_with_template(context, function, from, to, dominates_and_should_replace)
}

// ---- Local.cpp: isInstructionTriviallyDead

/// `Instruction::willReturn`
/// ([Instruction.cpp:1367](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instruction.cpp#L1367)).
fn will_return(
    context: &Context,
    declarations: &Declarations,
    function: &Function,
    inst: InstId,
) -> bool {
    match function.instruction(inst).opcode {
        // Volatile operations are not guaranteed to return.
        Opcode::Load { volatile: true, .. } | Opcode::Store { volatile: true, .. } => false,
        Opcode::Call(_) | Opcode::Invoke(_) => effects::states(context, declarations, function, inst, Fact::WillReturn),
        _ => true,
    }
}

/// `Instruction::mayThrow`
/// ([Instruction.cpp:1331](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instruction.cpp#L1331)).
/// DEVIATE 13.
fn may_throw(
    context: &Context,
    declarations: &Declarations,
    function: &Function,
    inst: InstId,
) -> bool {
    match function.instruction(inst).opcode {
        Opcode::Call(_) => !effects::states(context, declarations, function, inst, Fact::NoUnwind),
        Opcode::Resume | Opcode::Invoke(_) => true,
        _ => false,
    }
}

/// `isGuaranteedToTransferExecutionToSuccessor`
/// ([ValueTracking.cpp:7951](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L7951)):
/// the instruction returns to the one after it and does not unwind. DEVIATE: a
/// volatile access and a call that is not `willreturn`/`nounwind` are not; MIR
/// has no other source.
pub(crate) fn is_guaranteed_to_transfer_execution_to_successor(
    context: &Context,
    declarations: &Declarations,
    function: &Function,
    inst: InstId,
) -> bool {
    if matches!(
        function.instruction(inst).opcode,
        Opcode::Unreachable | Opcode::Resume
    ) {
        return false;
    }
    will_return(context, declarations, function, inst) && !may_throw(context, declarations, function, inst)
}

/// `Instruction::mayHaveSideEffects`
/// ([Instruction.cpp:1358](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instruction.cpp#L1358)).
pub(crate) fn may_have_side_effects(
    context: &Context,
    callees: &Callees,
    declarations: &Declarations,
    function: &Function,
    inst: InstId,
) -> bool {
    memory::of(context, callees, function, inst).writes
        || may_throw(context, declarations, function, inst)
        || !will_return(context, declarations, function, inst)
}

/// `isInstructionTriviallyDead`
/// ([Local.cpp:402](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L402)):
/// nothing reads the result and the instruction has no side effects.
pub fn is_instruction_trivially_dead(
    context: &Context,
    callees: &Callees,
    declarations: &Declarations,
    function: &Function,
    inst: InstId,
) -> bool {
    let unused = function.instruction(inst).result.is_none_or(|result| function.users(result).is_empty());
    unused && would_instruction_be_trivially_dead(context, callees, declarations, function, inst)
}

/// `wouldInstructionBeTriviallyDead`
/// ([Local.cpp:409](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L409)):
/// the instruction has no side effects, so it is dead once nothing reads it.
/// DEVIATE 7, 8, 9, 10, 11, 12.
pub fn would_instruction_be_trivially_dead(
    context: &Context,
    callees: &Callees,
    declarations: &Declarations,
    function: &Function,
    inst: InstId,
) -> bool {
    let instruction = function.instruction(inst);
    if instruction.opcode.is_terminator() {
        return false;
    }
    // We don't want the landingpad-like instructions removed by anything this
    // general.
    if matches!(instruction.opcode, Opcode::LandingPad { .. }) {
        return false;
    }
    if !will_return(context, declarations, function, inst) {
        return false;
    }
    if !may_have_side_effects(context, callees, declarations, function, inst) {
        return true;
    }
    // Special case intrinsics that "may have side effects" but can be deleted
    // when dead.
    match intrinsic_of(context, declarations, function, inst) {
        Some(Intrinsic::LifetimeStart | Intrinsic::LifetimeEnd) => match instruction.operands[1] {
            Operand::Constant(id) if context.get(id).kind == ConstantKind::Poison => true,
            // If the only uses of the alloca are lifetime intrinsics, then the
            // intrinsics are dead.
            Operand::Value(pointer) => {
                function.users(pointer).iter().all(|one| is_lifetime(context, declarations, function, one.user))
            }
            _ => false,
        },
        // Assumptions are dead if their condition is trivially true.
        Some(Intrinsic::Assume) => match instruction.operands[0] {
            Operand::Constant(id) => match context.get(id).kind {
                ConstantKind::Int(bits) => {
                    let width = context.types.int_bits(context.get(id).ty).unwrap_or(1);
                    bits & llrm_mir::context::mask(width) != 0
                }
                _ => false,
            },
            _ => false,
        },
        _ => false,
    }
}

// ---- Local.cpp: EliminateDuplicatePHINodes

/// `Instruction::isIdenticalToWhenDefined`
/// ([Instruction.cpp:1008](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instruction.cpp#L1008)),
/// of two phis. DEVIATE 14.
fn is_identical_to_when_defined(
    function: &Function,
    this: InstId,
    other: InstId,
) -> bool {
    let (this, other) = (function.instruction(this), function.instruction(other));
    if this.opcode != other.opcode || this.operands.len() != other.operands.len() || this.ty != other.ty {
        return false;
    }
    // Two phis with no operands are identical; any other pair compares its
    // values, and its blocks, which MIR keeps among the operands.
    this.operands == other.operands
}

/// `Instruction::isIdenticalTo`
/// ([Instruction.cpp:1003](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instruction.cpp#L1003)),
/// of two phis: also the same flags. DEVIATE 14.
fn is_identical_to(
    function: &Function,
    this: InstId,
    other: InstId,
) -> bool {
    is_identical_to_when_defined(function, this, other)
        && function.instruction(this).flags == function.instruction(other).flags
}

/// The value `phi` defines.
fn phi_value(
    function: &Function,
    phi: InstId,
) -> ValueId {
    function.instruction(phi).result.expect("a phi's value")
}

/// `EliminateDuplicatePHINodesNaiveImpl`
/// ([Local.cpp:1384](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L1384)):
/// every pair is compared, ignoring the flags.
fn eliminate_duplicate_phi_nodes_naive_impl(
    function: &mut Function,
    bb: BlockId,
    to_remove: &mut IdSet<InstId>,
) -> bool {
    // This implementation doesn't currently consider undef operands
    // specially.
    let phis = edges::phis(function, bb);
    let mut changed = false;
    let mut i = 0;
    while i < phis.len() {
        let pn = phis[i];
        i += 1;
        // Is there an identical PHI node in this basic block? Only the upper
        // triangle: the lower one was checked already.
        for &duplicate_pn in &phis[i..] {
            if to_remove.contains(&duplicate_pn) {
                continue;
            }
            if !is_identical_to_when_defined(function, duplicate_pn, pn) {
                continue;
            }
            // A duplicate. Replace this PHI with the base PHI.
            function.replace_value(phi_value(function, duplicate_pn), Operand::Value(phi_value(function, pn)));
            to_remove.insert(duplicate_pn);
            changed = true;
            // The RAUW can change PHIs that we already visited: start over.
            i = 0;
            break;
        }
    }
    changed
}

/// `EliminateDuplicatePHINodesSetBasedImpl`
/// ([Local.cpp:1420](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L1420)):
/// a set of the phis seen, two being one where `isIdenticalTo`.
fn eliminate_duplicate_phi_nodes_set_based_impl(
    function: &mut Function,
    bb: BlockId,
    to_remove: &mut IdSet<InstId>,
) -> bool {
    let phis = edges::phis(function, bb);
    let mut phi_set: HashMap<(TypeId, Vec<Operand>, Flags), InstId> = HashMap::default();
    let mut changed = false;
    let mut i = 0;
    while i < phis.len() {
        let pn = phis[i];
        i += 1;
        if to_remove.contains(&pn) {
            continue;
        }
        let instruction = function.instruction(pn);
        let key = (instruction.ty, instruction.operands.clone(), instruction.flags);
        match phi_set.get(&key) {
            None => {
                phi_set.insert(key, pn);
            }
            Some(&first) => {
                debug_assert!(is_identical_to(function, first, pn));
                // A duplicate. Replace this PHI with its duplicate.
                function.replace_value(phi_value(function, pn), Operand::Value(phi_value(function, first)));
                to_remove.insert(pn);
                changed = true;
                // The RAUW can change PHIs that we already visited. Start over
                // from the beginning.
                phi_set.clear();
                i = 0;
            }
        }
    }
    changed
}

/// Blocks with at most this many phis use the exhaustive search
/// (`PHICSENumPHISmallSize`).
const PHICSE_NUM_PHIS_SMALL_SIZE: usize = 32;

/// `EliminateDuplicatePHINodes(BasicBlock *, SmallPtrSetImpl<PHINode *> &)`
/// ([Local.cpp:1490](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L1490)):
/// each duplicate phi of `bb` is replaced by its first and added to
/// `to_remove`, for the caller to erase. Whether any was.
pub fn eliminate_duplicate_phi_nodes(
    function: &mut Function,
    bb: BlockId,
    to_remove: &mut IdSet<InstId>,
) -> bool {
    if edges::phis(function, bb).len() <= PHICSE_NUM_PHIS_SMALL_SIZE {
        return eliminate_duplicate_phi_nodes_naive_impl(function, bb, to_remove);
    }
    eliminate_duplicate_phi_nodes_set_based_impl(function, bb, to_remove)
}

/// `EliminateDuplicatePHINodes(BasicBlock *)`
/// ([Local.cpp:1501](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L1501)):
/// as above, and the duplicates erased.
pub fn eliminate_duplicate_phi_nodes_and_erase(
    function: &mut Function,
    bb: BlockId,
) -> bool {
    let mut to_remove = IdSet::new();
    let changed = eliminate_duplicate_phi_nodes(function, bb, &mut to_remove);
    for pn in to_remove.iter() {
        function.erase(pn).expect("a replaced phi");
    }
    changed
}

// ---- BasicBlockUtils.cpp

/// `FoldSingleEntryPHINodes`
/// ([BasicBlockUtils.cpp:195](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/BasicBlockUtils.cpp#L195)):
/// each phi of `bb` is its first incoming value, or poison if that is itself.
pub fn fold_single_entry_phi_nodes(
    context: &Context,
    function: &mut Function,
    bb: BlockId,
) -> bool {
    let phis = edges::phis(function, bb);
    for &pn in &phis {
        let value = phi_value(function, pn);
        let incoming = function.instruction(pn).operands[0];
        if incoming != Operand::Value(value) {
            function.replace_value(value, incoming);
        } else {
            let ty = function.value(value).ty;
            let poison = context.constant(Constant { ty, kind: ConstantKind::Poison });
            function.replace_value(value, Operand::Constant(poison));
        }
        function.erase(pn).expect("a replaced phi");
    }
    !phis.is_empty()
}

/// `MergeBlockIntoPredecessor`
/// ([BasicBlockUtils.cpp:237](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/BasicBlockUtils.cpp#L237)):
/// `bb`, which its predecessor alone reaches, joins the predecessor and is
/// erased. With `predecessor_with_two_successors`, the predecessor may end in
/// a conditional branch to `bb` and another block, and then branches to where
/// `bb` jumped. Whether it merged. DEVIATE 15, 16, 24.
pub fn merge_block_into_predecessor(
    context: &Context,
    function: &mut Function,
    bb: BlockId,
    predecessor_with_two_successors: bool,
) -> bool {
    // Can't merge if there are multiple predecessors, or no predecessors.
    let predecessors = function.predecessors(bb);
    if predecessors.len() != 1 {
        return false;
    }
    let pred_bb = predecessors[0];

    // Don't break self-loops.
    if pred_bb == bb {
        return false;
    }

    // Don't break unwinding instructions or terminators with other
    // side-effects.
    let pti = function.terminator(pred_bb).expect("a predecessor ends in a terminator");
    if matches!(
        function.instruction(pti).opcode,
        Opcode::Invoke(_) | Opcode::Resume
    ) {
        return false;
    }

    // Can't merge if there are multiple distinct successors.
    let successors = function.successors(pred_bb);
    if !predecessor_with_two_successors && !(successors.len() == 1 && successors[0] == bb) {
        return false;
    }

    // Currently only allow PredBB to have two predecessors, one being BB.
    // Update BI to branch to BB's only successor instead of BB.
    let mut fall_thru = None;
    if predecessor_with_two_successors {
        let branch = function.instruction(pti);
        if branch.opcode != Opcode::Br || branch.operands.len() != 3 {
            return false;
        }
        let jump = function.terminator(bb).expect("a block ends in a terminator");
        let jump = function.instruction(jump);
        if jump.opcode != Opcode::Br || jump.operands.len() != 1 {
            return false;
        }
        let Operand::Block(new_succ) = jump.operands[0] else { unreachable!("a jump's block") };
        let fall_thru_path = if branch.operands[1] == Operand::Block(bb) { 0 } else { 1 };
        fall_thru = Some((new_succ, fall_thru_path));
    }

    // Can't merge if there is PHI loop.
    for pn in edges::phis(function, bb) {
        let value = Operand::Value(phi_value(function, pn));
        if function.instruction(pn).operands.iter().step_by(2).any(|one| *one == value) {
            return false;
        }
    }

    // Begin by getting rid of unneeded PHIs.
    fold_single_entry_phi_nodes(context, function, bb);

    let sti = function.terminator(bb).expect("a block ends in a terminator");

    // Move all definitions in the successor to the predecessor...
    let moved: Vec<InstId> = function.iter_block(bb).filter(|one| *one != sti).collect();
    if fall_thru.is_none() {
        // DEVIATE 24: the pin erases `PTI` after redirecting the uses of BB.
        // Its records stand before what `BB` starts with, as the pin's IR
        // keeps them among the code, and each moved instruction takes its own
        // along; `settle_records` does that only while `PTI` still names BB as
        // the one block it enters, so it is erased first.
        function.erase(pti).expect("a jump defines nothing");
        function.move_run(&moved, pred_bb).expect("a placed instruction");
    } else {
        for inst in moved {
            function.move_to(inst, Position::Before(pti)).expect("a placed instruction");
        }
    }

    // Make all PHI nodes that referred to BB now refer to Pred as their
    // source...
    function.replace_block_uses_with(bb, pred_bb);

    if let Some((new_succ, fall_thru_path)) = fall_thru {
        // Delete the unconditional branch from BB.
        function.erase(sti).expect("a jump defines nothing");
        // Update branch in the predecessor.
        function.set_operand(pti, 1 + fall_thru_path, Operand::Block(new_succ));
    } else {
        // Move terminator instruction.
        function.move_to(sti, Position::End(pred_bb)).expect("a placed instruction");
    }

    // Finally, erase the old block.
    function.erase_block(bb).expect("an emptied block nothing names");
    true
}

// ---- Instruction.cpp: andIRFlags

/// What an instruction's flags are about: the classes `andIRFlags` tells
/// apart.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FlagClass {
    OverflowingBinary,
    Trunc,
    PossiblyExact,
    PossiblyDisjoint,
    FloatMath,
    GepWrap,
    PossiblyNonNeg,
    SameSign,
}

impl FlagClass {
    /// The flags the class carries.
    fn flags(self) -> Flags {
        let mut flags = Flags::default();
        let bits: &[Flags] = match self {
            FlagClass::OverflowingBinary | FlagClass::Trunc => &[Flags::NSW, Flags::NUW],
            FlagClass::PossiblyExact => &[Flags::EXACT],
            FlagClass::PossiblyDisjoint => &[Flags::DISJOINT],
            FlagClass::FloatMath => &[Flags::FAST],
            FlagClass::GepWrap => &[Flags::INBOUNDS, Flags::NUSW, Flags::NUW],
            FlagClass::PossiblyNonNeg => &[Flags::NNEG],
            FlagClass::SameSign => &[Flags::SAMESIGN],
        };
        bits.iter().for_each(|one| flags.insert(*one));
        flags
    }
}

/// Whether `ty` is a float, or a vector of them: `isSupportedFloatingPointType`
/// of the scalar type.
fn is_float_or_float_vector(
    context: &Context,
    ty: TypeId,
) -> bool {
    match context.types.get(ty) {
        Type::Float(_) => true,
        Type::Vector { element, .. } => matches!(context.types.get(*element), Type::Float(_)),
        _ => false,
    }
}

/// The classes `instruction` is an instance of.
fn flag_classes(
    context: &Context,
    instruction: &Instruction,
) -> Vec<FlagClass> {
    match &instruction.opcode {
        Opcode::Binary(BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Shl) => {
            vec![FlagClass::OverflowingBinary]
        }
        Opcode::Cast(CastOp::Trunc) => vec![FlagClass::Trunc],
        Opcode::Binary(BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::AShr | BinaryOp::LShr) => {
            vec![FlagClass::PossiblyExact]
        }
        Opcode::Binary(BinaryOp::Or) => vec![FlagClass::PossiblyDisjoint],
        Opcode::GetElementPtr { .. } => vec![FlagClass::GepWrap],
        Opcode::Cast(CastOp::ZExt) => vec![FlagClass::PossiblyNonNeg],
        Opcode::Cast(CastOp::UIToFP) => vec![FlagClass::PossiblyNonNeg, FlagClass::FloatMath],
        Opcode::ICmp(_) => vec![FlagClass::SameSign],
        Opcode::FNeg
        | Opcode::Binary(BinaryOp::FAdd | BinaryOp::FSub | BinaryOp::FMul | BinaryOp::FDiv | BinaryOp::FRem)
        | Opcode::Cast(CastOp::FPTrunc | CastOp::FPExt | CastOp::SIToFP)
        | Opcode::FCmp(_) => vec![FlagClass::FloatMath],
        Opcode::Phi | Opcode::Select | Opcode::Call(_) if is_float_or_float_vector(context, instruction.ty) => {
            vec![FlagClass::FloatMath]
        }
        _ => Vec::new(),
    }
}

/// The flags of `this` with those in `mask` also in `other`.
fn and_flags(
    this: Flags,
    other: Flags,
    mask: Flags,
) -> Flags {
    let mut out = Flags::default();
    // `FAST` is the seven float bits, each listed on its own as well.
    for &(flag, _) in Flags::NAMES.iter().filter(|(flag, _)| *flag != Flags::FAST) {
        let kept = this.contains(flag) && (!mask.contains(flag) || other.contains(flag));
        if kept {
            out.insert(flag);
        }
    }
    out
}

/// `Instruction::andIRFlags`
/// ([Instruction.cpp:770](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instruction.cpp#L770)):
/// of each class both instructions are in, `this` keeps the flags `other`
/// has. DEVIATE 4.
pub fn and_ir_flags(
    context: &Context,
    function: &mut Function,
    this: InstId,
    other: InstId,
) {
    let theirs = function.instruction(other);
    let (other_flags, other_classes) = (theirs.flags, flag_classes(context, theirs));
    let mine = function.instruction(this);
    let mut flags = mine.flags;
    for class in flag_classes(context, mine) {
        if other_classes.contains(&class) {
            flags = and_flags(flags, other_flags, class.flags());
        }
    }
    if flags != mine.flags {
        function.set_flags(this, flags);
    }
}

/// `Instruction::dropPoisonGeneratingFlags`
/// ([Instruction.cpp:427](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instruction.cpp#L427)):
/// the flags a poison value can come of: the wrap, exact, disjoint, nneg,
/// samesign and GEP flags, and a float's `nnan` and `ninf`. DEVIATE 4, 5.
pub fn drop_poison_generating_flags(
    context: &Context,
    function: &mut Function,
    inst: InstId,
) {
    let instruction = function.instruction(inst);
    let mut flags = instruction.flags;
    for class in flag_classes(context, instruction) {
        let dropped = if class == FlagClass::FloatMath {
            let mut poison = Flags::NNAN;
            poison.insert(Flags::NINF);
            poison
        } else {
            class.flags()
        };
        flags = and_flags(flags, Flags::default(), dropped);
    }
    if flags != instruction.flags {
        function.set_flags(inst, flags);
    }
}

// ---- Attributes: tryIntersectAttributes

/// How two attributes of a kind intersect (`IntersectAnd`, `IntersectMin`,
/// `IntersectCustom`, `IntersectPreserve` of Attributes.td).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Intersect {
    And,
    Min,
    Align,
    Memory,
    Range,
    Preserve,
}

/// Flags LLVM intersects with "and". `nocapture` is `captures(none)`, whose
/// union with itself is itself.
const INTERSECT_AND: [&str; 22] = [
    "cold",
    "dead_on_unwind",
    "hot",
    "inlinehint",
    "mustprogress",
    "noalias",
    "nocallback",
    "nocapture",
    "nofree",
    "nonnull",
    "norecurse",
    "noreturn",
    "nosync",
    "noundef",
    "nounwind",
    "readnone",
    "readonly",
    "returned",
    "speculatable",
    "willreturn",
    "writable",
    "writeonly",
];

fn attribute_name(attribute: &Attribute) -> &str {
    match attribute {
        Attribute::Flag(name) | Attribute::Int(name, _) | Attribute::Type(name, _) | Attribute::Str(name, _) => name,
        Attribute::Memory(_) => "memory",
        Attribute::Range { .. } => "range",
        Attribute::Initializes(_) => "initializes",
    }
}

/// The rule for `attribute`'s kind. An attribute LLVM does not know
/// (`noretain`, `releases`, `nearcode`, `threeway`) is a string attribute:
/// preserved.
fn intersect_rule(attribute: &Attribute) -> Intersect {
    match attribute {
        Attribute::Flag(name) if INTERSECT_AND.contains(&name.as_str()) => Intersect::And,
        Attribute::Int(name, _) if name == "dereferenceable" || name == "dereferenceable_or_null" => Intersect::Min,
        Attribute::Int(name, _) if name == "align" => Intersect::Align,
        Attribute::Memory(_) => Intersect::Memory,
        Attribute::Range { .. } => Intersect::Range,
        _ => Intersect::Preserve,
    }
}

/// `memory(...)` access words as bits: read 1, write 2.
fn access_bits(access: &str) -> u8 {
    match access {
        "none" => 0,
        "read" => 1,
        "write" => 2,
        _ => 3,
    }
}

fn access_word(bits: u8) -> &'static str {
    ["none", "read", "write", "readwrite"][bits as usize]
}

/// The union of two `memory(...)` attributes: per location, what either
/// allows (`Attr0.getMemoryEffects() | Attr1.getMemoryEffects()`).
fn memory_union(
    this: &[(Option<String>, String)],
    other: &[(Option<String>, String)],
) -> Vec<(Option<String>, String)> {
    let default = |locations: &[(Option<String>, String)]| {
        locations.iter().find(|(location, _)| location.is_none()).map_or(0, |(_, access)| access_bits(access))
    };
    let at = |locations: &[(Option<String>, String)], named: &str| {
        locations
            .iter()
            .find(|(location, _)| location.as_deref() == Some(named))
            .map_or(default(locations), |(_, access)| access_bits(access))
    };
    let rest = default(this) | default(other);
    let mut named: Vec<&str> = this.iter().chain(other).filter_map(|(location, _)| location.as_deref()).collect();
    named.sort_unstable();
    named.dedup();
    let mut out: Vec<(Option<String>, String)> = named
        .into_iter()
        .map(|location| (location, at(this, location) | at(other, location)))
        .filter(|(_, bits)| *bits != rest)
        .map(|(location, bits)| (Some(location.to_owned()), access_word(bits).to_owned()))
        .collect();
    if rest != 0 || out.is_empty() {
        out.push((None, access_word(rest).to_owned()));
    }
    out
}

/// `AttributeSet::intersectWith`
/// ([Attributes.cpp:1048](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Attributes.cpp#L1048)):
/// what both sets allow, none where an attribute only one has must be
/// preserved. DEVIATE 3.
fn attribute_set_intersect_with(
    this: &[Attribute],
    other: &[Attribute],
) -> Option<Vec<Attribute>> {
    if this.len() == other.len() && this.iter().all(|one| other.contains(one)) {
        return Some(this.to_vec());
    }
    let find = |set: &[Attribute], name: &str| set.iter().find(|one| attribute_name(one) == name).cloned();
    let mut intersected = Vec::new();
    for attr0 in this {
        // If we don't have both attributes, then fail if the attribute is
        // must-preserve or drop it otherwise.
        let Some(attr1) = find(other, attribute_name(attr0)) else {
            if intersect_rule(attr0) == Intersect::Preserve {
                return None;
            }
            continue;
        };
        match (intersect_rule(attr0), attr0, &attr1) {
            (Intersect::And, _, _) => intersected.push(attr0.clone()),
            (Intersect::Min, Attribute::Int(name, a), Attribute::Int(_, b)) => {
                intersected.push(Attribute::Int(name.clone(), *a.min(b)))
            }
            (Intersect::Align, Attribute::Int(name, a), Attribute::Int(_, b)) => {
                intersected.push(Attribute::Int(name.clone(), *a.min(b)))
            }
            (Intersect::Memory, Attribute::Memory(a), Attribute::Memory(b)) => {
                intersected.push(Attribute::Memory(memory_union(a, b)))
            }
            (Intersect::Range, _, _) => {
                if *attr0 == attr1 {
                    intersected.push(attr0.clone());
                }
            }
            // Attributes with no intersection rule. Only intersect if they are
            // equal. Otherwise fail.
            _ => {
                if *attr0 != attr1 {
                    return None;
                }
                intersected.push(attr0.clone());
                // `byval` turns the alignment into must-preserve.
                if attribute_name(attr0) == "byval" && find(this, "align") != find(other, "align") {
                    return None;
                }
            }
        }
    }
    // An attribute only `other` has: dropped, unless it must be preserved.
    let only_other = other.iter().filter(|one| find(this, attribute_name(one)).is_none());
    if only_other.into_iter().any(|one| intersect_rule(one) == Intersect::Preserve) {
        return None;
    }
    Some(intersected)
}

/// `AttributeList::intersectWith`
/// ([Attributes.cpp:1885](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Attributes.cpp#L1885)),
/// of two calls' lists: the function, return and argument sets, one by one.
#[allow(clippy::type_complexity)]
pub(crate) fn attribute_list_intersect_with(
    this: (&[Attribute], &[Attribute], &[Vec<Attribute>]),
    other: (&[Attribute], &[Attribute], &[Vec<Attribute>]),
) -> Option<(Vec<Attribute>, Vec<Attribute>, Vec<Vec<Attribute>>)> {
    let (attrs, return_attrs) =
        (attribute_set_intersect_with(this.0, other.0)?, attribute_set_intersect_with(this.1, other.1)?);
    let count = this.2.len().max(other.2.len());
    let argument_attrs = (0..count)
        .map(|index| {
            let side = |list: &[Vec<Attribute>]| list.get(index).cloned().unwrap_or_default();
            attribute_set_intersect_with(&side(this.2), &side(other.2))
        })
        .collect::<Option<Vec<_>>>()?;
    Some((attrs, return_attrs, argument_attrs))
}

/// `CallBase::tryIntersectAttributes`
/// ([InstrTypes.h:1512](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/InstrTypes.h#L1512)):
/// the call `this` gets the attributes both calls have, and says so; where
/// they do not intersect it is left as it is.
pub fn try_intersect_attributes(
    function: &mut Function,
    this: InstId,
    other: InstId,
) -> bool {
    if this == other {
        return true;
    }
    let (Opcode::Call(mine) | Opcode::Invoke(mine)) = &function.instruction(this).opcode else { return false };
    let (Opcode::Call(theirs) | Opcode::Invoke(theirs)) = &function.instruction(other).opcode else { return false };
    let intersected = attribute_list_intersect_with(
        (&mine.attrs, &mine.return_attrs, &mine.argument_attrs),
        (&theirs.attrs, &theirs.return_attrs, &theirs.argument_attrs),
    );
    let Some((attrs, return_attrs, argument_attrs)) = intersected else { return false };
    function.set_call_attributes(this, attrs, return_attrs, argument_attrs);
    true
}

// ---- Local.cpp: combineMetadata, patchReplacementInstruction

/// Whether two nodes are one: LLVM uniques non-distinct nodes. DEVIATE 1.
fn same_node(
    metadata: &[MetadataNode],
    a: MetadataId,
    b: MetadataId,
) -> bool {
    let (x, y) = (&metadata[a.0 as usize], &metadata[b.0 as usize]);
    a == b || (!x.distinct && !y.distinct && x == y)
}

/// The metadata of `kind` on `inst`.
fn metadata_of(
    function: &Function,
    inst: InstId,
    kind: &str,
) -> Option<MetadataId> {
    function.instruction(inst).metadata.iter().find(|(one, _)| one == kind).map(|(_, node)| *node)
}

/// `setMetadata`: `node` is the kind's one, or none removes it.
fn set_metadata(
    function: &mut Function,
    inst: InstId,
    kind: &str,
    node: Option<MetadataId>,
) {
    if metadata_of(function, inst, kind) == node {
        return;
    }
    function.unannotate(inst, kind);
    if let Some(node) = node {
        function.annotate(inst, kind, node);
    }
}

/// `combineMetadata`
/// ([Local.cpp:2946](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L2946)),
/// with `AAOnly` false, as `combineMetadataForCSE` has it. DEVIATE 1, 2.
fn combine_metadata(
    metadata: &[MetadataNode],
    function: &mut Function,
    k: InstId,
    j: InstId,
    does_k_move: bool,
) {
    // The one node both kinds' merge makes where the two are one: the most
    // generic range, TBAA tag, scope list and the like of a node and itself.
    let merged =
        |kmd: MetadataId, jmd: Option<MetadataId>| jmd.filter(|jmd| same_node(metadata, kmd, *jmd)).map(|_| kmd);
    let all: Vec<(String, MetadataId)> = function.instruction(k).metadata.clone();
    for (kind, kmd) in all {
        let jmd = metadata_of(function, j, &kind);
        let has_no_noundef = |function: &Function| metadata_of(function, k, "noundef").is_none();
        match kind.as_str() {
            "tbaa" | "alias.scope" | "noalias.addrspace" if does_k_move => {
                set_metadata(function, k, &kind, merged(kmd, jmd))
            }
            "tbaa" | "alias.scope" | "noalias.addrspace" => {}
            "noalias" | "mem.parallel_loop_access" | "llvm.access.group" if does_k_move => {
                set_metadata(function, k, &kind, merged(kmd, jmd))
            }
            "noalias" | "mem.parallel_loop_access" | "llvm.access.group" => {}
            "range" | "nofpclass" | "align" if does_k_move || has_no_noundef(function) => {
                set_metadata(function, k, &kind, merged(kmd, jmd))
            }
            "range" | "nofpclass" | "align" => {}
            "fpmath" => set_metadata(function, k, &kind, merged(kmd, jmd)),
            // If K moves, only keep the invariant metadata if it is present on
            // both instructions; if K does not move, its own stays valid.
            "invariant.load" | "invariant.group" if does_k_move => set_metadata(function, k, &kind, jmd),
            "invariant.load" | "invariant.group" => {}
            "nonnull" if does_k_move || has_no_noundef(function) => set_metadata(function, k, &kind, jmd),
            "nonnull" => {}
            // Merged separately below.
            "prof" | "mmra" | "memprof" | "callsite" => {}
            "callee_type" | "alloc_token" => set_metadata(function, k, &kind, merged(kmd, jmd)),
            // If K moves, it replaces J on J's path and must allow J's callees
            // as well. If K does not move, its callees remain valid.
            "callees" if does_k_move => set_metadata(function, k, &kind, merged(kmd, jmd)),
            "callees" => {}
            "dereferenceable" | "dereferenceable_or_null" if does_k_move => {
                set_metadata(function, k, &kind, merged(kmd, jmd))
            }
            "dereferenceable" | "dereferenceable_or_null" => {}
            "preserve.access.index" => {}
            "noundef" if does_k_move => set_metadata(function, k, &kind, jmd),
            "noundef" => {}
            "nontemporal" | "nosanitize" => set_metadata(function, k, &kind, jmd),
            // Preserve !mem.cache_hint only if it is present and equivalent on
            // both instructions.
            "mem.cache_hint" => {
                if jmd.is_none_or(|jmd| !same_node(metadata, kmd, jmd)) {
                    set_metadata(function, k, &kind, None)
                }
            }
            "captures" => set_metadata(function, k, &kind, merged(kmd, jmd)),
            // Two array selectors stay the arrays' own where K stands for J.
            // Dropped, the merged `inttoptr` forges a pointer into the data
            // segment (`forging`).
            DESCRIPTOR_WORD => set_metadata(function, k, &kind, jmd.map(|_| kmd)),
            // Remove unknown metadata.
            _ => set_metadata(function, k, &kind, None),
        }
    }
    // Merge prof, mmra, memprof and callsite: kept where K and J agree.
    for kind in ["mmra", "memprof", "callsite", "prof"] {
        let (kmd, jmd) = (metadata_of(function, k, kind), metadata_of(function, j, kind));
        let agree = match (kmd, jmd) {
            (Some(kmd), Some(jmd)) => same_node(metadata, kmd, jmd),
            (None, None) => true,
            _ => false,
        };
        if !agree {
            set_metadata(function, k, kind, None);
        }
    }
}

/// `combineMetadataForCSE`
/// ([Local.cpp:3122](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L3122)):
/// K's metadata is what both K and J hold, as far as K, kept for J's uses
/// too, is right on J's path. `does_k_move`: K runs where it did not.
pub fn combine_metadata_for_cse(
    metadata: &[MetadataNode],
    function: &mut Function,
    k: InstId,
    j: InstId,
    does_k_move: bool,
) {
    combine_metadata(metadata, function, k, j, does_k_move)
}

/// Whether `inst` is `extractvalue 0` of an `llvm.*.with.overflow` call:
/// `m_ExtractValue<0>(m_WithOverflowInst(..))`.
fn is_with_overflow_value(
    context: &Context,
    declarations: &Declarations,
    function: &Function,
    inst: InstId,
) -> bool {
    let instruction = function.instruction(inst);
    let Opcode::ExtractValue(indices) = &instruction.opcode else { return false };
    let Operand::Value(aggregate) = instruction.operands[0] else { return false };
    let ValueDef::Instruction(call) = function.value(aggregate).def else { return false };
    indices[..] == [0]
        && matches!(
            intrinsic_of(context, declarations, function, call),
            Some(Intrinsic::WithOverflow { .. })
        )
}

/// `patchReplacementInstruction`
/// ([Local.cpp:3194](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L3194)):
/// `repl`, which stands for `inst` from now on, is made no more restrictive
/// than `inst`: its flags, call attributes and metadata are what both have.
pub fn patch_replacement_instruction(
    context: &Context,
    declarations: &Declarations,
    metadata: &[MetadataNode],
    function: &mut Function,
    inst: InstId,
    repl: Operand,
) {
    let Operand::Value(repl) = repl else { return };
    let ValueDef::Instruction(repl_inst) = function.value(repl).def else { return };

    // Patch the replacement so that it is not more restrictive than the value
    // being replaced. When replacing the result of a llvm.*.with.overflow
    // intrinsic with an overflowing binary operator, nuw/nsw flags may no
    // longer hold.
    if flag_classes(context, function.instruction(repl_inst)).contains(&FlagClass::OverflowingBinary)
        && is_with_overflow_value(context, declarations, function, inst)
    {
        drop_poison_generating_flags(context, function, repl_inst);
    }
    // Note that if 'I' is a load being replaced by some operation, then
    // andIRFlags() would just erase all math flags from the original
    // arithmetic operation, which is clearly not wanted and not needed.
    else if !matches!(function.instruction(inst).opcode, Opcode::Load { .. }) {
        and_ir_flags(context, function, repl_inst, inst);
    }

    // Handle attributes.
    let calls = |inst: InstId| matches!(
        function.instruction(inst).opcode,
        Opcode::Call(_) | Opcode::Invoke(_)
    );
    if calls(repl_inst) && calls(inst) {
        let success = try_intersect_attributes(function, repl_inst, inst);
        debug_assert!(success, "We should not be trying to sink callbases with non-intersectable attributes");
    }

    // In general, GVN unifies expressions over different control-flow regions,
    // and so we need a conservative combination of the noalias scopes.
    combine_metadata_for_cse(metadata, function, repl_inst, inst, false);
}

/// `patchAndReplaceAllUsesWith`
/// ([GVN.cpp:2241](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L2241)):
/// `repl` patched, then read where `inst` was.
pub fn patch_and_replace_all_uses_with(
    context: &Context,
    declarations: &Declarations,
    metadata: &[MetadataNode],
    function: &mut Function,
    inst: InstId,
    repl: Operand,
) {
    patch_replacement_instruction(context, declarations, metadata, function, inst, repl);
    let result = function.instruction(inst).result.expect("a value");
    function.replace_value(result, repl);
}

// ---- Loads.cpp

/// `isPointerUseReplaceable`
/// ([Loads.cpp:875](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/Loads.cpp#L875)):
/// whether the use is in an `icmp` or `ptrtoint`, or a `phi` or `select` that
/// only feeds them. DEVIATE 17.
pub fn is_pointer_use_replaceable(
    function: &Function,
    at: Use,
    has_non_address_bits: bool,
) -> bool {
    let mut limit = 40;
    let mut worklist = vec![at.user];
    let mut visited = IdSet::new();

    while !worklist.is_empty() {
        limit -= 1;
        if limit == 0 {
            break;
        }
        let user = worklist.pop().expect("a nonempty worklist");
        if !visited.insert(user) {
            continue;
        }
        let instruction = function.instruction(user);
        if matches!(instruction.opcode, Opcode::ICmp(_)) {
            continue;
        }
        // FIXME in LLVM: the PtrToIntInst case here is not strictly correct, as
        // it changes which provenance is exposed.
        if !has_non_address_bits && matches!(instruction.opcode, Opcode::Cast(CastOp::PtrToInt)) {
            continue;
        }
        if matches!(instruction.opcode, Opcode::Phi | Opcode::Select) {
            let result = instruction.result.expect("a phi or select value");
            worklist.extend(function.users(result).iter().map(|one| one.user));
        } else {
            return false;
        }
    }

    limit != 0
}

/// `isPointerAlwaysReplaceable`
/// ([Loads.cpp:899](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/Loads.cpp#L899)):
/// whether `from` can be replaced with `to` wherever they are equal. DEVIATE
/// 17.
pub fn is_pointer_always_replaceable(
    context: &Context,
    layout: &DataLayout,
    declarations: &Declarations,
    sizes: &Sizes,
    function: &Function,
    from: Operand,
    to: Operand,
) -> bool {
    let is_null = |operand: Operand| {
        matches!(
            operand,
            Operand::Constant(id) if context.get(id).kind == ConstantKind::Null
        )
    };
    // This is not strictly correct, but we do it for now to retain important
    // optimizations.
    if is_null(to) {
        return true;
    }
    // Conversely, replacing null in the default address space with destination
    // pointer is always valid.
    if is_null(from) && pointer_address_space(context, type_of(context, function, from)) == 0 {
        return true;
    }
    // Allow replacement with dereferenceable constants. This is not strictly
    // correct, but required for vtable assumptions.
    let is_based_on_constant_global = |operand: Operand| {
        let (Operand::Constant(id), _) = valuetracking::underlying(context, layout, function, operand) else {
            return false;
        };
        let ConstantKind::Global(global) = context.get(id).kind else { return false };
        matches!(
            declarations.get(global.0 as usize).map(|one| &one.kind),
            Some(GlobalKind::Variable(variable)) if variable.constant
        )
    };
    if matches!(to, Operand::Constant(_))
        && is_ptr_or_ptr_vector(context, type_of(context, function, to))
        && valuetracking::dereferenceable(context, layout, sizes, function, to, 1)
        && is_based_on_constant_global(to)
    {
        return true;
    }
    valuetracking::underlying(context, layout, function, from).0
        == valuetracking::underlying(context, layout, function, to).0
}

/// `canReplacePointersInUseIfEqual`
/// ([Loads.cpp:924](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/Loads.cpp#L924)):
/// whether the use of a pointer `from` that equals `to` may read `to`.
pub fn can_replace_pointers_in_use_if_equal(
    context: &Context,
    layout: &DataLayout,
    declarations: &Declarations,
    sizes: &Sizes,
    function: &Function,
    at: Use,
    to: Operand,
) -> bool {
    let ty = type_of(context, function, to);
    let from = function.instruction(at.user).operands[at.index as usize];
    debug_assert_eq!(type_of(context, function, from), ty, "values must have matching types");
    // Not a pointer, just return true.
    if !is_ptr_or_ptr_vector(context, ty) {
        return true;
    }

    // Do not perform replacements in lifetime intrinsic arguments.
    if is_lifetime(context, declarations, function, at.user) {
        return false;
    }

    if is_pointer_always_replaceable(context, layout, declarations, sizes, function, from, to) {
        return true;
    }

    let spec = layout.pointer(pointer_address_space(context, ty));
    let has_non_address_bits = spec.index_bits != spec.bits;
    is_pointer_use_replaceable(function, at, has_non_address_bits)
}

/// `canReplacePointersIfEqual`
/// ([Loads.cpp:944](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/Loads.cpp#L944)):
/// whether `from`, a pointer that equals `to`, may be replaced by it
/// everywhere.
pub fn can_replace_pointers_if_equal(
    context: &Context,
    layout: &DataLayout,
    declarations: &Declarations,
    sizes: &Sizes,
    function: &Function,
    from: Operand,
    to: Operand,
) -> bool {
    debug_assert_eq!(
        type_of(context, function, from),
        type_of(context, function, to),
        "values must have matching types"
    );
    // Not a pointer, just return true.
    if !is_ptr_or_ptr_vector(context, type_of(context, function, from)) {
        return true;
    }

    is_pointer_always_replaceable(context, layout, declarations, sizes, function, from, to)
}

/// `changeToUnreachable`
/// ([Local.cpp:2543](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L2543)):
/// `inst` and what follows it in its block become an `unreachable`; the
/// successors lose one phi input for each edge. Returns how many instructions
/// went. DEVIATE: no `DomTreeUpdater`, `MemorySSAUpdater` (the caller
/// rebuilds), and `PreserveLCSSA` is false.
pub fn change_to_unreachable(
    context: &Context,
    function: &mut Function,
    inst: InstId,
) -> usize {
    let block = function.parent(inst).expect("a placed instruction");
    // Loop over all of the successors, removing BB's entry from any PHI nodes.
    let successors: Vec<BlockId> = function
        .terminator(block)
        .map(|term| {
            function
                .instruction(term)
                .operands
                .iter()
                .filter_map(|one| if let Operand::Block(to) = one { Some(*to) } else { None })
                .collect()
        })
        .unwrap_or_default();
    for successor in successors {
        function.predecessor_removed(context, successor, block, false);
    }
    // The terminator's type is `void`.
    let void = function.instruction(function.terminator(block).expect("a terminated block")).ty;
    let unreachable = function.create_instruction(Opcode::Unreachable, void, Vec::new(), Flags::default(), None);
    function.insert(unreachable, Position::Before(inst)).expect("a placed block");
    // All instructions after this are dead.
    let rest: Vec<InstId> = function.iter_block(block).skip_while(|&one| one != inst).collect();
    let removed = rest.len();
    for dead in rest {
        if let Some(result) = function.instruction(dead).result
            && !function.users(result).is_empty()
        {
            let ty = function.instruction(dead).ty;
            let poison = context.constant(Constant { ty, kind: ConstantKind::Poison });
            function.replace_all_uses_with(result, Operand::Constant(poison));
        }
        function.erase(dead).expect("its uses were replaced");
    }
    removed
}
