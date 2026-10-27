> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/machinecp.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/machinecp.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/machinecp.rs sha256:04ca98ccd9bb2e0267d4fffa3142fb029c79c4dc98f75db141bcf45f60b56eda -->

# machinecp

Forward and backward copy propagation inside a block, and dead-copy
removal: LLVM's `MachineCopyPropagation`.

## How it works
Registers are chosen. Each block is walked twice and alone, backward then
forward, with a tracker keyed by the byte lanes of the general registers
(28 of them, LLVM's register units). The tracker knows, per lane, the copy
that last defined it, whether that copy is still available (neither side
written since), the lanes it defines, and the instructions that read a
copy's source (`src_users`).

Forward (`forwardCopyPropagateBlock`): a copy that repeats an available one
(`ax = bx ... bx = ax` too) is dropped; a read of a copy's destination
reads its source instead; a copy whose destination is read by nothing
before the block ends and is not live out is dead and removed.
Backward (`backwardCopyPropagateBlock`): a copy whose source dies there
(`Y = op ...; Z = Y`) moves into the producer: the producer writes Z, and
the reads of Y between them read Z.

```text
  mov bx, ax                 mov bx, ax      <- dead when bx is not read
  add cx, bx          =>     add cx, ax         again and is not live out
  mov bx, ax                 (dropped)       <- bx == ax already
```

A removed copy becomes an anchor (`lir::anchor`), as in `machinedce`. A
rewrite is kept only when the new spelling still selects and the decoded
effects are the old ones with the renamed lanes replaced.

LLVM's `CopyTracker` is one `DenseMap` of units; here the units are 28
slots of an array and a set of units is a `u64` mask.

DEVIATE 1: LLVM asks a machine operand whether it is `renamable` and
whether `getRegClassConstraint` admits the new register; llrm has neither
flag, so the check is `select::priced_in` plus the effects test above.
Exit: none needed; the flags exist only because LLVM's operands carry
constraints llrm's instructions state in their semantics.

DEVIATE 2: an instruction with unknown effects, or a contract of requires,
delivers, clobbers, a spread or a group, is never rewritten and ends every
tracked copy (LLVM sees a regmask and the operands themselves).

## Example
No honest example: it runs inside the peephole phase, whose dump mixes it
with every other rewrite. The tests build the bodies.

## Runs
In the peephole phase, where `copyprop` ran (it and `regthrash`'s rename
are gone from the pipeline). Skipped when the body holds no register copy
(`needs::COPY`); again after the zero-extension rewrite on targets that
have one.

## See also
LLVM `MachineCopyPropagation`
(https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineCopyPropagation.cpp);
`crate::backend`
