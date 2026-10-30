//! Loops that reach no far allocation.
//!
//! The far heap moves arrays at DIM, REDIM, ERASE and SHELL only (the
//! llrm runtime's `fh_alloc` and `qb_shell_reclaim`), so a far array's
//! descriptor words are fixed for as long as a loop runs that reaches none:
//! not in its blocks, and not through the routines it calls. A routine
//! allocates if it does so itself, calls one that does, or calls a routine
//! this module does not define. Any error handler in the program voids the
//! fact: its RESUME may run past the allocation it handled.
//!
//! The llrm runtime on the segmented target only. The MS runtime may move a
//! descriptor's location at any call (`tags.rs`); a flat target keeps every
//! array in the local heap, which FRE and CLOSE slide. So does a near array
//! on the segmented target: unless every DIM is far, CLOSE, FRE(string) and
//! FRE(number) (`file_close`, `B_FRSD`, `B_FRI2`: `lh_compact`) allocate.

use std::collections::{BTreeMap, BTreeSet};

use super::invariance::{all_far, dominators, reaches};
use super::tags::Tag;
use super::{Compiler, Function};

pub(super) fn applied(compiler: &mut Compiler) {
    if compiler.runtime != "llrm"
        || compiler.options.one_space()
        || compiler.functions.iter().any(|function| function.error_handler.is_some())
    {
        return;
    }
    let near_moves = !all_far(compiler);
    let allocating = allocating(compiler, near_moves);
    let defined: BTreeSet<u32> = compiler
        .functions
        .iter()
        .filter_map(|function| compiler.signatures.get(super::canonical(&function.name)).map(|one| one.symbol))
        .collect();
    let quiet =
        |callee: Option<u32>| callee.is_some_and(|symbol| defined.contains(&symbol) && !allocating.contains(&symbol));
    let found: Vec<(u32, BTreeSet<u32>)> =
        compiler.functions.iter().map(|function| (function.id, loops(function, near_moves, quiet))).collect();
    for (function, latches) in found {
        if let Some(one) = compiler.functions.iter_mut().find(|one| one.id == function) {
            one.quiet_loops = latches;
        }
    }
}

/// The symbols of the routines that may allocate, directly or through a call.
fn allocating(
    compiler: &Compiler,
    near_moves: bool,
) -> BTreeSet<u32> {
    let symbol = |function: &Function| compiler.signatures.get(super::canonical(&function.name)).map(|one| one.symbol);
    let mut out: BTreeSet<u32> = BTreeSet::new();
    let mut calls: BTreeMap<u32, Vec<Option<u32>>> = BTreeMap::new();
    for function in &compiler.functions {
        let Some(own) = symbol(function) else { continue };
        let mut direct = false;
        for one in function.blocks.iter().flat_map(|block| &block.instructions) {
            direct |= allocates(one, near_moves);
            if matches!(one.tag, Some(Tag::Invoke { .. })) {
                let callee = function.calls.iter().find(|call| call.instruction == one.id).and_then(|call| call.callee);
                calls.entry(own).or_default().push(callee);
            }
        }
        if direct {
            out.insert(own);
        }
    }
    let defined: BTreeSet<u32> = compiler.functions.iter().filter_map(symbol).collect();
    // An unknown callee may do anything.
    for (caller, callees) in &calls {
        if callees.iter().any(|callee| callee.is_none_or(|one| !defined.contains(&one))) {
            out.insert(*caller);
        }
    }
    loop {
        let before = out.len();
        for (caller, callees) in &calls {
            if callees.iter().flatten().any(|callee| out.contains(callee)) {
                out.insert(*caller);
            }
        }
        if out.len() == before {
            return out;
        }
    }
}

/// Whether the instruction allocates, moves or frees a far array, or runs a
/// child process; with `near_moves`, whether it slides the local heap, where a
/// near array lives.
fn allocates(
    one: &super::Instruction,
    near_moves: bool,
) -> bool {
    matches!(
        one.tag,
        Some(Tag::Allocate(_) | Tag::Reallocate(_) | Tag::Release { .. })
    )
        || one.callee.as_deref() == Some("B$SSHL")
        || (near_moves && matches!(one.callee.as_deref(), Some("B$CLOS" | "B$FRSD" | "B$FRI2")))
}

/// The back-edge blocks of `function`'s loops that contain no allocating
/// instruction and call only what `quiet` says.
fn loops(
    function: &Function,
    near_moves: bool,
    quiet: impl Fn(Option<u32>) -> bool,
) -> BTreeSet<u32> {
    let dominated = dominators(function);
    let mut out = BTreeSet::new();
    for latch in &function.blocks {
        let Some(terminator) = latch.terminator.as_ref() else { continue };
        for &header in &terminator.targets {
            if !dominated.get(&latch.id).is_some_and(|set| set.contains(&header)) {
                continue;
            }
            // The natural loop of the back edge latch -> header.
            let body: Vec<u32> = function
                .blocks
                .iter()
                .map(|block| block.id)
                .filter(|&block| {
                    block == header || (reaches(function, block, latch.id) && dominated[&block].contains(&header))
                })
                .collect();
            let calm = body
                .iter()
                .all(
                    |id| function
                        .blocks
                        .iter()
                        .find(|block| block.id == *id)
                        .is_some_and(
                            |block| block
                                .instructions
                                .iter()
                                .all(
                                    |one| !allocates(one, near_moves)
                                        && (!matches!(one.tag, Some(Tag::Invoke { .. }))
                                            || quiet(
                                                function
                                                    .calls
                                                    .iter()
                                                    .find(|call| call.instruction == one.id)
                                                    .and_then(|call| call.callee),
                                            )),
                                ),
                        ),
                );
            if calm {
                out.insert(latch.id);
            }
        }
    }
    out
}
