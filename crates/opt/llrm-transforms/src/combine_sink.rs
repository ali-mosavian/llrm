//! Sinking one instruction to the block that is its only user's, as
//! InstCombine's driver does after popping it. Translated from LLVM's
//! `tryToSinkInstruction` and the `getOptionalSinkBlockForInst` lambda
//! ([`InstructionCombining.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L5565)).
//!
//! # How it works
//! `sink_block` names where `inst` could go: the one block every undroppable
//! user is in (a phi's user is the block its value comes from), when that block
//! is reachable, is not `inst`'s own, and is either a block whose only
//! predecessor is `inst`'s, or one that ends the function (no successors).
//! More than 32 undroppable users, or two blocks, name none.
//!
//! `sinkable` says whether `inst` may move there: not a phi, landing pad,
//! terminator or alloca; not something that may throw or not return; not a
//! convergent call; nothing that writes memory; a load or a call that reads
//! memory only into a block whose only predecessor is its own, and only when
//! nothing after it in its block writes (an `llvm.assume` does not), unless it
//! is `!invariant.load`.
//!
//! `sink` moves it before the first non-phi, non-landing-pad instruction of the
//! destination. The `llvm.assume`s that read it outside the destination
//! (`droppable_uses`) read `true` after: the driver adds them to its worklist.
//!
//! ```text
//! b0: %x = add %a, 1            b0: br %c, b1, b2
//!     br %c, b1, b2     ==>     b1: %x = add %a, 1
//! b1: ret %x                        ret %x
//! b2: ret 0                     b2: ret 0
//! ```
//!
//! MIRROR: every check and its order; the user scan; the position.
//!
//! DEVIATE, with the reason: a compare of a runtime call's flags result is
//! never sunk (the flags do not survive to another block); an instruction that
//! writes memory is never sunk (`SoleWriteToDeadLocal` needs a capture analysis
//! llrm has not translated); `isDroppable` is the `llvm.assume` call (MIR has
//! no operand bundles, so no `dereferenceable` bundle keeps a sink away);
//! `isEHPad` is `landingpad` (MIR has no funclet pads); debug records are not
//! moved (MIR has none).
//!
//! # Example
//! `%x` is read only in `b1`, whose only predecessor is `b0`:
//! ```mir
//! define i32 @f(i32 %a, i1 %c) {
//! b0:
//!   %x = add i32 %a, 1
//!   br i1 %c, label %b1, label %b2
//!
//! b1:
//!   ret i32 %x
//!
//! b2:
//!   ret i32 0
//! }
//! ```
//! After:
//! ```mir
//! define i32 @f(i32 %a, i1 %c) {
//! b0:
//!   br i1 %c, label %b1, label %b2
//!
//! b1:
//!   %x = add i32 %a, 1
//!   ret i32 %x
//!
//! b2:
//!   ret i32 0
//! }
//! ```
//!
//! From unit test `a_value_read_in_one_successor_moves_there` in
//! `combine_sink_tests.rs`.
//!
//! # Runs
//! Not a pass: called by the combine driver once it lands.
//!
//! # See also
//! `sink` (the sinking pass, which weighs frequencies).

use llrm_mir::context::Context;
use llrm_mir::edit::Position;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{BlockId, Function, InstId, Operand, Use};
use llrm_mir::opcode::Opcode;
use llrm_mir::valuetracking::Query;
use llrm_mir::valuetracking::guarantee::{may_throw, may_write_to_memory, will_return};

/// `instcombine-max-sink-users`.
const MAX_SINK_NUM_USERS: u32 = 32;

/// The block `inst` could be sunk to; `getOptionalSinkBlockForInst`.
pub fn sink_block(
    q: &Query,
    inst: InstId,
) -> Option<BlockId> {
    let function = q.function;
    let block = function.parent(inst)?;
    let value = function.instruction(inst).result?;
    let mut user_parent = None;
    let mut users = 0;
    for one in function.users(value) {
        let user = function.instruction(one.user);
        // `User::isDroppable` reads the callee only of a call.
        if matches!(user.opcode, Opcode::Call(_)) {
            llrm_mir::counts::bump(llrm_mir::counts::Event::SinkAssumeLookups);
            if q.intrinsic(one.user) == Some(Intrinsic::Assume) {
                continue;
            }
        }
        if users > MAX_SINK_NUM_USERS {
            return None;
        }
        let mut in_block = function.parent(one.user)?;
        if user.opcode == Opcode::Phi {
            let Operand::Block(from) = user.operands[one.index as usize | 1] else { unreachable!("a phi's block") };
            in_block = from;
        }
        if user_parent.is_some_and(|parent| parent != in_block) {
            return None;
        }
        user_parent = Some(in_block);
        if users == 0 {
            if in_block == block || !q.dt?.is_reachable_from_entry(in_block) {
                return None;
            }
            if function.sole_predecessor(in_block) != Some(block) && !function.successors(in_block).is_empty() {
                return None;
            }
        }
        users += 1;
    }
    user_parent
}

/// Whether `inst` may move to `dest`; the checks of `tryToSinkInstruction`.
pub fn sinkable(
    q: &Query,
    inst: InstId,
    dest: BlockId,
) -> bool {
    let function = q.function;
    let instruction = function.instruction(inst);
    // DEVIATE: a call's flags live only until the next flag-setting
    // instruction, so their compare stays beside the call.
    if llrm_mir::facts::compares_flags_result(q.context, q.globals, function, inst) {
        return false;
    }
    if matches!(
        instruction.opcode,
        Opcode::Phi | Opcode::LandingPad { .. } | Opcode::Alloca { .. }
    )
        || instruction.opcode.is_terminator()
        || may_throw(q, inst)
        || !will_return(q, inst)
    {
        return false;
    }
    if let Opcode::Call(info) = &instruction.opcode
        && (llrm_mir::memory::has(&info.attrs, "convergent") || callee_has(q, inst, "convergent"))
    {
        return false;
    }
    if may_write_to_memory(q, inst) {
        return false;
    }
    if may_read(q, inst) && !instruction.metadata.iter().any(|(kind, _)| kind == "invariant.load") {
        let Some(block) = function.parent(inst) else { return false };
        if function.sole_predecessor(dest) != Some(block) {
            return false;
        }
        let mut scan = function.next_in_block(inst);
        while let Some(one) = scan {
            if may_write_to_memory(q, one) && q.intrinsic(one) != Some(Intrinsic::Assume) {
                return false;
            }
            scan = function.next_in_block(one);
        }
    }
    true
}

/// `Instruction::mayReadFromMemory()` for what MIR has: a load, and a call
/// that is not stated to write only.
fn may_read(
    q: &Query,
    inst: InstId,
) -> bool {
    match &q.function.instruction(inst).opcode {
        Opcode::Load { .. } => true,
        Opcode::Call(info) | Opcode::Invoke(info) => {
            let reads = |attrs: &[llrm_mir::opcode::Attribute]| llrm_mir::memory::stated(attrs).reads;
            reads(&info.attrs) && callee_attrs(q, inst).is_none_or(reads)
        }
        _ => false,
    }
}

fn callee_attrs<'a>(
    q: &Query<'a>,
    call: InstId,
) -> Option<&'a [llrm_mir::opcode::Attribute]> {
    let global = llrm_mir::memory::callee(q.context, q.function, call)?;
    q.globals.get(global.0 as usize)?.function().map(|function| function.attrs.as_slice())
}

fn callee_has(
    q: &Query,
    call: InstId,
    flag: &str,
) -> bool {
    callee_attrs(q, call).is_some_and(|attrs| llrm_mir::memory::has(attrs, flag))
}

/// The uses of `inst` that `sink` to `dest` makes `true`: the `llvm.assume`s
/// outside `dest` (`dropDroppableUses`). The driver adds their users to its
/// worklist.
pub fn droppable_uses(
    q: &Query,
    inst: InstId,
    dest: BlockId,
) -> Vec<Use> {
    let Some(value) = q.function.instruction(inst).result else { return Vec::new() };
    q.function
        .users(value)
        .into_iter()
        .filter(|one| q.function.parent(one.user) != Some(dest) && q.intrinsic(one.user) == Some(Intrinsic::Assume))
        .copied()
        .collect()
}

/// Moves `inst` to the first insertion point of `dest`, after `sinkable` said
/// it may, and makes each of `droppable` (from `droppable_uses`) `true`.
pub fn sink(
    context: &Context,
    function: &mut Function,
    inst: InstId,
    dest: BlockId,
    droppable: &[Use],
) {
    let yes = Operand::Constant(context.int(context.types.int(1), 1));
    for one in droppable {
        function.set_operand(one.user, one.index as usize, yes);
    }
    let before = function
        .iter_block(dest)
        .find(|&one| !matches!(
            function.instruction(one).opcode,
            Opcode::Phi | Opcode::LandingPad { .. }
        ))
        .expect("a terminator");
    function.move_to(inst, Position::Before(before)).expect("a placed instruction");
}
