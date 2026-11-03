//! Reference parameters nothing else reaches while the callee runs.
//!
//! `noalias` says memory the parameter reaches is touched through it alone.
//! BASIC does not promise that: `CALL f(a, a)` passes one variable twice, and
//! a SHARED variable passed by reference is also named by the callee. It is
//! provable of a parameter when the callee is internal (`--whole-program`)
//! and every call of it passes, for that parameter, a local of the caller
//! that no other argument of the call names, a variable nothing else holds
//! the address of. An element, a pointer, a copy or a STATIC or SHARED
//! variable is never proven.

use super::tags::{Passing, Tag};
use super::{Compiler, Function};

pub(super) fn applied(compiler: &mut Compiler) {
    if !compiler.options.whole_program {
        return;
    }
    let proven: Vec<(u32, Vec<usize>)> = compiler
        .functions
        .iter()
        .filter(|callee| callee.linkage == "internal")
        .filter_map(|callee| {
            let symbol = compiler.signatures.get(super::canonical(&callee.name))?.symbol;
            let mut exclusive: Vec<usize> = callee
                .promises
                .iter()
                .filter_map(|&(value, _)| callee.parameters.iter().position(|&one| one == value))
                .collect();
            let mut called = false;
            for caller in &compiler.functions {
                for one in caller.blocks.iter().flat_map(|block| &block.instructions) {
                    let call = caller.calls.iter().find(|call| call.instruction == one.id);
                    if call.and_then(|call| call.callee) != Some(symbol) {
                        continue;
                    }
                    called = true;
                    let Some(Tag::Invoke { arguments }) = &one.tag else {
                        return None;
                    };
                    exclusive.retain(|&index| distinct_local(caller, arguments, index));
                }
            }
            (called && !exclusive.is_empty()).then(|| (callee.id, exclusive))
        })
        .collect();
    for function in &mut compiler.functions {
        if let Some((_, indexes)) = proven.iter().find(|(id, _)| *id == function.id) {
            function.no_alias.extend(indexes);
        }
    }
}

/// Whether argument `index` is a local of `caller` that no other argument names
/// or may reach.
fn distinct_local(
    caller: &Function,
    arguments: &[Passing],
    index: usize,
) -> bool {
    let Some(Passing::Variable(place)) = arguments.get(index) else {
        return false;
    };
    caller.places.iter().any(|one| one.id == *place && one.storage == "local")
        && arguments.iter().enumerate().all(|(at, other)| {
            at == index
                || match other {
                    Passing::Variable(again) => again != place,
                    Passing::Value | Passing::Temporary | Passing::Array(_) => true,
                    _ => false,
                }
        })
}
