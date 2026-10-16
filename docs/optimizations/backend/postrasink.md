> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/postrasink.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/postrasink.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/postrasink.rs sha256:6851260c46039e1fbe363495b7c08e8733b9ebbf8ae01d4d82ee8b98c4511a47 -->

# postrasink

Moves a register copy, or a spill store, into the one successor that needs
it.

## How it works
In a block that ends in a branch, a copy `mov d, s` whose result `d` is live
into only one successor is dead on the path to the others. The copy moves to
the head of that successor:

```text
 before                        after
 B: mov edi, edx               B: test eax, eax
    test eax, eax                 jne body
    jne body                   ret_path: (nothing reads EDI)
 ret_path: ret                 body: mov edi, edx ...
 body: ... reads edi
```

Blocks are taken once, bottom-up within a block, in reverse post-order, as
LLVM's `PostRAMachineSinking` does, so a copy that lands in a block is seen
when that block's turn comes, and later copies land ahead of earlier ones.
Liveness of register lanes is computed once for the body (`live_in`).

A copy moves only when every one of these holds: it writes lanes it does not
read; exactly one successor has its result live; that successor is entered
only from this block and by one edge; and no instruction between the copy
and the branch touches (reads or writes) the copy's result or source
(`touches`).

A spill store `mov [slot], r` (a plain store to a fixed frame cell) moves by
the same rule, with cells instead of registers: the one successor whose
reachable blocks may read the slot (`Reads`, cell liveness worked out once
for the function) receives it, provided nothing after the store in its block
may read or overwrite the cell or touch `r`. A target entered from elsewhere
too, such as a loop header, takes the store on the edge instead, in a block
of its own (a bridge, with a jump to the target) which the layout puts in
line; the branch is retargeted and the block odds are rerouted.

The point is shrink wrapping. A path that returns at once then has no copy
and no store of a saved register, and the prologue can be placed after the
test, as gcc and LLVM do. A store of a slot only one successor reads can be
the only thing that made a frame necessary on the other path.

`latched` is the other way round: a block of copies alone on a back edge,
whose one predecessor branches to it and elsewhere, has them run before that
branch when the other way out does not read what they write, and the edge
block keeps only its jump (a back edge's `mov; jmp` becomes a `jl`).

## Example
A function copies its argument `ax` into `bx`, then either returns at once
or loops reading `bx`. Before (stage `ParallelCopy`):

```lir
═══ _f@3 · ParallelCopy · m16 ══════════════════════════════════════════════════
fn _f@3(t15@ax, y5.2.lcssa@dx, y5.1.lcssa@cx) -> @eax

B0  entry  → B2 B1                                                ×1.0
    #41  bx         = mov.i16     ax
     ; ...
     #2               cmp.i16     dx, 0
     #3               je          B2 else B1                            ; 38% taken
     ; ...
B6  ← B1  → B5                                                    ×0.6
    #21  dx         = shl.i16     dx, 1
     ; ...
    #23  bx         = add.i16     bx, dx
```

After (stage `Peephole`; the `test`, `xor` and `add` are other peephole
rewrites):

```lir
═══ _f@3 · Peephole · m16 ══════════════════════════════════════════════════════
fn _f@3(t15@ax, y5.2.lcssa@dx, y5.1.lcssa@cx) -> @eax

B0  entry  → B2 B1                                                ×1.0
     ; ...
     #2               test.i16    dx, dx
     #3               je          B2 else B1                            ; 38% taken
     ; ...
B6  ← B1  → B5                                                    ×0.6
    #41  bx         = mov.i16     ax
    #21  dx         = add.i16     dx, dx
     ; ...
    #23  bx         = add.i16     bx, dx
```

Only the loop path reads `bx`, and `B1` and `B6` each have one predecessor,
so the copy moves down to `B6` and the path through `B2` runs without it.

From `ISEL_DUMP=1` of `int f(int *p, int n, int k)`, which returns 0 when
`n` is 0 and else sums `p[i] * k`, with `-m16 -O2`. Unit test
`test_a_copy_only_one_successor_reads_moves_into_it` asserts the copy count
per block.

## Runs
A sub-pass of [`crate::backend::peephole`](peephole.md), after `spillforward`: a reload
on the other path is gone by then, so the store need not run there. Once per
peephole run. `latched` follows it.

## Keeps
The body is returned unchanged where no block with several successors holds
a copy or spill store. Block odds follow a bridged edge.

## See also
- LLVM `PostRAMachineSinking`:
  <https://github.com/llvm/llvm-project/blob/llvmorg-18.1.8/llvm/lib/CodeGen/MachineSink.cpp>
