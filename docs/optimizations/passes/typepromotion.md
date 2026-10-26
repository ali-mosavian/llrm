> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/typepromotion.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/typepromotion.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/typepromotion.rs sha256:0d3cef400d2f278bbb9bc4ea075815c4e1c1825060ef4ca1675f49766e1b9a08 -->

# typepromotion

CodeGenPrepare's `TypePromotionTransaction`: a run of edits to a function
that can be taken back to any point it passed, or kept.
([CodeGenPrepare.cpp#L3381](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L3381)).

## How it works
Every edit is made on the function at once and recorded with what undoes
it: an operand set (`OperandSetter`), the operands of an instruction hidden
(`OperandsHider`), a `trunc`, `sext` or `zext` made (`TruncBuilder`,
`SExtBuilder`, `ZExtBuilder`), a type mutated (`TypeMutator`), the uses of a
value moved to another (`UsesReplacer`), an instruction moved
(`InstructionMoveBefore`) and an instruction removed (`InstructionRemover`:
its operands hidden, its uses given to the value that replaces it, and its
erasure left to `commit`). `Transaction::restoration_point` is a position
in the record; `Transaction::rollback` undoes the edits after it, last
first; `Transaction::commit` erases what was removed and forgets the
record.

```text
  let point = tpt.restoration_point();
  let wide  = tpt.create_sext(function, context, narrow, ty, before);
  tpt.rollback(function, point);        // the sext is gone again
```

MIRROR: the nine actions and the order they undo in; a removed instruction
stays in its block, unread, until `commit` (the pin keeps it unlinked: MIR
cannot unlink without erasing).

## Example
The tests in `typepromotion_tests.rs`:
`test_a_created_extension_is_erased_by_a_rollback` is the sketch above.

## Runs
Under `cgpaddr`'s `optimizeExt`, which makes a transaction for each
extension it considers.
