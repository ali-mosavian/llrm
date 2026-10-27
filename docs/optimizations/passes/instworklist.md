> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/instworklist.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/instworklist.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/instworklist.rs sha256:b001ad69803603155277a4f1390ea5995e05b217604858b2ef29ea53ea296288 -->

# instworklist

The instruction worklist of InstCombine and the passes that drive a
simplifier to a fixed point. Translated from LLVM's `InstructionWorklist`
(`llvm/include/llvm/Transforms/Utils/InstructionWorklist.h`).

## How it works
Two lists of instructions, neither holding one twice.

`push` puts an instruction on the stack; `remove_one` takes the last one
pushed. `add` puts it on the deferred list instead; `pop_deferred` takes the
last one added, so a driver that moves each deferred instruction to the
stack after a combine finishes visits them in the order they were added.
`remove` takes an instruction off both lists.

Every operation is O(1), amortised: a removed entry leaves an empty slot
that the next pop from that list steps over, and a map from instruction to
slot finds it. LLVM's deferred list is a `SmallSetVector`, whose removal is
O(n); the stack and its map are as LLVM's.

```text
add a, add b, add c      pop_deferred -> c, b, a
push c, push b, push a   remove_one   -> a, b, c   (the order added)
```

This type is the data structure only. The visit loop and the rule for which
users to add after a fold belong to the pass that uses it.

## Example
Three instructions added, then moved to the stack as a driver does:
```text
add %a, add %b, add %c
pop_deferred -> %c  push %c
pop_deferred -> %b  push %b
pop_deferred -> %a  push %a
remove_one   -> %a, %b, %c      (the order added)
```

From unit test `deferred_moved_to_the_stack_come_out_in_the_order_added` in
`instworklist_tests.rs`.

## Runs
Not a pass: a data structure. Nothing uses it yet; `instsimplify`'s driver
will.

## See also
LLVM:
[`InstructionWorklist.h`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Transforms/Utils/InstructionWorklist.h).
