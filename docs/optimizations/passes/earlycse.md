> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/earlycse.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/earlycse.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/earlycse.rs sha256:3994421f029cd07cf2b86c8aa000299c736de2e463531953e5a35e1440ed4b91 -->

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
- DEVIATE `simplifyInstruction` is `simplify_instruction`, a stub with the
  signature of fold-poc's `llrm_mir::simplify::simplify_instruction`
  (InstructionSimplify.cpp): `algebraic::identity`, constants folded, a
  select on a constant or with equal arms, `x cmp x`, `icmp ne (zext i1 x),
  0`, `gep p, 0`, a phi of one value. It goes when theirs builds.
- DEVIATE flags are intersected whenever a value is reused (`combineIRFlags`
  does it only when poison-generating flags remain); a load's metadata not
  shared by both loads is dropped (`combineMetadataForCSE` merges it).
- DEVIATE a call's attributes are not intersected; calls that touch no
  memory are numbered unless they are `llvm.`/`llrm.` markers.
- DEVIATE a GEP's constant offset is taken for byte GEPs only.
- DEVIATE MemorySSA is the manager's, built once for the function as it was
  and not updated (`MSSAUpdater`); an erased store stays a def in it, which
  is only conservative. Erasures wait for the end of the walk so that what
  it asks about still exists.
- DEVIATE the clobber walk is `MemorySSA::clobbers` over the instruction's
  `MemRef`: a set of nearest defs, each of which must dominate the earlier
  access, where LLVM's walker answers one.

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
[`crate::pipeline`](pipeline.md)'s `llvm_order` lists (`earlycse@649`, `earlycse@1180`),
behind `Options::llvm_order` (`LLRM_EXP_LLVM`), off by default.
