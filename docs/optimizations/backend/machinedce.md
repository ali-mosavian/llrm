> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/machinedce.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/machinedce.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/machinedce.rs sha256:fcaab19ceba966353eba41a3d47ec8c13b16afc39db1ee979e0915258e2f3b0b -->

# machinedce

Removes a pure allocated instruction whose whole result is dead.

## How it works
MIR dead-code elimination reasons about values before allocation. Lowering,
splitting and physical rewrites can leave a machine computation whose
virtual definition still exists but whose register and flag results nothing
reads. This pass answers only that machine question, from the lane
liveness of [`crate::backend::liveness`](liveness.md).

Each block is walked backward from the lanes dead at its exit
(`dead_at_exit`). An instruction is removed when every lane it writes is
dead and it is pure: an operation from `_PURE` (move, exchange, address,
binary, multiply, compare, unary, funnel, extend), with register,
immediate or address operands (and a named frame slot as a source), no
relocation, no stack or segment destination, no positional register (their
effects are not lane-visible), no call or spill role. Otherwise the walk
applies the instruction's effect and goes on. Memory reads, control flow,
trapping arithmetic and x87 work are never removed.

```text
  mov ax,bx        both write only ax and flags, and the
  add ax,1         mov below overwrites ax: both become anchors
  mov ax,si        (ax live after, flags dead)
```

A removed instruction becomes an anchor (`lir::anchor`) that keeps source
ownership and emits no bytes. A sweep can expose more dead work, so sweeps
repeat, at most once per instruction, until one changes nothing.

## Example
No honest example: it is one sub-pass of `peephole`, whose `ISEL_DUMP=1`
stage shows all the sub-passes' work together, and its tests build bodies by
hand and assert on the operations. A removed instruction prints in the dump
as an empty line (`0x0003     ; inserted`).

## Runs
The last body rewrite of `peephole` before `popped_arguments`, and
`jumps::optimized` after each tail merge.
