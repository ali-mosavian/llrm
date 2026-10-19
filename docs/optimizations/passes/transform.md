> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/transform.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/transform.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/transform.rs sha256:e2b70784bf215588cb9733546ab26962fc9e28a0b4ee77df500c4098c9b7a22b -->

# transform

The helpers [`crate::gvn`](gvn.md), [`crate::dead`](dead.md), [`crate::decide`](decide.md),
and [`crate::unswitch`](unswitch.md) share: value reuse, liveness of
values, and cleanup of phis after an edge goes.

## How it works
`subexpressions` reuses a load: blocks are taken in layout order, and a
non-volatile load gets a key (`_computation`): its opcode, flags and type,
and its pointer as earlier replacements leave it. A load whose key an
earlier one has in its block, with nothing between the two that may write
its bytes (`_undisturbed`, from `memoryssa::Accesses`), reads the earlier
result and is erased; with `avoid_store_crossing` a store between them
refuses it too. Arithmetic, comparisons, addresses and casts are numbered
by `gvn::table`, as LLVM's GVN does; `joined` keys them with `_computation`
until scalar PRE replaces it.

`equal_calls` makes two calls with one callee and arguments one when the
caller's `silent` says they touch no memory: the later takes the earlier's
result where the earlier has certainly run.

`forwarded` is store-to-load forwarding across blocks: each load that
`avail::forwardable_walk` says a known value serves becomes that value.
`Crossings` answers whether a store lies on some path between the value's
definition and the load, for `avoid_store_crossing`; `Forwarding` keeps the
walk for the second numbering of one function.

```text
  before                        after
  %b = load %p                  %b = load %p
  store %v, %q                  store %v, %q   ; may not write %p's bytes
  %d = load %p                  (uses of %d read %b)
```

`live` marks, to a fixed point, every value that something staying reads;
`dead` removes the rest. `_comparison` finds the `icmp` in a block that
defines a branch's condition. `_trivial_phis` resolves joins after an edge
disappears: each phi keeps only the inputs of its remaining predecessors, a
phi with one other value is that value, and a phi with the arms of an
earlier phi in its block is that phi (LLVM's `EliminateDuplicatePHINodes`).

## Example
No honest example: these are helpers shared by passes, and none changes a
function on its own. Read [`crate::gvn`](gvn.md) and its tests for them in use.

## Provides
`subexpressions`, `equal_calls`, `forwarded` ([`crate::gvn`](gvn.md)); `live` and
`live_except` ([`crate::dead`](dead.md), [`crate::floatloop`](floatloop.md)); `_comparison`
([`crate::decide`](decide.md), [`crate::unswitch`](unswitch.md)); `_trivial_phis`
([`crate::decide`](decide.md), [`crate::unswitch`](unswitch.md)).

## Used by
[`crate::gvn`](gvn.md) calls `subexpressions`, `equal_calls` and `forwarded` on each
function it numbers; [`crate::dead`](dead.md) and [`crate::floatloop`](floatloop.md) call `live`
to decide what to delete; [`crate::decide`](decide.md) and
[`crate::unswitch`](unswitch.md) call `_trivial_phis` after they drop an edge. Not a
pass: it runs only inside those.

## Needs
`memoryssa::Accesses`, `avail` and `cfg::Dominance` (`llrm_analysis`).
