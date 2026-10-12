> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/jumps.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/jumps.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/jumps.rs sha256:da24966e90e8fc4e505c0d34d8bc3e7bc3e61f0b7f9f120f5d4d01c30e2745b9 -->

# jumps

Orders the blocks of an allocated body and removes the jumps and blocks the
order makes redundant; merges and copies tails.

## How it works
Blocks are first written with every fall-through as an explicit jump
(`_explicit`), which makes any order correct. `optimized` then runs these
steps on the body.

1. `_hoisted` moves an edge block's no-byte anchors up before the branch
   into it, so the edge block becomes a bare jump that threading can skip.
2. `_placed` orders the blocks. By estimated frequency (every block counts
   once at -Os): the likelier edge falls through, a diamond's likelier arm
   falls into its join, and a new trace starts at the busiest block left. A
   loop header that only decides whether to go round (`_tests`) is placed
   after its latch, so a pass round the loop takes one branch. Cold blocks
   go last.
3. `threaded` applies local changes until none applies (`_step`): a jump to
   a block that only jumps goes to its target (`_through`; a loop header
   with a measured trip count is kept), a jump to the next block is dropped,
   `jcc L; jmp M` where `L` is next becomes `j!cc M`, and a `jcc` over a
   block that only jumps is inverted. Unreachable blocks go.
4. `merged` joins blocks with the same physical tail (`_tail_key`: the same
   instructions, registers and successors) and renames the values of the
   copy to the kept block's; `machinedce::eliminated` then removes what the
   merge made dead (a compare nothing selects on). Both repeat until nothing
   changes, and the merged body is threaded again. `preferred` takes it only
   if static work falls and the executed work (`_work`, blocks by frequency)
   does not rise.

Then, unless sized for code size, `duplicated_tails` replaces a `jmp` to a
short tail (at most `TAIL_DUPLICATION` plain instructions besides its
jumps) by a copy of the tail, where the copy's own fall-through jump runs
less often than the jump removed, and a copy that would make a loop
irreducible is refused. `duplicated` then does the same for tails that
return, `rotated` rotates a loop so its latch falls into the header
(gcc's bb-reorder `rotate_loop`) when that takes fewer jumps by the
target's jump prices, and `inverted` flips each `jcc L; jmp M` the copying
left where `L` is next.

```text
  before                       after (placed, threaded)
  test: cmp; jge exit          jmp test
  body: ...; jmp test          body: ...
  exit: ...                    test: cmp; jl body
                               exit: ...
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
phase name. A body in source order (`source_order`) is only made explicit.
`LLRM_DEBUG=blocks` prints each block's frequency and size afterwards.
`duplicated_returns` runs once per procedure in `assemble::assembled_by`
(`lir duplicated returns`).

## See also
LLVM `BranchFolding` and `MachineBlockPlacement`; gcc `cleanup_cfg`,
`bb-reorder` and `copy_bb_p`.
