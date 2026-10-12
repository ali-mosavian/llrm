> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/postrasink.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/postrasink.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/postrasink.rs sha256:14fe44369eefbb49c528c2e4fc83e1d4b475584d88288654767a17a7ff64aca1 -->

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

## Runs
A sub-pass of [`crate::backend::peephole`](peephole.md), after `spillforward`: a reload
on the other path is gone by then, so the store need not run there. Once per
peephole run.

## Keeps
The body is returned unchanged where no block with several successors holds
a copy or spill store. Block odds follow a bridged edge.

## See also
- LLVM `PostRAMachineSinking`:
  <https://github.com/llvm/llvm-project/blob/llvmorg-18.1.8/llvm/lib/CodeGen/MachineSink.cpp>
