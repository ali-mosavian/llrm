> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/copyprop.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/copyprop.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/copyprop.rs sha256:a0e05a2c6bf32bbd0a61a32418c03c4f5547750a32c55d25ad9478d88374637b -->

# copyprop

Forward and remove register-to-register copies after allocation.

## How it works
The pass tracks, per byte lane of the seven general registers (28 lanes),
what is known on every path to a point. LLVM's MachineCopyPropagation
does the same per register unit. Two facts are solved together by a
forward must-analysis over the CFG:

- `Equal`: a partition of the lanes into classes that hold the same value.
  Undirected. It decides whether a copy is redundant.
- A set of directed pairs `dest <- oldest source` from the copies that reach
  the point. It decides which register an operand may be renamed to, so that
  the older value is the one read and the copy can die.

A `mov reg,reg` of equal widths merges the destination lanes into their
source lanes' classes. Any other write, found from the instruction's
decoded effects (`peephole::_register_effects`), takes the written lanes
out of their classes and drops every pair that names them. An instruction
with no known effects resets the partition. At a join the facts are
intersected, from the must-analysis top; the entry block and any block
with no reachable predecessor start from nothing, so a back edge cannot
prove its own equality.

```text
  mov bx, ax                 mov bx, ax      <- now dead if bx is not
  add cx, bx          =>     add cx, ax         read later; machinedce
  mov [si+bx], dx            mov [si+ax], dx    removes it
  mov bx, ax                 (anchor)        <- bx == ax already
```

A second walk applies the facts. A copy whose every lane pair is already
equal becomes an anchor. For every other instruction, each explicit register
source and each register of an address (base or index, except the frame
base) is tried against the directed pair that reaches it. The rename
is kept only when all of these hold: the older register covers exactly
the same lanes at the same width, the lanes are proven equal, the
instruction does not write the older register, the new spelling still
selects as a real instruction, and the decoded effects of the new form
are the old ones with the renamed lanes replaced. Instructions with
register constraints (`requires`, `delivers`, clobbers, spreads, groups)
or a relocation are left alone.

A body with phis is returned unchanged.

## Example
No honest example: the pass runs inside the peephole phase, whose dump mixes
it with every other rewrite, and its unit test builds the body in Rust. Run
a program with `ISEL_DUMP=1` and compare the `ParallelCopy` and `Peephole`
stages for the renamed operands.

## Runs
Inside the peephole phase, after register thrashing and frame copies.
Skipped when the body holds no register copy (`needs::COPY`). Runs again
after the zero-extension rewrite on targets that have one, so the copies
that rewrite makes are forwarded. Once per peephole run, not a fixed
point. No flag of its own.

## Needs
[`crate::analysis::dataflow`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/scev/analysis.rs) and loop predecessors
([`crate::analysis::loops`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/scev/analysis.rs)).

## See also
LLVM `MachineCopyPropagation`; `crate::backend`
