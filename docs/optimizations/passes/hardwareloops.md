> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/hardwareloops.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/hardwareloops.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/hardwareloops.rs sha256:b6bda01b3a3a81951fbbbb504b80b60b671b86de8de5a74cfea0e924516d277a -->

# hardwareloops

A counted loop whose target counts in hardware: LLVM's `HardwareLoops`
(`hardware-loops`; HardwareLoops.cpp and `HardwareLoopInfo` at
`d1106deb71cc`).

## How it works
The loops are visited innermost first. For a loop of the target's choice
(`Tti::is_hardware_loop_profitable`, which also sets the counter's width
and the decrement) that has a preheader and no irreducible control flow,
an exiting block is looked for that every trip passes through, ends in a
conditional branch, and has a loop-invariant, non-zero exit count that fits
the counter (`isHardwareLoopCandidate`). The trip count `exit count + 1` is
written in the counter's type before the preheader's branch and handed to
`llvm.set.loop.iterations`; the exit branch's condition becomes
`llvm.loop.decrement(decrement)`, true where the loop goes on. The old
condition, and the induction variable only it read, are deleted.

A loop inside another stops the search at the parent when a conversion was
made and the target does not allow nesting.

## Example
`for (i = 0; i != 10; i++) body` has exit count 9: `set.loop.iterations(10)`
in the preheader, and `br i1 (loop.decrement(1)), loop, exit` where
`br i1 (icmp ne i.next, 10), loop, exit` was.

## Runs
In the strength slot, after `lsr`, only for a target whose
`Tti::is_hardware_loop_profitable` says yes: none does yet, so the pass
changes nothing anywhere until a target lowers the intrinsics.

## Limits
No entry test is made (`llvm.test.set.loop.iterations`). A target whose
counter is a register value (`counter_in_reg`) gets the phi form:
`llvm.start.loop.iterations`, a phi, `llvm.loop.decrement.reg` and `icmp ne
0`, which the interpreter runs.

DEVIATE 1: `isHardwareLoopCandidate` rejects an exit count whose type is
wider than the counter. Here the count is taken when its unsigned range
fits the counter and is cut to it: an `int` loop of 54 trips has an `i16`
exit count, and a one-byte counter holds it. The trip count wraps to zero
at the counter's largest value plus one, which is the full count of a
counter that runs from zero. Exit: none; a target that wants the pin's
test sets a counter as wide as its types.

DEVIATE 2: swapping the exit branch's successors drops its `prof` branch
weights; the pin swaps them. Exit: swap the weights.

DEVIATE 3: the target's hook sees no loop (the pin passes the loop, SCEV,
the assumption cache and library info), and the pass asks it once, not per
loop. Exit: the first target whose answer depends on the loop.

DEVIATE 4: `noduplicate`, which `llvm.set.loop.iterations`,
`llvm.test.set.loop.iterations` and `llvm.loop.decrement` carry, is not an
attribute MIR holds. Nothing after this pass copies a loop or a call
(the strength slot is followed by the code generator). Exit: MIR's
`noduplicate`.

DEVIATE 5: a loop is eligible only in LoopSimplify form (the pin inserts a
preheader where there is none). Exit: `InsertPreheaderForLoop`.

DEVIATE 6: when the exit test was `icmp` of a counter that only the test and
its own update read (the phi and the update read each other), that counter
is deleted. The pin leaves the cycle, which `RecursivelyDelete...` does not
see as dead. Exit: none.

## See also
`HardwareLoopsImpl::TryConvertLoop`, `HardwareLoop::Create`,
`HardwareLoopInfo::isHardwareLoopCandidate`.
