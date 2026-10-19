> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/selects.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/selects.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/selects.rs sha256:dcab584eb3a7b03eb151810c9da266c3ef29f02fd869530f0c365017cb5c472e -->

# selects

Turns each MIR `select` into a branch around an empty block and a phi, where
the target has no conditional move.

## How it works
`select c, a, b` splits its block at the select. The instructions after it
move to a new join block, which takes over the block's way out (the phis of
its successors are renamed to name the join). A new block `taken` holds only
a jump to the join. The original block ends in a branch on `c` to `taken` or
the join, and the join starts with `phi [a, taken], [b, block]`, which
replaces the select's uses:

```text
 block:  ...                   block:  ...
         %r = select c, a, b           br c, taken, join
         rest                  taken:  br join
                               join:   %r = phi [a, taken], [b, block]
                                       rest
```

An operand of the select that only the select uses, is safe to execute
speculatively and costs `TCC_Expensive` or more (a division) moves into the
block of its own side, so the other path does not run it: CodeGenPrepare's
`sinkSelectOperand`. A cheap one stays, as in LLVM. Where the target states
no cost model every instruction costs one, and nothing moves.

The loop is repeated until no select is left in any function, so two selects
in one block each get their own split. The result is checked by an
interpreter test that runs a function before and after.

## Example
Before:

```mir
define i16 @f(i16 %a, i16 %b) {
entry:
  %less = icmp ult i16 %a, %b
  %least = select i1 %less, i16 %a, i16 %b
  %twice = add i16 %least, %least
  br label %done
done:
  %r = phi i16 [ %twice, %entry ]
  ret i16 %r
}
```

After:

```mir
define i16 @f(i16 %a, i16 %b) {
entry:
  %less = icmp ult i16 %a, %b
  br i1 %less, label %0, label %1
0:
  br label %1
1:
  %2 = phi i16 [ %a, %0 ], [ %b, %entry ]
  %twice = add i16 %2, %2
  br label %done
done:
  %r = phi i16 [ %twice, %1 ]
  ret i16 %r
}
```

The select became a branch to the empty block `0` and a phi in `1`; the phi
of `done` now names `1` as its predecessor.

From unit test `test_a_select_becomes_a_branch_and_a_phi` in `selects.rs`.

## Runs
Once per program in `driver::optimized`, after `fpconvert` and before the
spill-model dump, only where the target's `datalayout.toml` lists `select`
under `expand` (m16 and m32: no conditional move before the Pentium Pro).
There is no flag; it runs at every -O level.

## See also
LLVM's CodeGenPrepare (`optimizeSelectInst`) makes the same change where the
target prices a branch below a conditional move.
