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

use super::tags::Tag;
use super::{Compiler, Function};

/// The block `__main` starts in, which no branch enters.
const ENTRY: u32 = 1;

pub(super) fn applied(compiler: &mut Compiler) {
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
