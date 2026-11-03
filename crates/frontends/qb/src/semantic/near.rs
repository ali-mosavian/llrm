//! Programs whose arrays are all in DGROUP.
//!
//! A formal reaches its array through a far pointer, the selector at +2 and
//! the adjusted offset at +0Ah, because the array it is handed may be in the
//! far heap. Where the program is all in this module and the runtime allocates
//! none of its arrays (every DIM made one a frame array, `--frame-arrays`; the
//! rest are static), each formal's array is in DGROUP too: the selector is
//! never read, and an element is `ds:origin + subscript`, as C's.

use super::tags::{Passing, Tag};
use super::Compiler;

pub(super) fn all_near(compiler: &Compiler) -> bool {
    if !compiler.options.whole_program || compiler.runtime != "llrm" || compiler.options.one_space() {
        return false;
    }
    let defined: std::collections::BTreeSet<u32> = compiler
        .functions
        .iter()
        .filter_map(|function| compiler.signatures.get(super::canonical(&function.name)).map(|one| one.symbol))
        .collect();
    compiler.functions.iter().all(|function| {
        function.blocks.iter().flat_map(|block| &block.instructions).all(|one| match &one.tag {
            Some(Tag::Allocate(shape)) => shape.frame,
            Some(Tag::Reallocate(_) | Tag::Release { .. }) => false,
            Some(Tag::Invoke { arguments }) if arguments.iter().any(|one| matches!(one, Passing::Array(_))) => function
                .calls
                .iter()
                .find(|call| call.instruction == one.id)
                .and_then(|call| call.callee)
                .is_some_and(|callee| defined.contains(&callee)),
            _ => true,
        })
    })
}
