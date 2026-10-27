> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/leaopt.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/leaopt.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/leaopt.rs sha256:c96b16b5f622730c82341835b3d20ff4ae06343ba7d2708cb44a21bafd0b6599 -->

# leaopt

Two address computations that differ only in a displacement are one:
LLVM's `X86OptimizeLEAs`
([X86OptimizeLEAs.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86OptimizeLEAs.cpp)).

## How it works
Before phi elimination, each block's `lea d, [base + index*scale + k]` are
listed by everything but `k` (`findLEAs`, `getMemOpKey`). Two steps follow.

`removeRedundantLEAs`: a later LEA of a list goes when every reader of its
value is an instruction that has the value as the base of its one address
and nowhere else (`isReplaceable`); the readers then read the earlier LEA's
value, their displacement shifted by the difference of the two.

`removeRedundantAddrCalc`, in a build for size only: a load or store whose
address is the list's address plus another displacement is made through the
LEA's value (`chooseBestLEA`: the displacement fits an int32, the closest
LEA, one before the access if there is one, one whose displacement is a
byte if another's is not). A LEA after the access is lifted to just before
it.

```text
  v1 := lea [v0+8]            v1 := lea [v0+8]
  v2 := lea [v0+24]           mov v3, [v1]
  mov v3, [v1]        ==>     mov v4, [v1+16]
  mov v4, [v2]
```

DEVIATE LO-1 (exit: displacements of a symbol or a frame cell shifted by
`Addr::plus`, owner w-6, 2026-11-09): only an address of registers and a
number is a candidate; the pin also takes a global, a constant-pool entry
or a symbol as the displacement.

DEVIATE LO-2 (exit: the wrap of a 16-bit offset proved, owner w-6,
2026-11-09): only a body of 32-bit addresses is rewritten; a displacement
moved between two 16-bit sums can change what wraps.

DEVIATE LO-3 (no number): the first step runs over the whole body before
the second runs over any block; the pin does both for one block, then the
next. A block laid out before the one whose LEA is removed sees the
second step before the readers change.

## Example
The unit tests `a_later_lea_of_the_same_address_is_the_earlier_one` and
`a_size_build_makes_an_access_through_the_lea`.

## Runs
A machine phase after `machinesink` and before `phielim`
(`flow::machine_with`), at every -O level but -O0.
