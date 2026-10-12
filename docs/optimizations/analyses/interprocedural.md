> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/interprocedural.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/interprocedural.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/interprocedural.rs sha256:1be3e73afb6cee1a808fc1b4acdc7a4181dbc3328fb7852f491114d063821e53 -->

# interprocedural

Facts that cross direct procedure boundaries: constant arguments, constant
returns, purity and termination of calls.

## How it works
A direct call is opaque to ordinary SCCP even when every return of the named
body has the same value. These functions compute that over a module (or,
with `program_*`, a whole program) and write it into the body, where the
ordinary pipeline folds the consumers.

Arguments. `current_call_constants` lists, per call or `invoke` with a known
callee, the integer-constant actuals. `constant_parameters` (and
`program_parameters`, which also reads other modules' calls) keeps, for each
`eligible` (private) body, the parameter positions where every call agrees
on one constant; a recursive call that passes the parameter on at the same
position adds no new value. `specialize_parameters` replaces the uses of
such a parameter by the constant.

Returns. `constant_returns` finds, for each exactly defined body, the one
integer every `ret` returns. `propagate_returns` replaces uses of a direct
call's result by it; the call stays, and only a separate purity proof erases
it.

```text
 caller:  %r = call @g(i32 5)        g(%x): ... ret i32 7
                                     (every call passes 5, every ret 7)
 after:   %r = call @g(i32 5)        g(%x): uses of %x become 5
          uses of %r become 7
```

Purity and termination, as stated by attributes the whole-module step
stamped on each body. `stated_pure` is the exactly defined bodies that state
no memory effect, `willreturn` and `nounwind`. `erasable` asks the same of
one call (LLVM's `wouldInstructionBeTriviallyDead`), and
`remove_dead_pure_calls` erases the calls with no used result that are
erasable. `returns_without_looping` holds when every CFG path ends in `ret`
without revisiting a block. `cannot_fault` holds when every access is
non-volatile and either in a frame object (`frameescape::framed`) or a
constant offset ([`crate::pointerfacts`](pointerfacts.md)) from an initialized near global
this module defines.

Noreturn. `noreturn_procedures` and `program_noreturn` are
[`crate::noreturn`](noreturn.md)'s greatest fixed point over the eligible bodies;
`terminal_calls` applies its cut after the calls to them.

A body is exactly defined when its linkage says the definition here is the
one that runs (LLVM's `hasExactDefinition`).

## Provides
The functions above, and the re-export of `noreturn::terminal_sites`.
Consumers: the `interprocedural` program pass and `ipacp` in
llrm-transforms.

## Needs
[`crate::effects`](../backend/effects.md) (`callee`, `states`, `writes_memory`), [`crate::cfg`](cfg.md),
[`crate::noreturn`](noreturn.md), [`crate::frameescape`](frameescape.md), [`crate::pointerfacts`](pointerfacts.md).

## See also
LLVM's IPSCCP.
