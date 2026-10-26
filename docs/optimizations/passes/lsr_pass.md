> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/lsr_pass.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/lsr_pass.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/lsr_pass.rs sha256:4d820b14dbf7db97f5832d220d04b059f7135554edce2d33374b682b6d328a15 -->

# lsr_pass

LLVM's loop strength reduction as a pass (`LoopStrengthReduce`,
`ReduceLoopStrength` and `LSRInstance::LSRInstance`;
LoopStrengthReduce.cpp at `d1106deb71cc`, l.6307-6430 and 7187-7265).

## How it works
Each innermost loop of simplify form, innermost first, is taken once: its IV
users are found ([`crate::ivusers`](ivusers.md)), they become uses and fixups with a
first formula ([`crate::lsr_collect`](lsr_collect.md)), the formulae are generated and
narrowed ([`crate::lsr_generate`](lsr_generate.md), [`crate::lsr_narrow`](lsr_narrow.md)), and the solver
picks one per use ([`crate::lsr_solve`](lsr_solve.md)). If it picks, the code is expanded
([`crate::lsr_implement`](lsr_implement.md)) and what the replaced operands leave unread goes.
The costs are the target's ([`llrm_mir::tti::Tti`](https://github.com/ali-mosavian/llrm/blob/main/crates/ir/llrm-mir/src/tti.rs)).

## Example
A local array filled and summed in a loop, exit `i + 1 == n`, becomes a
pointer phi stepping by 4 and a counter from `%n - 1` to 0; the interpreter
agrees on every input.

## Runs
In the pipeline's strength slot, after `fixednarrow`.

## Limits
DEVIATE: not yet ported: the debug value salvage. `DeleteDeadPHIs` reads
the header's phis only. `replaceCongruentIVs` runs on every loop in
simplify form, as `ReduceLoopStrength` does. A loop of more than 200 IV
users is left (`MaxIVUsers`).
