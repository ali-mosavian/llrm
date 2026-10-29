> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/peephole.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/peephole.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/peephole.rs sha256:73d34b184d2039d12a33eeb312df3149b93152052eff0ab168cdccc805b1ef1a -->

# peephole

Local rewrites of allocated LIR that need the final physical registers.

## How it works
The phase is one fixed sequence of sub-passes over the body. Most are
generated rule groups: `peephole.peep` of the target (`llrm-x86`, plus
`llrm-x86-m16` or `llrm-x86-m32`) lists the groups, each with a window walk,
`match` patterns, `when` guards and a `rewrite`; `build.rs` turns them into
the matchers under `peep/`, bound to the target as `peep::Rules`. The rest
are hand-written functions in this file or their own modules. A window skips
meta instructions, so a source-map marker between two instructions never
stops a rewrite.

What the passes know is physical: liveness of register lanes and flag lanes
(`lanes.rs`, `effects.rs`) and the CPU's prices (`Profile`). A rewrite fires
only when the lanes and flags it changes are dead, and, where the target has
a choice, when the profile prices it no dearer in cycles or bytes.

The order is deliberate. Register renaming and copy propagation come first,
so every later pass sees fewer copies. Sinking of copies and spill stores
follows the reload forwarding, because a reload gone from the other path is
what lets the store leave it. Operand folding and address selection come
next, then the flag passes, then dead-code elimination, and last the repair
of what the 16-bit target's 32-bit addresses need.

```text
 frame copies -> machinecp -> extensions -> copysink
   -> postrasink -> forwarded reloads -> forwarded copies
   -> storecombine -> pushed constants
   -> pushes, constants, commuted, transferred, high extracts, restored
      copies, shuttles, overwritten, fused, fused calls, far loads,
      loaded segments
   -> exact addresses -> addresses (LEA) -> stepped after compare
   -> secondary bases -> increments, borrows, doubled, narrowed
   -> widened moves, zero extensions -> zeroed loads -> shared stores
   -> machine CSE -> narrowed moves, zeroes, tested, compares again,
      zero compares, waits -> machine DCE -> popped arguments
   -> frame adjust cleanup -> upper zero (non-native targets)
```

Typical rewrites, before and after:

```text
 inc edi ; cmp edi,0 ; jne L      inc edi ; jne L        (tested)
 cmp r,0 ; jcc                    or r,r ; jcc           (zero_compares)
 mov r,0 ; sbb r,0                sbb r,r                (borrows)
 mov z,y ; add z,x  (flags dead)  lea z,[y+x]            (addresses)
 mov r,[m] ; mov es,[m+2]         les r,[m]              (far_loads)
```

A sub-pass whose subject the body lacks is skipped. `needs.rs` keeps one bit
per candidate shape (a register copy, a cell read through an unheld 32-bit
register, an exact cell) per block,
and `gated` runs the pass only if some block has its bit. `LLRM_CHECK_NEEDS`
runs the skipped pass anyway and asserts it changed nothing.

When the frame has a size but the body reads or writes no spill cell, no
barrier and no stack-register operand outside the frame, the synthetic frame
reservations (`frame_adjust`) are dropped. A native target returns before
the last pass: its addresses read whole registers, where a 16-bit target's
32-bit address is the prefixed form of a 16-bit one and needs the upper half
proven zero (`upperzero.rs`).

## Example
A loop counting `n` down, after allocation:

Before:

```lir
block 0x0000
  0x0000  AX := mov 0x1  ; inserted
  0x0000  BX := mov 0x0  ; inserted
  0x0000  jmp   ; inserted
block 0x0001
  0x0004  CX := mov DX  ; inserted
  0x0004  CX := sub CX, 0x1  ; inserted
  0x0006  cmp DX, 0x0  ; inserted
  0x0006  jne   ; inserted
```

After:

```lir
block 0x0000
  0x0000  AX := mov 0x1  ; inserted
  0x0000  BX := xor BX, BX  ; inserted
  0x0000  jmp   ; inserted
block 0x0001
  0x0004  CX := mov DX  ; inserted
  0x0004  CX := dec CX  ; inserted
  0x0006  DX := or DX, DX  ; inserted
  0x0006  jne   ; inserted
```

The zero load became `xor`, the subtraction of one `dec`, and the compare
with zero `or`, which sets the flags `jne` reads as `cmp` did.

From `ISEL_DUMP=1` of a C countdown loop compiled with `llrm-cc -O2 -m16`:
the `ParallelCopy` and `Peephole` stages.

## Runs
Once per body, as the machine phase after `prologue` and before `loopslots`
(`flow::machine_with`), in every machine of every candidate allocation.
There is no flag to turn it off; at -O1 and up it runs the same rounds, as
gcc's `-fpeephole2` does.

## Needs
Allocated, two-address LIR with the frame laid out
([`crate::backend::allocate`](allocate.md), [`crate::backend::prologue`](prologue.md)). Liveness comes
from [`crate::backend::liveness`](liveness.md).

## See also
- gcc `-fpeephole2`:
  <https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/opts.cc#L638>
