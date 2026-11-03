//! A by-reference copy-out nothing can change: BASIC passes a far array's
//! element to a near formal through a slot, and stores the slot back after the
//! call.
//!
//! # How it works
//! A store to an element of a BASIC array (an address built on an
//! `inttoptr` the frontend marks `!llrm.descriptor`) of the value just loaded
//! from that same address, with one direct call in between, is dropped when
//!
//! - the call hands the callee a slot the value was copied into, in a parameter
//!   that is `readonly nocapture`, and
//! - the callee writes no memory but its own locals and further copy-outs of
//!   this kind, and calls only callees that do the same: a set found as the
//!   greatest fixpoint, so a recursive traversal belongs to it.
//!
//! No write but a copy-out of an unchanged slot can then happen during the
//! call, so the element is what the load read and the store writes it back
//! unchanged: the result is BC's own, whatever the callee does.
//!
//! ```text
//!  %v = load i16, ptr addrspace(1) %e        %v = load i16, ptr addrspace(1) %e
//!  store i16 %v, ptr %slot                   store i16 %v, ptr %slot
//!  call @f(ptr %slot)  ; readonly nocapture  call @f(ptr %slot)
//!  store i16 %v, ptr addrspace(1) %e   ==>
//! ```
//!
//! The store is what kept a body that only reads its arguments
//! `memory(readwrite)`, so its descriptor pointer was reloaded after every call
//! and [`crate::argpromotion`] refused it.
//!
//! # Runs
//! In the interprocedural step, before [`crate::argpromotion`], once the
//! callees' parameters are stated `readonly nocapture`; the memory effects are
//! stated again after. Not at -O0.

use std::collections::BTreeSet;

use llrm_mir::context::GlobalId;
use llrm_mir::facts::Facts;
use llrm_mir::memory;
use llrm_mir::module::{DESCRIPTOR_WORD, Function, InstId, Module, Operand, ValueDef};
use llrm_mir::opcode::{CastOp, Opcode};

/// The copy-outs dropped from every function of `module`; the functions
/// changed.
pub fn elided(module: &mut Module) -> Vec<GlobalId> {
    let members = readers(module);
    let mut found: Vec<(GlobalId, Vec<InstId>)> = Vec::new();
    for (id, _, function) in module.functions().filter(|(_, _, function)| !function.is_declaration()) {
        let dead = copy_outs(module, function, &members);
        if !dead.is_empty() {
            found.push((id, dead));
        }
    }
    let mut changed = Vec::new();
    for (id, dead) in found {
        let (_, function) = crate::interprocedural::function_mut(module, id);
        if function.erase_all(&dead).is_ok() {
            changed.push(id);
        }
    }
    changed
}

/// The bodies that write no memory but their own locals and copy-outs of
/// slots, calling only each other: every body, less any with another write or
/// call, until none is left out.
fn readers(module: &Module) -> BTreeSet<GlobalId> {
    let mut members: BTreeSet<GlobalId> =
        module.functions().filter(|(_, _, function)| !function.is_declaration()).map(|(id, _, _)| id).collect();
    let callees = memory::callees(module);
    loop {
        let before = members.len();
        let kept: BTreeSet<GlobalId> = members
            .iter()
            .copied()
            .filter(|&id| {
                module.global(id).function().is_some_and(|f| writes_only_copy_outs(module, &callees, f, &members))
            })
            .collect();
        members = kept;
        if members.len() == before {
            return members;
        }
    }
}

fn writes_only_copy_outs(
    module: &Module,
    callees: &memory::Callees,
    function: &Function,
    members: &BTreeSet<GlobalId>,
) -> bool {
    let blocks = blocks_of(function);
    for (_, insts) in &blocks {
        for (at, &inst) in insts.iter().enumerate() {
            if !memory::of(&module.context, callees, function, inst).writes {
                continue;
            }
            let instruction = function.instruction(inst);
            let allowed = match &instruction.opcode {
                Opcode::Store { volatile: false, .. } => {
                    instruction.operands.get(1).is_some_and(|&address| in_a_local(function, address))
                        || is_copy_out(module, function, insts, at, inst, members)
                }
                Opcode::Call(_) => {
                    memory::callee(&module.context, function, inst).is_some_and(|callee| members.contains(&callee))
                }
                _ => false,
            };
            if !allowed {
                return false;
            }
        }
    }
    true
}

/// Whether `address` is in one of `function`'s own allocas.
pub(crate) fn in_a_local(
    function: &Function,
    address: Operand,
) -> bool {
    let mut at = address;
    for _ in 0..16 {
        let Operand::Value(value) = at else { return false };
        let ValueDef::Instruction(inst) = function.value(value).def else { return false };
        let instruction = function.instruction(inst);
        match &instruction.opcode {
            Opcode::Alloca { .. } => return true,
            Opcode::GetElementPtr { .. } => match instruction.operands.first() {
                Some(&next) => at = next,
                None => return false,
            },
            _ => return false,
        }
    }
    false
}

fn blocks_of(function: &Function) -> Vec<(llrm_mir::module::BlockId, Vec<InstId>)> {
    let mut blocks: Vec<(llrm_mir::module::BlockId, Vec<InstId>)> = Vec::new();
    for (block, inst) in function.walk() {
        match blocks.last_mut() {
            Some((last, insts)) if *last == block => insts.push(inst),
            _ => blocks.push((block, vec![inst])),
        }
    }
    blocks
}

/// The stores of `function` that write a slot back and can change nothing.
fn copy_outs(
    module: &Module,
    function: &Function,
    members: &BTreeSet<GlobalId>,
) -> Vec<InstId> {
    let mut dead = Vec::new();
    for (_, insts) in &blocks_of(function) {
        for (at, &store) in insts.iter().enumerate() {
            if is_copy_out(module, function, insts, at, store, members) {
                dead.push(store);
            }
        }
    }
    dead
}

fn is_copy_out(
    module: &Module,
    function: &Function,
    insts: &[InstId],
    at: usize,
    store: InstId,
    members: &BTreeSet<GlobalId>,
) -> bool {
    let instruction = function.instruction(store);
    if !matches!(instruction.opcode, Opcode::Store { volatile: false, .. }) {
        return false;
    }
    let (Some(&Operand::Value(value)), Some(&address)) = (instruction.operands.first(), instruction.operands.get(1))
    else {
        return false;
    };
    if !on_array_element(function, address) {
        return false;
    }
    // The load the value comes from, of the same address, earlier in the block.
    let ValueDef::Instruction(load) = function.value(value).def else { return false };
    let Some(from) = insts[..at].iter().position(|&one| one == load) else { return false };
    let reads = function.instruction(load);
    if !matches!(reads.opcode, Opcode::Load { volatile: false, .. }) || reads.operands.first() != Some(&address) {
        return false;
    }
    // Between the two: one direct call and nothing else that writes.
    let between = &insts[from + 1..at];
    let is_call = |one: InstId| matches!(
        function.instruction(one).opcode,
        Opcode::Call(_) | Opcode::Invoke(_)
    );
    let calls: Vec<InstId> = between.iter().copied().filter(|&one| is_call(one)).collect();
    let [call] = calls[..] else { return false };
    // The other arguments' slots are filled between the two: stores to locals
    // are not writes anyone else sees.
    let foreign = |one: InstId| {
        let instruction = function.instruction(one);
        match instruction.opcode {
            Opcode::Store { .. } => !instruction.operands.get(1).is_some_and(|&address| in_a_local(function, address)),
            Opcode::Atomic(_) => true,
            _ => false,
        }
    };
    if between.iter().any(|&one| foreign(one)) {
        return false;
    }
    let invoked = function.instruction(call);
    if !matches!(invoked.opcode, Opcode::Call(_)) {
        return false;
    }
    let Some(callee) = memory::callee(&module.context, function, call) else { return false };
    if !members.contains(&callee) {
        return false;
    }
    let Some(target) = module.global(callee).function() else { return false };
    // The slot: an alloca the value was stored to before the call and the call
    // is the only other user of, passed to a parameter the callee only reads.
    for (index, argument) in invoked.operands.iter().enumerate().take(target.parameters().len()) {
        let Operand::Value(slot) = *argument else { continue };
        let ValueDef::Instruction(origin) = function.value(slot).def else { continue };
        if !matches!(function.instruction(origin).opcode, Opcode::Alloca { .. }) {
            continue;
        }
        let facts = Facts::of(target.parameter_attrs.get(index).map_or(&[][..], |one| &one[..]));
        if !(facts.read_only() && facts.no_capture()) {
            continue;
        }
        let copied_in = between
            .iter()
            .any(
                |&one| {
                    let candidate = function.instruction(one);
                    matches!(candidate.opcode, Opcode::Store { volatile: false, .. })
                        && candidate.operands.first() == Some(&Operand::Value(value))
                        && candidate.operands.get(1) == Some(&Operand::Value(slot))
                },
            );
        let mut only_ours = true;
        for user in function.users(slot) {
            only_ours &= user.user == call
                || (matches!(function.instruction(user.user).opcode, Opcode::Store { .. }) && user.index == 1);
        }
        if copied_in && only_ours {
            return true;
        }
    }
    false
}

/// Whether `address` is built on the segment the frontend made from an array
/// descriptor: an element of a BASIC array.
fn on_array_element(
    function: &Function,
    address: Operand,
) -> bool {
    let mut at = address;
    for _ in 0..16 {
        let Operand::Value(value) = at else { return false };
        let ValueDef::Instruction(inst) = function.value(value).def else { return false };
        let instruction = function.instruction(inst);
        match &instruction.opcode {
            Opcode::GetElementPtr { .. } | Opcode::Cast(CastOp::AddrSpaceCast) => match instruction.operands.first() {
                Some(&next) => at = next,
                None => return false,
            },
            Opcode::Cast(CastOp::IntToPtr) => {
                return instruction.metadata.iter().any(|(kind, _)| kind == DESCRIPTOR_WORD);
            }
            _ => return false,
        }
    }
    false
}
