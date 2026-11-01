//! A landing pad that branches back into the code it guards, made enterable
//! from a `catch`.
//!
//! # Why
//! A wasm `catch` body can branch out of its `try` or back to a `loop` that
//! encloses it, never into the middle of the `try`'s body. C++ never asks for
//! it. A handler that resumes at any statement does (BASIC's `RESUME`): the pad
//! has successors that reach invokes unwinding to it without dominating them,
//! so the cycle through the pad has two entries and `FixIrreducibleControlFlow`
//! dispatches into the pad. The pin then stops in `placeTryMarker` ("Explicit
//! branch to an EH pad!"; `tests/eh/reentry.ll`).
//!
//! # How it works
//! A pad is *re-entering* when one of its successors reaches an invoke that
//! unwinds to it and does not dominate the pad. In such a function the body
//! runs inside a dispatch loop and the pad ends in a branch back to it:
//!
//! ```text
//! entry:  allocas; br start                  entry:  allocas; resume = -1; br loop
//! start:  ...                                loop:   switch resume [k: target k, else start]
//! pad:    ...; br/switch to targets          start:  ...
//!                                            pad:    ...; br/switch to re.k
//!                                            re.k:   resume = k; br loop
//! ```
//!
//! The `try` then begins at the loop's head, which dominates every invoke, and
//! the pad leaves the `catch` by a branch to the enclosing loop. A pad that
//! contains an invoke unwinding to itself (a handler that re-arms) is first cut
//! at that invoke: the rest runs in a block the loop enters.
//!
//! Values the dispatch no longer lets reach a re-entry target stay in a stack
//! slot, as `SjLjEHPrepare`'s `DemoteRegToStack` keeps them.
//!
//! O(n) per pad: one walk over the invokes, one backwards over the blocks.
//!
//! DEVIATE W20: the pin has no such pass; it asserts. Exit: none, the form is
//! outside what C++ makes.

use std::collections::BTreeSet;

use llrm_codegen::backend::ehprepare::{demote_live_into, demote_phis};
use llrm_mir::GlobalId;
use llrm_mir::TypeId;
use llrm_mir::domtree::DomTree;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Module, Operand, ValueDef};
use llrm_mir::opcode::{Flags, Opcode};

/// Each function of `module` with a re-entering landing pad, rewritten.
pub fn reentry(module: &mut Module) -> Result<(), String> {
    let found: Vec<GlobalId> = module
        .functions()
        .filter(|(_, _, function)| !function.is_declaration() && !pads(function).is_empty())
        .map(|(id, _, _)| id)
        .collect();
    for id in found {
        let name = module.global(id).name.owned().unwrap_or_default();
        function(module, id).map_err(|why| format!("@{name}: {why}"))?;
    }
    Ok(())
}

/// The blocks that start with a `landingpad`.
fn pads(function: &Function) -> Vec<BlockId> {
    function
        .layout()
        .iter()
        .copied()
        .filter(|&block| {
            function
                .iter_block(block)
                .find(|&one| function.instruction(one).opcode != Opcode::Phi)
                .is_some_and(|one| matches!(function.instruction(one).opcode, Opcode::LandingPad { .. }))
        })
        .collect()
}

/// The pad an `invoke` unwinds to.
fn unwind_of(
    function: &Function,
    inst: InstId,
) -> Option<BlockId> {
    let instruction = function.instruction(inst);
    if !matches!(instruction.opcode, Opcode::Invoke(_)) {
        return None;
    }
    match instruction.operands.iter().rev().nth(1) {
        Some(&Operand::Block(pad)) => Some(pad),
        _ => None,
    }
}

fn function(
    module: &mut Module,
    id: GlobalId,
) -> Result<(), String> {
    let mut b = module.builder(id);
    let void = b.context.types.void();
    for pad in pads(b.function) {
        cut_at_invoke(b.function, void, pad)?;
    }
    let reentering = reentering_pads(b.function);
    if reentering.is_empty() {
        return Ok(());
    }
    let entry = b.function.entry().ok_or("a function with no entry")?;
    let first =
        b.function.iter_block(entry).find(|&one| !matches!(b.function.instruction(one).opcode, Opcode::Alloca { .. }));
    let tail: Vec<InstId> = first.map_or_else(Vec::new, |first| {
        let mut run = vec![first];
        run.extend(std::iter::successors(b.function.next_in_block(first), |&one| b.function.next_in_block(one)));
        run
    });
    if tail.is_empty() {
        return Err("an entry block with no code after its allocas".to_owned());
    }
    // entry, loop, start: the body moves to `start`.
    let looped = b.function.create_block(Some("eh.loop"));
    let start = b.function.create_block(Some("eh.start"));
    b.function.insert_block(looped, Some(entry))?;
    b.function.insert_block(start, Some(looped))?;
    b.function.move_run(&tail, start)?;
    for next in b.function.successors(start) {
        rename_incoming(b.function, next, entry, start);
    }
    let i32 = b.context.types.int(32);
    let slot = b.alloca(i32, "eh.resume");
    let none = b.int(32, -1);
    b.position(entry);
    b.store(none, slot, false);
    b.br(looped);
    // One re-entry block per successor of each pad, numbered across pads.
    let mut cases: Vec<(Operand, BlockId)> = Vec::new();
    let mut redirects: Vec<(BlockId, BlockId)> = Vec::new();
    for pad in &reentering {
        let terminator = b.function.terminator(*pad).ok_or("a pad with no terminator")?;
        let mut after = *pad;
        for target in b.function.successors(*pad) {
            let number = cases.len();
            let reenter = b.function.create_block(Some(&format!("eh.re{number}")));
            b.function.insert_block(reenter, Some(after))?;
            after = reenter;
            let value = b.int(32, number as i128);
            b.position(reenter);
            b.store(value, slot, false);
            b.br(looped);
            for index in 0..b.function.instruction(terminator).operands.len() {
                if b.function.instruction(terminator).operands[index] == Operand::Block(target) {
                    b.function.set_operand(terminator, index, Operand::Block(reenter));
                }
            }
            cases.push((value, target));
            redirects.push((*pad, target));
        }
    }
    b.position(looped);
    let next = b.load(i32, slot, false, "eh.next");
    b.switch(next, start, &cases);
    // Values and phis the loop now stands between.
    let dom = DomTree::of(b.function);
    let reaches = |function: &Function, value| match function.value(value).def {
        ValueDef::Argument(_) => true,
        ValueDef::Instruction(def) => function.parent(def).is_some_and(|block| dom.dominates(block, looped)),
    };
    for &(pad, target) in &redirects {
        let shared = redirects.iter().filter(|&&(_, other)| other == target).count() > 1;
        let phis: Vec<InstId> = b
            .function
            .iter_block(target)
            .take_while(|&one| b.function.instruction(one).opcode == Opcode::Phi)
            .collect();
        let unreachable = phis
            .iter()
            .any(
                |&phi| b.function
                    .instruction(phi)
                    .operands
                    .chunks(2)
                    .any(
                        |pair| pair[1] == Operand::Block(pad)
                            && matches!(pair[0], Operand::Value(value) if !reaches(b.function, value)),
                    ),
            );
        if unreachable || shared {
            demote_phis(b.context, b.function, target)?;
        } else {
            rename_incoming(b.function, target, pad, looped);
        }
    }
    let targets: BTreeSet<BlockId> = redirects.iter().map(|&(_, target)| target).collect();
    for target in targets {
        demote_live_into(b.context, b.function, target, &reaches)?;
    }
    Ok(())
}

/// `pad` cut before the first invoke it contains: the pad ends in a branch to
/// the block the invoke starts.
fn cut_at_invoke(
    function: &mut Function,
    void: TypeId,
    pad: BlockId,
) -> Result<(), String> {
    let Some(call) = function.iter_block(pad).find(|&one| unwind_of(function, one).is_some()) else {
        return Ok(());
    };
    let run: Vec<InstId> = std::iter::successors(Some(call), |&one| function.next_in_block(one)).collect();
    let cont = function.create_block(Some("eh.cont"));
    function.insert_block(cont, Some(pad))?;
    function.move_run(&run, cont)?;
    for next in function.successors(cont) {
        rename_incoming(function, next, pad, cont);
    }
    let branch = function.create_instruction(Opcode::Br, void, vec![Operand::Block(cont)], Flags::default(), None);
    function.insert(branch, Position::End(pad))?;
    Ok(())
}

/// The pads with a successor that reaches an invoke unwinding to them and does
/// not dominate them.
fn reentering_pads(function: &Function) -> Vec<BlockId> {
    let dom = DomTree::of(function);
    let mut found = Vec::new();
    for pad in pads(function) {
        let mut reaching: BTreeSet<BlockId> = function
            .layout()
            .iter()
            .copied()
            .filter(|&block| function.terminator(block).is_some_and(|t| unwind_of(function, t) == Some(pad)))
            .collect();
        let mut work: Vec<BlockId> = reaching.iter().copied().collect();
        while let Some(block) = work.pop() {
            for before in function.predecessors(block) {
                if before != pad && reaching.insert(before) {
                    work.push(before);
                }
            }
        }
        if function
            .successors(pad)
            .iter()
            .any(|&next| next != pad && reaching.contains(&next) && !dom.dominates(next, pad))
        {
            found.push(pad);
        }
    }
    found
}

/// The phis of `block` taking `from`'s edge now take `to`'s.
fn rename_incoming(
    function: &mut Function,
    block: BlockId,
    from: BlockId,
    to: BlockId,
) {
    let phis: Vec<InstId> =
        function.iter_block(block).take_while(|&one| function.instruction(one).opcode == Opcode::Phi).collect();
    for phi in phis {
        for index in 0..function.instruction(phi).operands.len() {
            if function.instruction(phi).operands[index] == Operand::Block(from) {
                function.set_operand(phi, index, Operand::Block(to));
            }
        }
    }
}
