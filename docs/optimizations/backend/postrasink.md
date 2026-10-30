> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/postrasink.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/postrasink.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/postrasink.rs sha256:22f13d62d1b79ebf1a0b8376a09e8743674881be731e29ee32eff53abe752c27 -->

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
too, such as a loop header, takes no store.

The point is shrink wrapping. A path that returns at once then has no copy
and no store of a saved register, and the prologue can be placed after the
test, as gcc and LLVM do. A store of a slot only one successor reads can be
the only thing that made a frame necessary on the other path.

`latched` is the other way round: a block of copies alone on a back edge,
whose one predecessor branches to it and elsewhere, has them run before that
branch when the other way out does not read what they write, and the edge
block keeps only its jump (a back edge's `mov; jmp` becomes a `jl`).

## Example
A function copies its argument into `DI`, then either returns at once or
loops reading `DI`.

Schematic, from the fixture of test
`test_a_copy_only_one_successor_reads_moves_into_it` (not printer output):
Before:

```lir
block 0x0001
  0x0001  DI := mov DX
  0x0002  cmp AX, BX
  0x0003  jl
block 0x0005
  0x0005  ret
block 0x0009
  0x0009  AX := add AX, DI
  0x000a  jmp
```

After:

```lir
block 0x0001
  0x0002  cmp AX, BX
  0x0003  jl
block 0x0005
  0x0005  ret
block 0x0009
  0x0001  DI := mov DX
  0x0009  AX := add AX, DI
  0x000a  jmp
```

Only block 0x0009 reads `DI` and has no other predecessor, so the copy moves
there and the path through 0x0005 runs without it.

From unit test `test_a_copy_only_one_successor_reads_moves_into_it` in
`postrasink.rs`, which asserts the copy count per block (0 in 0x0001, 1 in
0x0009).

## Runs
A sub-pass of [`crate::backend::peephole`](peephole.md), after `copysink`. Once per
peephole run. `latched` follows it.

## Keeps
The body is returned unchanged where no block with several successors holds
a copy or spill store.

## Deviations from the pin
The copy half is `PostRAMachineSinkingImpl::tryToSinkCopy`; a copy moves
when no later instruction of the block touches what it reads or writes and
one successor, entered from this block alone, has its result live in.
- DEVIATE P1: the spill-store half (a store to a frame cell sunk by cell
  liveness, for shrink wrapping) has no counterpart in the pin, where
  `ShrinkWrap` places the prologue. Exit: ShrinkWrap translated.
- DEVIATE P2: `latched` has no counterpart in the pin (x_cordic `-Os`).
  Exit: the coalescer or block placement keeps the copy off the back edge.
- Blocks are walked in reverse post-order and past a call, not the pin's
  layout order with a call stopping a block: the pin's way gives 7 different
  objects of 556 (net +4 B, 2 larger), so ours stays.

## See also
- LLVM `PostRAMachineSinking`:
  <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineSink.cpp#L2243>
