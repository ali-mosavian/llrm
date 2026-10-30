//! Descriptor words that never change after the program's DIMs.
//!
//! `!invariant.load` says a location holds one value wherever it is
//! dereferenceable, so it may be stated only where nothing can rewrite a
//! descriptor, ever: not in a callee, and not after the load is inlined into a
//! caller that DIMs again. A far array's descriptor changes at its own
//! DIM, REDIM and ERASE, and at any other far allocation, which can compact the
//! heap and rewrite every selector. The llrm runtime moves arrays only there
//! and in SHELL.
//!
//! This holds of a program that is all in this module (`--whole-program`)
//! on the llrm runtime, with: no REDIM, ERASE or SHELL; every DIM in the
//! main code's entry block, which nothing re-enters; no error handler, whose
//! RESUME could run it again. The words are then final once the entry block
//! has run, so every descriptor-word load after it is invariant. A load in
//! the entry block itself is not stated: it may precede a DIM.

use std::collections::{BTreeMap, BTreeSet};

use super::shapes::{Identity, pointers};
use super::tags::{Passing, Slot, Tag};
use super::{Compiler, Function};

/// The block `__main` starts in, which no branch enters.
const ENTRY: u32 = 1;

pub(super) fn applied(compiler: &mut Compiler) {
    shape_words(compiler);
    if !proven(compiler) {
        return;
    }
    for function in &mut compiler.functions {
        let main = function.name == "__main";
        function
            .invariant
            .extend(
                function
                    .blocks
                    .iter()
                    .filter(|block| !(main && block.id == ENTRY))
                    .flat_map(|block| &block.instructions)
                    .filter(|one| one.op == "load" && matches!(one.tag, Some(Tag::DescriptorField { .. })))
                    .map(|one| one.id),
            );
    }
}

fn proven(compiler: &Compiler) -> bool {
    if !compiler.options.whole_program || compiler.runtime != "llrm" {
        return false;
    }
    compiler.functions.iter().all(|function| {
        let main = function.name == "__main";
        function.error_handler.is_none()
            && (!main || !entered(function))
            && function.blocks.iter().all(|block| {
                block.instructions.iter().all(|one| {
                    // A callee that is not a runtime routine of the program is
                    // not seen; every call of one is an `Invoke`.
                    !matches!(one.tag, Some(Tag::Reallocate(_) | Tag::Release { .. }))
                        && one.callee.as_deref() != Some("B$SSHL")
                        && (!matches!(one.tag, Some(Tag::Allocate(_))) || (main && block.id == ENTRY))
                        && !matches!(
                            &one.tag,
                            Some(Tag::Invoke { .. }) if one.callee.is_none() && !defined(compiler, function, one.id)
                        )
                })
            })
    })
}

/// Whether a branch enters `__main`'s entry block.
fn entered(function: &Function) -> bool {
    function.blocks.iter().filter_map(|block| block.terminator.as_ref()).any(|one| one.targets.contains(&ENTRY))
}

/// Whether the call `instruction` of `function` is of a procedure this module
/// defines.
fn defined(
    compiler: &Compiler,
    function: &Function,
    instruction: u32,
) -> bool {
    function.calls.iter().find(|call| call.instruction == instruction).and_then(|call| call.callee).is_some_and(
        |symbol| {
            compiler.functions.iter().any(|one| {
                compiler.signatures.get(super::canonical(&one.name)).is_some_and(|signature| signature.symbol == symbol)
            })
        },
    )
}

/// The shape words of a local array: rank, counts and lower bounds change
/// only at that array's own DIM, REDIM and ERASE, and the data offset of a far
/// array (which B$DDIM puts at 0 of its own segment) at the DIM. They hold
/// where the procedure that DIMs it runs once: called once in the program,
/// from `__main`'s entry block, so no inlined copy sits in a loop that would
/// DIM it again on the same descriptor. The data pointer and selector are not
/// among them: any other far allocation can move the array.
///
/// The array must also be local, never passed to a call nor stored, DIM'd
/// by one DIM that no cycle repeats and that precedes the load on every path,
/// and never REDIM'd or ERASEd.
fn shape_words(compiler: &mut Compiler) {
    if !compiler.options.whole_program {
        return;
    }
    let once: BTreeSet<u32> = once_called(compiler);
    // Segmented targets only: a flat array's origin is a heap pointer, which a
    // later DIM can move.
    let segmented = !compiler.options.one_space();
    let marked: Vec<(u32, u32)> = compiler
        .functions
        .iter()
        .filter(|function| once.contains(&function.id) && function.error_handler.is_none())
        .flat_map(|function| local_words(function, segmented).into_iter().map(move |load| (function.id, load)))
        .collect();
    for function in &mut compiler.functions {
        function.invariant.extend(marked.iter().filter(|(id, _)| *id == function.id).map(|(_, load)| *load));
    }
}

/// The procedures called by exactly one call, in `__main`'s entry block, which
/// nothing re-enters; no procedure is recursive through it.
fn once_called(compiler: &Compiler) -> BTreeSet<u32> {
    let Some(main) = compiler.functions.iter().find(|function| function.name == "__main") else {
        return BTreeSet::new();
    };
    if entered(main) || main.error_handler.is_some() {
        return BTreeSet::new();
    }
    let mut calls: BTreeMap<u32, Vec<(u32, u32)>> = BTreeMap::new();
    for caller in &compiler.functions {
        for block in &caller.blocks {
            for one in &block.instructions {
                let Some(symbol) =
                    caller.calls.iter().find(|call| call.instruction == one.id).and_then(|call| call.callee)
                else {
                    continue;
                };
                calls.entry(symbol).or_default().push((caller.id, block.id));
            }
        }
    }
    compiler
        .functions
        .iter()
        .filter(|function| function.name != "__main")
        .filter_map(|function| {
            let symbol = compiler.signatures.get(super::canonical(&function.name))?.symbol;
            matches!(
                calls.get(&symbol)?.as_slice(),
                [(caller, block)] if *caller == main.id && *block == ENTRY
            )
            .then_some(function.id)
        })
        .collect()
}

/// The ids of the word loads of `function`'s local arrays that this module
/// proves invariant.
fn local_words(
    function: &Function,
    segmented: bool,
) -> Vec<u32> {
    let pointers = pointers(function);
    let local = |value: &u32| match pointers.get(value) {
        Some(identity @ Identity::Local(..)) => Some(*identity),
        _ => None,
    };
    let instructions: Vec<(u32, &super::Instruction)> =
        function.blocks.iter().flat_map(|block| block.instructions.iter().map(move |one| (block.id, one))).collect();
    // Arrays that escape, or change shape after their DIM.
    let mut barred: BTreeSet<Identity> = BTreeSet::new();
    let mut dims: BTreeMap<Identity, Vec<(u32, &super::Instruction)>> = BTreeMap::new();
    let mut released: Vec<(Identity, u32)> = Vec::new();
    for &(block, one) in &instructions {
        match &one.tag {
            Some(Tag::Reallocate(shape)) => barred.extend(local(&shape.descriptor)),
            // The procedure's own release of the array as it returns ends its
            // life: no load follows it.
            Some(Tag::Release { descriptor }) if returns(function, block) => {
                released.extend(local(descriptor).map(|identity| (identity, one.id)));
            }
            Some(Tag::Release { descriptor }) => barred.extend(local(descriptor)),
            Some(Tag::Allocate(shape)) => {
                if let Some(identity) = local(&shape.descriptor) {
                    dims.entry(identity).or_default().push((block, one));
                }
            }
            Some(Tag::Invoke { arguments }) => {
                for passing in arguments {
                    if let Passing::Array(value) = passing {
                        barred.extend(local(value));
                    }
                }
            }
            _ if matches!(one.op, "store" | "copy") => {
                for operand in &one.operands {
                    if let super::Operand::Value(value) = operand {
                        barred.extend(local(value));
                    }
                }
            }
            _ => {}
        }
    }
    let dominators = dominators(function);
    let cyclic = |block: u32| {
        function
            .blocks
            .iter()
            .find(|one| one.id == block)
            .and_then(|one| one.terminator.as_ref())
            .is_some_and(|terminator| terminator.targets.iter().any(|&next| reaches(function, next, block)))
    };
    let mut out = Vec::new();
    for &(block, one) in &instructions {
        let Some(Tag::DescriptorField { descriptor, field }) = &one.tag else {
            continue;
        };
        let Some(identity) = local(descriptor).filter(|identity| !barred.contains(identity)) else {
            continue;
        };
        let [(dim_block, dim)] = dims.get(&identity).map(Vec::as_slice).unwrap_or_default() else {
            continue;
        };
        let position = |id: u32| instructions.iter().position(|(_, other)| other.id == id);
        if released.iter().any(|&(freed, at)| freed == identity && position(at) < position(one.id)) {
            continue;
        }
        let word = match field {
            Slot::Rank | Slot::Count(_) | Slot::Lower(_) | Slot::CountOf(_) | Slot::LowerOf(_) => true,
            Slot::Origin => segmented && super::shapes::far(&dim.operands),
            Slot::Data | Slot::Selector => false,
        };
        let after = if *dim_block == block {
            instructions.iter().position(|(_, other)| other.id == dim.id)
                < instructions.iter().position(|(_, other)| other.id == one.id)
        } else {
            dominators.get(&block).is_some_and(|set| set.contains(dim_block))
        };
        if word && after && !cyclic(*dim_block) {
            out.push(one.id);
        }
    }
    out
}

/// Whether `block` of `function` ends in a return.
fn returns(
    function: &Function,
    block: u32,
) -> bool {
    function
        .blocks
        .iter()
        .find(|one| one.id == block)
        .and_then(|one| one.terminator.as_ref())
        .is_some_and(|terminator| terminator.kind == "return")
}

/// Whether `target` is reachable from `from` by branches.
fn reaches(
    function: &Function,
    from: u32,
    target: u32,
) -> bool {
    let mut seen = BTreeSet::new();
    let mut work = vec![from];
    while let Some(block) = work.pop() {
        if block == target {
            return true;
        }
        if seen.insert(block) {
            let next = function.blocks.iter().find(|one| one.id == block).and_then(|one| one.terminator.as_ref());
            work.extend(next.into_iter().flat_map(|one| one.targets.iter().copied()));
        }
    }
    false
}

/// For each block, the blocks on every path to it from the entry (itself
/// included).
fn dominators(function: &Function) -> BTreeMap<u32, BTreeSet<u32>> {
    let Some(entry) = function.blocks.first().map(|block| block.id) else {
        return BTreeMap::new();
    };
    let all: BTreeSet<u32> = function.blocks.iter().map(|block| block.id).collect();
    let mut predecessors: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for block in &function.blocks {
        for target in block.terminator.iter().flat_map(|one| &one.targets) {
            predecessors.entry(*target).or_default().push(block.id);
        }
    }
    let mut dominated: BTreeMap<u32, BTreeSet<u32>> =
        all.iter().map(|&id| (id, if id == entry { BTreeSet::from([entry]) } else { all.clone() })).collect();
    loop {
        let mut changed = false;
        for &id in all.iter().filter(|&&id| id != entry) {
            let mut meet: Option<BTreeSet<u32>> = None;
            for before in predecessors.get(&id).into_iter().flatten() {
                let theirs = &dominated[before];
                meet = Some(match meet {
                    Some(so_far) => so_far.intersection(theirs).copied().collect(),
                    None => theirs.clone(),
                });
            }
            let mut now = meet.unwrap_or_default();
            now.insert(id);
            if now != dominated[&id] {
                dominated.insert(id, now);
                changed = true;
            }
        }
        if !changed {
            return dominated;
        }
    }
}
