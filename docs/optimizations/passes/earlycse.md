> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/earlycse.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/earlycse.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/earlycse.rs sha256:ebe6013fb3dc975c51946718ee7804809735d676c6069f87117ffcf60d7855dc -->

# earlycse

LLVM's EarlyCSE with MemorySSA (`early-cse<memssa>`): one walk down the
dominator tree, scoped tables of what is available, and a generation count
that memory operations advance.

A translation of `llvm/lib/Transforms/Scalar/EarlyCSE.cpp` at d1106deb71cc;
each function carries the permalink of the one it translates. What is not
translated is marked DEVIATE with its reason:

- DEVIATE no `invariant.start`, `guard`, `sideeffect`, `pseudoprobe`, masked
  or target memory intrinsics, memset forwarding, fences or atomics: MIR has
  none (or `fill` makes them later). `llvm.assume` is handled.
- DEVIATE flags are intersected whenever a value is reused (`combineIRFlags`
  does it only when poison-generating flags remain); a load's metadata not
  shared by both loads is dropped (`combineMetadataForCSE` merges it).
- DEVIATE a call's attributes are not intersected; a call that only reads or
  only writes memory (`memory::of`, which counts every location including
  `inaccessiblemem`) is numbered unless it is an `llvm.`/`llrm.` marker.
- DEVIATE a GEP's constant offset is taken for byte GEPs only.
- DEVIATE MemorySSA is the translated `Mssa`, built once for the function as
  it was and not updated (`MSSAUpdater`); an erased store stays a def in it,
  which is only conservative. Erasures wait for the end of the walk so that
  what it asks about still exists.
- the clobbering access is `llrm_analysis::mssa::view::Translated`'s: one
  access, the `MemoryPhi` where the nearest clobbers diverge, as LLVM's
  walker answers.
- DEVIATE (call walker) the clobbering access of a call is its defining
  access, not LLVM's call-vs-call query (the walker asks the modref of the
  call against each def): any write between two calls blocks the CSE, which
  is only conservative. It rests on the fact that llrm has no call-vs-call
  alias query until basicaa-port's S3 (an unknown callee's footprint omits
  inaccessible memory, #2219). Exit: S3 landed, then translate the walker's
  call query. The eligibility is pinned by
  `test_a_call_that_touches_inaccessible_memory_is_not_a_read_only_or_write_only_call`.

## How it works
The dominator tree is walked depth first with a scope per node: the scoped
tables hold the expressions, loads, calls and GEPs available there, each
stamped with the generation of memory when it was made. An instruction that
`simplify_instruction` folds is replaced; one equal to an available entry
is replaced by it; a load is the stored or loaded value of an earlier
access when the MemorySSA clobber query says nothing wrote between them
(`isSameMemGeneration`); a store that rewrites what an earlier one wrote
goes. Entering a branch arm records the condition as known.

## Example
Before (`test_a_commuted_operation_is_the_one_before`):
```mir
define i32 @f(i32 %x, i32 %y) {
b0:
  %a = add i32 %x, %y
  %b = add i32 %y, %x
  %c = mul i32 %a, %b
  ret i32 %c
}
```

After:
```mir
define i32 @f(i32 %x, i32 %y) {
b0:
  %a = add i32 %x, %y
  %c = mul i32 %a, %a
  ret i32 %c
}
```

## Used by
[`crate::pipeline`](pipeline.md)'s `llvm_order` lists (`earlycse@649`, `earlycse@1180`).
