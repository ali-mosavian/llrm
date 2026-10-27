> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/looploadelim.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/looploadelim.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/looploadelim.rs sha256:4142ea03051133d3d8f4efe8c816e3eedc06f7b4b0ad92fba34c35563bd48053 -->

# looploadelim

`LoopLoadElimination`: a load of what the previous iteration stored is
replaced by the stored value, carried round the loop in a phi.

## How it works
A translation of LLVM `d1106deb71cc` `LoopLoadElimination.cpp`. For each
innermost loop in rotated form with one exiting block, `LoopAccessInfo`
lists the dependences between its memory accesses. A store whose value a
load reads one iteration later (`a[i + 1] = x; ... = a[i]`, a distance of
one element, the load unconditional, the store on every path to the latch)
is a candidate, unless the load has another unsafe dependence or several
stores forward to it. Stores that may overwrite the location in between
(the pointers written on the path from the first forwarding store to the
last forwarded load) need run-time checks; more than one check for each
candidate on average is too many, and none is made when optimizing for
size. The loop is then versioned ([`crate::loopversioning`](loopversioning.md)) and, in the
loop that is kept, each candidate load is replaced by a phi of the value
loaded in the preheader (`load_initial`) and the stored value.

## Example
```text
loop:  %x = load a[i];  ...  store %y, a[i + 1]
```
becomes
```text
ph:    %load_initial = load a[0]
loop:  %store_forwarded = phi [ %load_initial, %ph ], [ %y, %latch ]; ...
```

## DEVIATE 24 (no predicated SCEV)
`LAI.getPSE().getPredicate()` is always true, so its complexity check and
the versioning for SCEV predicates are absent.

## DEVIATE 33 (no profile)
`shouldOptimizeForSize` is the function's `optsize`/`minsize`: there is no
`ProfileSummaryInfo` or `BlockFrequencyInfo`.

## Runs
In the parity pipeline at `loop-load-elim` (l.1400 of
`PassBuilderPipelines.cpp`, after `infer-alignment`).
