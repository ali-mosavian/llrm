> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/jumps.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/jumps.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/jumps.rs sha256:9af634a90ed0676dcb02461367a4ee3f992c6d761e25b00cbf92bfa2487c9d4f -->

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
irreducible is refused. A tail is not copied into a jump that follows a
branch whose other way is likelier than the tail (and is not the block
itself): the copy would leave code between that branch and what follows
it, and the likelier way would be taken whatever is laid next (a latch with
a short exit tail would pay a taken back edge every trip to save the exit's
one jump). `duplicated` then does the same for tails that return,
`rotated` rotates a loop so its latch falls into the header (gcc's
bb-reorder `rotate_loop`) when that takes fewer jumps by the target's jump
prices, and `inverted` flips each `jcc L; jmp M` the copying left where `L`
is next; where neither way is next, it makes the likelier way the
conditional jump, so the trip that goes on pays one taken branch, not a
branch not taken and a jump (the loop that went round by `je exit; jmp
top`).

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
The `Scheduler` stage ends with the loop entered through a block that ends
in an explicit jump (`#15`). `ControlFlow` places the blocks (renumbering
them), inverts the entry branch so the loop follows it, and drops the jump
to the block that follows:

```lir
═══ _sum@3 · Scheduler · m16 ═══════════════════════════════════════════════════
B0  entry  → B3 B5                                                ×1.0
    #26  t1         = elided      y4.11
     #3               test.i16    ax, ax
     #4               jg          B3 else B5                            ; 97% taken
; ...
B3  ← B0  → B2                                                    ×1.0
    #13  bx         = xor.i16     bx, bx
    #14  cx         = xor.i16     cx, cx
    #15               jmp         B2
; ...

═══ _sum@3 · ControlFlow · m16 ═════════════════════════════════════════════════
B0  entry  → B1 B4
    #26  t1         = elided      y4.11
     #3               test.i16    ax, ax
     #4               jle         B4 else B1                            ; 3% taken

B1  ← B0  → B2
    #13  bx         = xor.i16     bx, bx
    #14  cx         = xor.i16     cx, cx
```

From `ISEL_DUMP=1 llrm-c -m16 -O2` of a C function `sum` with a `while`
loop, stages `Scheduler` and `ControlFlow`.

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
