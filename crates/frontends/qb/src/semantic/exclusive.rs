//! Reference parameters nothing else reaches while the callee runs.
//!
//! `noalias` says memory the parameter reaches is touched through it alone.
//! BASIC does not promise that: `CALL f(a, a)` passes one variable twice, and
//! a SHARED variable passed by reference is also named by the callee. It is
//! provable of a parameter when the callee is internal (`--whole-program`)
//! and every call of it passes, for that parameter, a local of the caller
//! that no other argument of the call names, a variable nothing else holds
//! the address of, or a copy made for the call (an expression), which nothing
//! else names, or a parameter the caller itself has exclusive. An element, or a
//! STATIC or SHARED variable is never proven.

use std::collections::BTreeMap;

use super::tags::{Passing, Tag};
use super::{Compiler, Function};

pub(super) fn applied(compiler: &mut Compiler) {
    if !compiler.options.whole_program {
        return;
    }
    // Greatest fixpoint: a parameter a procedure passes on is exclusive in the
    // callee if it is in the caller, so a recursion proves itself.
    let mut exclusive: BTreeMap<u32, Vec<usize>> = compiler
        .functions
        .iter()
        .filter(|callee| callee.linkage == "internal")
        .map(|callee| {
            let promised = callee
                .promises
                .iter()
                .filter_map(|&(value, _)| callee.parameters.iter().position(|&one| one == value))
                .collect();
            (callee.id, promised)
        })
        .collect();
    loop {
        let mut changed = false;
        for callee in compiler.functions.iter().filter(|callee| callee.linkage == "internal") {
            let Some(symbol) = compiler.signatures.get(super::canonical(&callee.name)).map(|one| one.symbol) else {
                exclusive.insert(callee.id, Vec::new());
                continue;
            };
            let mut kept = exclusive[&callee.id].clone();
            let mut called = false;
            for caller in &compiler.functions {
                let own = exclusive.get(&caller.id).map_or(&[][..], Vec::as_slice);
                for one in caller.blocks.iter().flat_map(|block| &block.instructions) {
                    let call = caller.calls.iter().find(|call| call.instruction == one.id);
                    if call.and_then(|call| call.callee) != Some(symbol) {
                        continue;
                    }
                    called = true;
                    let Some(Tag::Invoke { arguments }) = &one.tag else {
                        kept.clear();
                        continue;
                    };
                    kept.retain(|&index| distinct(caller, own, arguments, index));
                }
            }
            if !called {
                kept.clear();
            }
            if kept.len() != exclusive[&callee.id].len() {
                exclusive.insert(callee.id, kept);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    for function in &mut compiler.functions {
        if let Some(indexes) = exclusive.get(&function.id) {
            function.no_alias.extend(indexes);
        }
    }
}

/// The index of the parameter of `caller` that the pointer `value` is.
fn parameter_of(
    caller: &Function,
    value: u32,
) -> Option<usize> {
    caller.parameters.iter().position(|&one| one == value)
}

/// Whether argument `index` is a local of `caller` that no other argument names
/// or may reach, or a parameter of `caller` that is exclusive (`own`) and that
/// no other argument passes too.
fn distinct(
    caller: &Function,
    own: &[usize],
    arguments: &[Passing],
    index: usize,
) -> bool {
    let place = match arguments.get(index) {
        Some(Passing::Variable(place)) => *place,
        // A slot made for this call: no other argument can name it.
        Some(Passing::Temporary) => return true,
        Some(Passing::Pointer(value)) => {
            return parameter_of(caller, *value).is_some_and(|at| own.contains(&at))
                && arguments.iter().enumerate().all(|(other, passed)| {
                    other == index
                        || match passed {
                            Passing::Pointer(again) => again != value,
                            Passing::Variable(_) | Passing::Value | Passing::Temporary | Passing::Array(_) => true,
                            _ => false,
                        }
                });
        }
        _ => return false,
    };
    caller.places.iter().any(|one| one.id == place && one.storage == "local")
        && arguments.iter().enumerate().all(|(at, other)| {
            at == index
                || match other {
                    Passing::Variable(again) => *again != place,
                    Passing::Value | Passing::Temporary | Passing::Array(_) => true,
                    _ => false,
                }
        })
}
