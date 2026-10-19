> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine.rs sha256:fc851a0ad18c0983297d49a1c62b0a69e89acc77502efd30fd85bf664eb4ea75 -->

# combine

One worklist over a function's instructions: each is visited, simplified to
a value it equals when its operands say so, and what read it is visited
again.

A translation of LLVM's InstCombine driver,
[`InstructionCombining.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp)
at d1106deb71cc: `prepareWorklist` (:6031) seeds it, `run` (:5780) drains
it, `eraseInstFromFunction` (InstCombineInternal.h:504) re-queues what lost
a user, over
[`InstructionWorklist`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/include/llvm/Transforms/Utils/InstructionWorklist.h).
`visit` is [`llrm_mir::simplify::simplify_instruction_in`](https://github.com/ali-mosavian/llrm/blob/main/crates/ir/llrm-mir/src/simplify.rs).

MIRROR: the worklist (a stack, a map from instruction to slot, a deferred
set); the seed in reverse reverse-post-order after folding and deleting the
dead as it walks; the live-successor handling of a constant branch (the phi
inputs of a dead edge become poison); the pop, delete-if-dead, visit,
replace-all-uses, `pushUsersToWorkList`, erase sequence.

DEVIATE, with the reason: code sinking (`tryToSinkInstruction`) is not made
(no counterpart in llrm yet: its cost is a number the combine page owes);
blocks no live edge reaches keep their instructions (`decide` removes
them; LLVM's `removeAllNonTerminatorAndEHPadInstructions` is a CFG edit
this pass does not make); a call is never trivially dead here (`dead` knows
what a call does not write); `visit` is `simplify_instruction_in` alone: the
rewrites of InstCombine's `visit*` functions are `algebraic`'s, to move in
as they are translated (the visit map in the combine page).

## Used by
`fold` (its number step).

## Example
An instruction that simplifies is replaced, and what read it is visited
again:

```text
%a = add i32 %x, 0
%b = mul i32 %a, 1   ; %a becomes %x, then %b becomes %x
```
