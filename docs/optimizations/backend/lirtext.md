> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/lirtext.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/lirtext.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/lirtext.rs sha256:82cff53ff01767b0244eab37a4e98e090f08ebc934944f46bdfc675d933f7dcf -->

# lirtext

LIR as text: the one printer of a body, used by every dump, check message
and test. `ISEL_DUMP=1` prints it after each machine phase;
`ISEL_DUMP=verbose` adds what the compact form leaves out.

## How it works
Per body: a banner, a `fn` line (the registers its arguments arrive in, the
registers it returns in), a `frame` line naming the frame's slots, then
each block. A block is a header (`B1  ← B0 B2  → B2 B3  loop  ×8.0`) and
its instructions, set in fixed columns: its position in the block, the
destination, `=`, the opcode with its width, the operands, then `; notes`.
Blocks are named by position and every target says its block, so no address
appears: two dumps of the same body differ only where the instructions do.
A block is indented four spaces for each loop around it, so a loop is a
shape on the page; its header says `loop` and, for an innermost loop whose
trip count is known, the count. Operand and instruction text is
`llrm_lir::text`.

A note is printed only where it says something: `spill` and `reload` for
frame traffic, `copy` for a phi's parallel copy, `remat`, `frame` for a
frame adjustment, `N% taken` on a branch whose odds isel stated, `elided`
for an operation selection removed that still carries a value (`v5 =
elided v2`). A call lists its clobbers only as a difference from the
body's most common set, which the `fn` line states. Verbose adds what an
instruction needs and gives in registers, the source bytes it covers, its
line, and each block's live-in and live-out values.

## Example
The loop of a Fibonacci function, after the allocator:

```lir
═══ _f@3 · RegAlloc · m16 ═══════════════════════════════════════════════════════
fn _f@3(v5@ax) -> @eax

B0  entry  → B1                                                    ×1.0
  0  bx      = mov.16     1
  1  cx      = mov.16     0
  2           jmp         B1

    B1  ← B0 B3  → B3 B2  loop                                     ×32.0
      3  ax      = sub.16     ax, 1
      4           cmp.16      ax, -1
      5           jne         B3 else B2                        ; 97% taken
```

## Used by
`assemble` after each machine phase when `ISEL_DUMP` is set, the checks of
`verify` to name a place, and the allocator's tests.
