> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/constraintelim_induction.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/constraintelim_induction.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/constraintelim_induction.rs sha256:3bc016d0da5c1ee6ee93d4bfebb95152faa8a6de073c5c8b3ee2a25679f92682 -->

# constraintelim_induction

`constraint-elimination`: the facts a loop's inductions give.

## How it works
A translation of LLVM `d1106deb71cc` `State::getMonotonicityInfo`,
`addBoundsForHeaderInductions` and `addInfoForInductions`, each function
linking the lines it translates. A header phi `{Start,+,Step}` that never
steps below its start (or above it, for a decrement) gives
`PN >= Start` in the loop; a loop exit compared against an invariant `B`
gives `PN < B` in the body, given a precondition on `Start` and `B` that
the walk checks before it adds the fact. The facts are queued as
condition facts of the loop's blocks, as a branch's are.

`Inductions` holds what LLVM's `State` holds by reference: `LI`, `SE`
and (for `SE`) the assumptions.

## Used by
`constraintelim::State::add_info_for`.

## Example
`for (j = 1919; j > -1; j--)`: the phi `{1919,+,-1}<nsw>` gives
`j <= 1919`, the header test `j > -1`, so `j <u 1920` is true.
