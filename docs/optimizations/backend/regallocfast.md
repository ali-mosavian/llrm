> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/regallocfast.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/regallocfast.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/regallocfast.rs sha256:f0390dfe03bc61708899e7063b95c68fa5d9f7ecc56bbbc4e4b19a3f05038cc2 -->

# regallocfast

`RegAllocFast`, the allocator of the pin's -O0 pipeline. Translated from
llvm/lib/CodeGen/RegAllocFast.cpp at d1106deb71cc
(<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocFast.cpp>).

## How it works
Each block is walked backwards. A value is given a register at its last
use, the register is free again at its def, and a value that may live out
of the block is stored to its slot right after each def. A register another
instruction needs (a pinned value, a clobber) displaces the value in it: the
value is reloaded right after that instruction. Values still held at the top
of the block are reloaded there. Nothing is analysed across blocks:
`may_live_out` and `may_live_in` look at the first eight uses and defs.

The body is the one `RegAlloc` prepared (pins, one class per value). Values
with a pin stand for physical registers: the use of one is `usePhysReg`, its
def `definePhysReg`/`freePhysReg`. An instruction's `clobbers` are its
register mask.

DEVIATE R1 (no number, exit 2026-11-13): the static copy hints are one level
deep (an argument's arrival, a copy out of a pinned value); `traceCopies`
follows no chain of copies (the pin follows three). A use copied into a
value already placed takes that value's register, so a chain ending in an
argument register is followed to its end.

DEVIATE R2 (no number, exit 2026-11-13): a body this does not cover is left
to the greedy allocator (`Fallback`): x87 or SSE values, segment registers,
a def narrower than its value, a pinned value that crosses blocks, a copy
group that reads what an earlier member wrote, or no register to take.

DEVIATE R3 (no number, exit 2026-11-13): no `DBG_VALUE` to move onto a
spill slot: LIR keeps `-g` as tags on instructions, which stay on the
instruction they were on.

## Used by
`allocate::RegAlloc` at -O0.

## Example
At -O0 a value that may live out of its block is stored to its slot after its
def and reloaded where the next block uses it.

```text
bb0:  %0 = mov 1            bb0:  eax = mov 1 ; store [slot0], eax
      jmp bb1         ->    bb1:  eax = load [slot0]
bb1:  use %0                      use eax
```
