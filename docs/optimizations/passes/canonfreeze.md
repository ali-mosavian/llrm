> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/canonfreeze.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/canonfreeze.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/canonfreeze.rs sha256:4f9fd4dbfdc6135556526a6199af2cebad0c965d4705c0e8fe2c181af8c673c5 -->

# canonfreeze

LLVM's `CanonicalizeFreezeInLoops` (`canon-freeze`;
CanonicalizeFreezeInLoops.cpp at `d1106deb71cc`): a freeze of an induction
variable or of its step is pushed out of the loop, to the preheader.

## How it works
Each loop of simplify form, innermost first, is searched for the phis of its
header that are induction variables stepped by an `add`, `sub` or `mul`
whose step value is not defined in the loop. A phi or step with a `freeze`
user is a candidate. For each candidate phi once: the step's poison
generating flags are dropped unless it is already not poison, the step value
and the phi's start value are frozen in the preheader unless they are
already not undef or poison, and the freezes in the loop are replaced by
their operands. ScalarEvolution forgets what it changed.

## Example
`i = phi [init, pre], [i.next, loop]; i.next = add nsw i, 1;
f = freeze i.next` becomes `init.frozen = freeze init` in the preheader,
`i = phi [init.frozen, pre], [i.next, loop]` and `i.next = add i, 1`: the
loop is an add recurrence again and the interpreter agrees.

## Runs
In the strength slot, before `lsr` (the pin runs it with loop-reduce).

## Limits
No frontend emits `freeze`, so on the corpus it changes nothing (0 of 77
IRs differ). No DEVIATE: the pin's `DenseMapInfo` set of candidates is a
list keyed by the freeze.
