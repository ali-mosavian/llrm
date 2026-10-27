> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/ssaupdaterbulk.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/ssaupdaterbulk.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/ssaupdaterbulk.rs sha256:c568694376cb9dfeaf9de6e773613057ad73e6a95073fa947fc01b25c9396631 -->

# ssaupdaterbulk

`SSAUpdaterBulk`: values defined in several blocks, rewritten for all their
uses at once, with a phi where definitions meet.

## How it works
A translation of LLVM `d1106deb71cc` `SSAUpdaterBulk.cpp`. The client adds
a variable for each value, says where it is available
(`SsaUpdaterBulk::add_available_value`) and which uses read it
(`SsaUpdaterBulk::add_use`); `SsaUpdaterBulk::rewrite_all_uses` then
does every variable. For one: the blocks the value is live into are found
walking back from the using blocks to the defining ones, the iterated
dominance frontier of the defining blocks pruned to those is where the phis
go ([`llrm_mir::idf`](https://github.com/ali-mosavian/llrm/blob/main/crates/ir/llrm-mir/src/idf.rs)), and each use reads the value that reaches its block,
found walking up the dominator tree to the first block that defines or
receives the value. A block the value is not defined above reads `undef`.

[`crate::ssaupdater::SsaUpdater`](ssaupdater.md) is the one-variable updater that walks the
predecessors itself; it reads the incoming blocks of a phi a block begins
with where this reads the control flow graph, and places phis by its own
dominator search, so the two can differ in where a phi stands.

## Example
`%v` defined in `b1` and `b2`, used in `b4` below them:
```mir
b0: br i1 %c, label %b1, label %b2
b1: %v1 = ...    br label %b4
b2: %v2 = ...    br label %b4
b4: use %v
```
`add_available_value(var, b1, %v1)`, `(var, b2, %v2)` and `add_use(var, the
use)`, then `rewrite_all_uses`, place `%v = phi [ %v1, %b1 ], [ %v2, %b2 ]`
in `b4` and read it.

## Deviations
- B1 `RewriteAndOptimizeAllUses` (the phi simplification and the
  deduplication passes) is not translated: nothing asks for it yet.
- B2 No value handles: `ValueHandleBase::ValueIsRAUWd` has nothing to
  notify.

## See also
[`SSAUpdaterBulk::RewriteAllUses`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SSAUpdaterBulk.cpp#L111).

## Used by
Named by `transform`.
