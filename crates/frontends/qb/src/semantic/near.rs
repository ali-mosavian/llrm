//! Programs whose arrays are all in DGROUP.
//!
//! A formal reaches its array through a far pointer, the selector at +2 and
//! the adjusted offset at +0Ah, because the array it is handed may be in the
//! far heap. Where the program is all in this module and the runtime allocates
//! none of its arrays (every DIM made one a frame array, `--frame-arrays`; the
//! rest are static), each formal's array is in DGROUP too: the selector is
//! never read, and an element is `ds:origin + subscript`, as C's.

use std::collections::{BTreeMap, BTreeSet};

use super::tags::{Passing, Tag};
use super::{Compiler, Function};

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

/// The stack the frame arrays of all the procedures may take together, which
/// the runtime's stack leaves room for.
const STACK_FOR_ARRAYS: usize = 8192;

/// The procedures whose frame arrays the stack cannot be promised for: one that
/// can call itself (every call takes another array), then the biggest until
/// the rest, which a call chain visits once each, fit `STACK_FOR_ARRAYS`.
pub(super) fn denied(compiler: &Compiler) -> BTreeSet<String> {
    let symbol = |function: &Function| compiler.signatures.get(super::canonical(&function.name)).map(|one| one.symbol);
    let by_symbol: BTreeMap<u32, &Function> =
        compiler.functions.iter().filter_map(|function| Some((symbol(function)?, function))).collect();
    let calls = |function: &Function| -> BTreeSet<u32> {
        function.calls.iter().filter_map(|call| call.callee).filter(|callee| by_symbol.contains_key(callee)).collect()
    };
    let recurs = |function: &Function| {
        let Some(own) = symbol(function) else { return false };
        let mut seen: BTreeSet<u32> = BTreeSet::new();
        let mut work: Vec<u32> = calls(function).into_iter().collect();
        while let Some(next) = work.pop() {
            if next == own {
                return true;
            }
            if seen.insert(next) {
                work.extend(calls(by_symbol[&next]));
            }
        }
        false
    };
    let mut sizes: Vec<(usize, String)> = Vec::new();
    let mut denied = BTreeSet::new();
    for function in &compiler.functions {
        let bytes: usize = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|one| match &one.tag {
                Some(Tag::Allocate(shape)) if shape.frame => Some(compiler.frame_bytes(shape)),
                _ => None,
            })
            .sum();
        if bytes == 0 {
            continue;
        }
        let key = super::array_key(&function.name);
        if recurs(function) {
            denied.insert(key);
        } else {
            sizes.push((bytes, key));
        }
    }
    sizes.sort();
    let mut total: usize = sizes.iter().map(|(bytes, _)| bytes).sum();
    while total > STACK_FOR_ARRAYS {
        let Some((bytes, key)) = sizes.pop() else { break };
        total -= bytes;
        denied.insert(key);
    }
    denied
}
