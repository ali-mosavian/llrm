> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/interprocedural.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/interprocedural.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/interprocedural.rs sha256:46a73bd0d673dc037904df6ad00b8963761757ae7e76f43e7585bb608fc04a56 -->

# interprocedural

Finds what the two sides of a direct call make certain: constant call
actuals, constant returns, purity and termination. Intra-procedural passes
cannot see across a call, so these facts are written into the bodies for
them to fold.

## How it works
A direct call is opaque to ordinary SCCP even when every return of the named
body has the same value. These functions compute that over a module (or,
with `program_*`, a whole program) and write it into the body, where the
ordinary pipeline folds the consumers.

Arguments. `current_call_constants` lists, per call or `invoke` with a known
callee, the integer-constant actuals (what `ipacp` clones for).

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
erasable.

Noreturn. `noreturn_procedures` and `program_noreturn` are
[`crate::noreturn`](noreturn.md)'s greatest fixed point over the eligible bodies;
`terminal_calls` applies its cut after the calls to them.

A body is exactly defined when its linkage says the definition here is the
one that runs (LLVM's `hasExactDefinition`).

## Example
`constant_returns` over a module of two private bodies:

```mir
define internal i16 @yes() {
b1:
  ret i16 37
}

define internal i16 @no(i1 %c) {
b1:
  br i1 %c, label %b2, label %b3

b2:
  ret i16 37

b3:
  ret i16 38
}
```

| body   | constant return                |
|--------|--------------------------------|
| `@yes` | 37                             |
| `@no`  | none (its two `ret`s disagree) |

From unit test `test_module_constant_returns_require_every_exit_to_agree` in
`interprocedural_tests.rs`.

## Provides
The functions above, and the re-export of `noreturn::terminal_sites`.
Consumers: the `interprocedural` program pass and `ipacp` in
llrm-transforms.

## Needs
[`crate::effects`](../backend/effects.md) (`callee`, `states`, `writes_memory`), [`crate::cfg`](cfg.md),
[`crate::noreturn`](noreturn.md), [`crate::frameescape`](frameescape.md), [`crate::pointerfacts`](pointerfacts.md).

## See also
LLVM's IPSCCP.
