> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/jumps.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/jumps.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/jumps.rs sha256:bad7966f81d555b13211ccda2c6d027000686abff0387b833a41cefd2383600c -->

# jumps

Orders the blocks of an allocated body and removes the jumps and blocks the
order makes redundant; merges and copies tails.

## How it works
Blocks are first written with every fall-through as an explicit jump
(`_explicit`), which makes any order correct. `optimized` then runs these
steps on the body.

1. `_hoisted` moves an edge block's no-byte anchors up before the branch
   into it, so the edge block becomes a bare jump that threading can skip.
2. `_placed` orders the blocks as LLVM's `MachineBlockPlacement` does
   (`placement`): chains joined along the likelier edges, loops innermost
   first with their top and exit chosen and rotated, trellises, and a short
   block copied into the blocks that jump to it where that saves taken
   branches. See `docs/architecture/jumps-placement-check.md`.
3. `threaded` is `branchfold` (LLVM's `OptimizeBranches`): a jump to a block
   that only jumps goes to its target (a loop header with a measured trip
   count is kept), a jump to the next block is dropped, `jcc L; jmp M` where
   `L` is next becomes `j!cc M`. Unreachable blocks go.
4. `tailmerge::merged` (LLVM's `BranchFolder::TailMergeBlocks`) gives the
   blocks that share the last instructions one copy of them, splitting a
   block where no one holds only the tail, and the body is threaded again; a
   compare the merge left nothing to select on stays, as in LLVM. Both
   repeat until nothing merges.
5. `inverted` flips each `jcc L; jmp M` the copying left where `L` is next;
   where neither way is next, it makes the likelier way the conditional
   jump, so the trip that goes on pays one taken branch, not a branch not
   taken and a jump (the loop that went round by `je exit; jmp top`).

```text
  before                       after (placed, a guard copy of the test)
  test: cmp; jge exit          cmp; jge exit
  body: ...; jmp test          body: ...; cmp; jl body
  exit: ...                    exit: ...
```

`duplicated_returns` is separate and runs later, from `assemble`, because
the frame teardown is emitted around `ret` and its size is known only to
`masm`. A block that only returns (no calls, nothing source-owned, within
`COPY_BB_INSNS` unconditional jumps of bytes, gcc's
`max-grow-copy-bb-insns`) is copied into each parent that is its only
predecessor edge, where the bytes saved are more than the extra copies
cost.

## Example
The `Scheduler` stage ends with the loop entered by an explicit jump.
`ControlFlow` places the blocks and drops the jump to the block that
follows:

```lir
=== Scheduler
    block 0x0000
      ; ...
      0x0003  AX := mov -7  ; inserted
      0x0003  jmp   ; inserted
    block 0x0004
      ; ...
      0x0008  AX := inc AX  ; inserted
      0x000a  jne   ; inserted
=== ControlFlow
    block 0x0000
      ; ...
      0x0003  AX := mov -7  ; inserted
    block 0x0004
      ; ...
      0x0008  AX := inc AX  ; inserted
      0x000a  jne   ; inserted
```

From `ISEL_DUMP=1` of a nib program (a struct method with a `while` loop,
called from `main`) at `-m16 -O2`, stages `Scheduler` and `ControlFlow`.

## Runs
`ControlFlow` is the last machine phase, because this physical order
decides which explicit edge is the fall-through. `jumps` shows as the
phase name. A body in source order (`source_order`) is only made explicit;
at -O0 (`Profile::optimized` off) every body is, and its redundant jumps are
dropped, as gcc's and clang's -O0 place no blocks.
`LLRM_DEBUG=blocks` prints each block's frequency and size afterwards.
`duplicated_returns` runs once per procedure in `assemble::assembled_by`
(`lir duplicated returns`).

## See also
LLVM `BranchFolding` and `MachineBlockPlacement`; gcc `cleanup_cfg`,
`bb-reorder` and `copy_bb_p`.
