> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/instsimplify.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/instsimplify.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/instsimplify.rs sha256:ff1efa776c9b5496a16b9508c686899f7009497533584540b5961bd6d31dfbe0 -->

# instsimplify

InstSimplify as a pass: every instruction that equals one of its operands,
or a constant, is replaced by it, and what that leaves dead goes.

## How it works
A translation of LLVM's
[`InstSimplifyPass.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/InstSimplifyPass.cpp)
`runImpl` (:27) at d1106deb71cc. MIRROR: the first sweep visits every
instruction of every reachable block in layout order, later sweeps only
the users of what was replaced; an instruction nothing reads and running
does nothing else is deleted, the instruction found by
`simplify_instruction_in` replaces the rest and its users are visited
again; the dead of a block go when the block is done, and with them what
only they read (`RecursivelyDeleteTriviallyDeadInstructions`).

The query has the function's dominator tree and assumptions
(`FunctionAnalyses`), built once: replacing a value and deleting dead
arithmetic changes neither.

## Parity slot
Slot 38 of the slot table (docs/architecture plan 2.5): `instsimplify`,
once per function pipeline after the loop group at -O1/-O2/-O3. Joined by
naming it in `llvm_order`.

## Keeps
Blocks and edges are unchanged: dominators and loops are preserved.

## Example
An instruction that equals an operand is replaced by it and then deleted:

```text
%a = add i32 %x, 0
%b = mul i32 %a, 2   ; reads %x after the pass
```

## Used by
Named by `pipeline`.
