> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/tailcallelim.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/tailcallelim.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/tailcallelim.rs sha256:0f253a8db4cf50c691240055aab452ec4407a76fc547bd04a24179df5fd70ff5 -->

# tailcallelim

Tail calls are marked, and a function's calls to itself in tail position
become a loop, as LLVM's `TailCallElim`.

## How it works
A translation of LLVM `d1106deb71cc` `TailRecursionElimination.cpp`, each
function linking the lines it translates.

`mark_tails` sets `tail` on every call that cannot reach a frame object of
the function: one made before any alloca (or `byval` parameter) escapes, or
whose arguments are all constants and parameters of a function that reads
no memory. `TailRecursionEliminator` then looks, in each block that ends in
a `ret` (or in a `br` to a block that is one `ret`, which it duplicates
first), for a tail call to the function itself with only instructions that
can move above the call after it, and turns it into a branch to a new
header block that holds a phi for every parameter.

A single add, mul, and, or, xor or shift by a constant on the call's result
on the way out is an accumulator: a phi in the header starts at the
operation's identity (or the base-case constant, for a shift) and takes the
operation's other operand on each trip. A return value that is known on one
trip but not on the others is carried by `ret.tr` and `ret.known.tr`.

```text
  before                        after
  b0: ...                       b0:  br tailrecurse
      %v = call f(%n-1)         tailrecurse:
      %s = add %v, %x             %n.tr = phi [%n, b0], [%n-1, tailrecurse]
      ret %s                      %acc  = phi [0, b0], [%acc+%x, ...]
                                  ...
                                  br tailrecurse     ; was the call
                                other exits: ret (%acc + value)
```

DEVIATE (anything MIR or Rust lacks):
24. MIR has no operand bundles, token types, `stackrestore` or pseudo
    probes, so the checks that refuse those have nothing to refuse.
25. No `OptimizationRemarkEmitter`, `BlockFrequencyInfo` or
    `ProfileSummaryInfo`: `shouldDisableTailCallsForCold` is false with its
    default option, and the entry count is only updated under instrumented
    PGO (`UpdateFunctionEntryCount` is false in the default pipeline).
26. The dominator trees are not updated in place (`DomTreeUpdater` is not
    passed): the pass reports what it changed and the manager drops the
    analyses a new entry block and edges leave stale, as `gvn` does.
27. `isSafeToLoadUnconditionally` is `dereferenceable` and the load's
    alignment: no "a load of it dominates" proof.
28. `isAssociative` is the integer operations only (`combine_shared`'s
    DEVIATE 2), so a float accumulator is not recognized.

## Example
`sumto(n, x) = n == 0 ? x : sumto(n - 1, x + n)` becomes a loop that
carries `n` and `x`; see `tailcallelim_tests.rs`.

## Runs
As `tailrec`, the name its option (`sibcalls`) and slot (`tailrec@688`)
keep: in the function pipeline after `gvn::EqualCalls`, so two equal calls
are one first, and before `loopsimplify`, so the loop it makes goes to the
loop passes. Once per round of the fixed point; at -O2 and above, off at -O0
and -O1 and by `-fno-optimize-sibling-calls`.

## Needs
`BasicAA` for the mod/ref of a call on a load that moves above it.

## Keeps
The dominators, loops and shape where only tail flags changed; nothing
where a loop was made.
