> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/regallocfast.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/regallocfast.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/regallocfast.rs sha256:ed74d4ccbeb43e59874fe472173686273190023c81f2daa09522d0c25a8b31bd -->

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

DEVIATE R1 (no number, exit 2026-11-13): no copy hints (`traceCopies`, the
hint of a copy into a pinned value): they only save a `mov`.

DEVIATE R2 (no number, exit 2026-11-13): a body this does not cover is left
to the greedy allocator (`Fallback`): x87 or SSE values, segment registers,
a def narrower than its value, a pinned value that crosses blocks, a copy
group that reads what an earlier member wrote, or no register to take.

DEVIATE R3 (no number, exit 2026-11-13): no `DBG_VALUE` to move onto a
spill slot: LIR keeps `-g` as tags on instructions, which stay on the
instruction they were on.

## Used by
`allocate::RegAlloc` at -O0.
