> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/lirtext.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/lirtext.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/lirtext.rs sha256:2d34c836aaa588bb1f312f31d58dd5c460d8182be6a14fc08d0d993530045982 -->

# lirtext

LIR as text: the one printer of a body, used by every dump, check message
and test. `ISEL_DUMP=1` prints it after each machine phase;
`ISEL_DUMP=verbose` adds what the compact form leaves out.

## How it works
Per body: a banner, a `fn` line (the registers its arguments arrive in, the
registers it returns in), a `frame` line naming the frame's slots, then
each block. A block is a header (`B1  ← B0 B2  → B2 B3  loop  ×8.0`) and
its instructions, set in fixed columns: its label (`#12`), the
destination, `=`, the opcode with its width, the operands, then `; notes`.
Blocks are named by position and every target says its block. An
instruction's label is given once, in emit order, and kept by every pass
that keeps the instruction (`Insn::label`, `Phi::label`), so two dumps of
one body differ only where the instructions do.
A block is indented four spaces for each loop around it, so a loop is a
shape on the page; its header says `loop` and, for an innermost loop whose
trip count is known, the count. Operand and instruction text is
`llrm_lir::text`.

A value is called by its name where MIR gave it one (`lsr.iv`), else `t12`;
a copy of a named value keeps the name (`LirBody::names`).

A note is printed only where it says something: `spill` and `reload` for
frame traffic, `copy` for a phi's parallel copy, `remat`, `frame` for a
frame adjustment, `N% taken` on a branch whose odds isel stated, `elided`
for an operation selection removed that still carries a value (`v5 =
elided v2`). A call lists its clobbers only as a difference from the
body's most common set, which the `fn` line states. Verbose adds what an
instruction needs and gives in registers, the source bytes it covers, its
line, and each block's live-in and live-out values.

## Example
The first blocks of a Fibonacci loop (`ISEL_DUMP=1 llrm-c -m16 -O2`), after
the allocator. `#20` is an operation selection removed, numbered when it
first appeared in a dump:

```lir
═══ _f@3 · RegAlloc · m16 ══════════════════════════════════════════════════════
fn _f@3(t5@ax) -> @eax

B0  entry  → B1                                                   ×1.0
    #24  t1         = elided      t5
     #2  bx         = mov.i2      1
     #3  cx         = mov.i2      0
     #4               jmp         B1

    B1  ← B0 B3  → B3 B2  loop                                        ×32.0
         #8  ax         = sub.i2      ax, 1
         #9               cmp.i2      ax, -1
        #10               jne         B3 else B2                            ; 97% taken
```

## Used by
`assemble` after each machine phase when `ISEL_DUMP` is set, the checks of
`verify` to name a place, and the allocator's tests.
