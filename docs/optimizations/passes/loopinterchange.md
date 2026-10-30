> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopinterchange.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopinterchange.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopinterchange.rs sha256:b8fb2083275a215fea01d232e8af935570696167c3bb5b64eb9f9c3dc6a15dff -->

# loopinterchange

Loop interchange: a loop of a perfect nest swaps places with the loop
around it when the dependences allow and the access order says the inner
loop should walk the other index (`LoopInterchange.cpp` at `d1106deb71cc`).

## How it works
Each perfect nest (a chain of loops, each the only child of its parent, two
deep or more) is a `LoopList`. The dependence matrix of the whole nest comes
from `DependenceInfo`: one row
for each distinct direction vector of a pair of memory accesses with a
write, one column for each loop and a last one saying whether the
dependence is known to point forward. Then a bubble sort moves the
innermost loop outwards as far as it pays, then the next, and so on
(`processLoopList`).

A pair of adjacent loops is interchanged when
- every row stays lexicographically non-negative with the two columns
  swapped (`isLegalToInterChangeLoops`);
- the loops hold no instruction that touches memory but a simple load or
  store, the header phis of the inner loops are inductions with a step
  invariant in the outer loop, or reductions that cross both loops (add,
  mul, and/or/xor, min/max, and the float ones; an `ninf` or `nsw`/`nuw` of
  one is dropped), the loops are tightly nested (a header, a latch and an
  exit block with nothing unsafe), the inner exit condition compares inner
  induction values with a value the outer loop leaves invariant, and the
  LCSSA phis at the exits are the kind the transform moves
  (`LoopInterchangeLegality`);
- the rules of `-loop-interchange-profitabilities` say so in order, the
  first to decide wins: `instorder` counts the base pointers whose access
  steps less in the inner loop than the outer one, against those that step
  more, and interchanges when the bad ones outnumber the good; `vectorize`
  interchanges when only the inner loop is blocked by dependences
  (`LoopInterchangeProfitability`).

The transform ([`transform`](loopinterchange_transform.md)) moves the
inner latch's compare and induction updates into a new latch, makes the
inner header branch to a single block, moves the inner preheader's code
into the outer header, rewires the branches so the old inner loop encloses
the old outer one, swaps the contents of the two preheaders, and rebuilds
LCSSA.

## Deviations from LLVM
- D1: `LoopInfo`, the dominator tree and SCEV are made again after each
  interchange (`restructureLoops`, `DT->applyUpdates` and `forgetLoop` are
  not translated). The pass asks `LoopInfo::analyze` after the branches are
  rewired, which gives the nest `restructureLoops` builds.
- D2: no `LoopCacheAnalysis`: the `cache` rule answers "cannot tell", as
  LLVM's does when the analysis fails; `-cache-line-size` has no use.
- D3: no optimization remarks (`ORE`); `LLRM_DEBUG=interchange` says why a
  nest is refused.
- D4: loops are named by their header block across interchanges, as LLVM
  keeps the `Loop *`; a loop's header is the block its phis sit in.
- D5: the loop pass manager's loop-simplify and LCSSA come from this pass:
  it runs [`loopsimplify`](loopsimplify.md) and [`lcssa`](lcssa.md)
  first.
- D6: DA runs on the `basic-aa` + `tbaa` pipeline, as `looploadelim` does.

## Example
`a[j][i] = a[j][i] + 1` with `i` the outer loop and `j` the inner: the inner
step is the row size, the outer step 4 bytes, so the loops swap and `j`
becomes the outer loop.

## Runs
-O3 and -Omax, once per body, before the vectorizer's rounds.

## Needs
`depanalysis`, `ivdescriptors`, `scev`.

## Keeps
Nothing: a change invalidates every analysis.

## See also
`LoopInterchange.cpp`: `processLoopList` (L751), `populateDependencyMatrix`
(L171), `LoopInterchangeLegality::canInterchangeLoops` (L1649),
`LoopInterchangeProfitability::isProfitable` (L2007),
`LoopInterchangeTransform::transform` (L2237), `adjustLoopBranches` (L2560).
