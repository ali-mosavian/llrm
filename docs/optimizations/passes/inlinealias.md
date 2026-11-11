> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/inlinealias.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/inlinealias.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/inlinealias.rs sha256:52c600910c4132ce66c31fbf62386bb3fcca22a76e0156446eb2b36330162ebb -->

# inlinealias

`AddAliasScopeMetadata`
([L1217](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/InlineFunction.cpp#L1217)):
the `noalias` arguments of an inlined call become alias scopes.

## How it works
Each `noalias` argument the callee uses gets a new scope in a new domain
and a `llvm.experimental.noalias.scope.decl` before the call
(`declare_scopes`). In the copy, an access whose pointers are all based on
`noalias` arguments is in the scopes of those it is based on, and an access
is `!noalias` the scopes of the arguments it is not based on, unless that
argument may have been captured before it (`scoped`).

## Example
`f(noalias %p, %q)` with `store %p` and `load %q` inlined: the store is in
`p`'s scope and the load is `!noalias` it.

## Deviations
- names are the callee's where the module's declarations are at hand, else
  generic.
- IA2 (owner w9, exit 2026-12-15): `CalleeAAR` is the call's and the
  callee's attributes and `GlobalsAA`'s function effects (where the walk has
  them), not a query of every alias analysis of the callee.
- A copy is never simplified while it is made (`isSimplified`).

## Provides
`Scopes`, `declare_scopes`, `scoped`.

## Needs
[`crate::inlinemetadata`](inlinemetadata.md) for the lists, `llrm_analysis::capturetracking`.
