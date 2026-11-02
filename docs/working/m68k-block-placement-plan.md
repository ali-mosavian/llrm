# Block placement for the m68k spike (plan)

Goal: `MachineBlockPlacement` (pin d1106deb71cc) over `llrm-spike-mf`'s `MFunction`, generic over a small block trait, so
the x86 LIR can implement the same trait after the main merge and `llrm-codegen/src/backend/jumps/placement.rs` goes.
Row it moves: library `partition` (`beq .exit; bra .header` latch becomes one `bne`); gcd, strlen, countgt mention loop
rotation too. Do not edit the x86 LIR file (parity is frozen).

## Inputs, already in the tree
- Frequencies: `llrm_mir::blockfrequency::BlockFrequencyInfo::calculate<G: FlowGraph, P: EdgeProbabilities>` is generic;
  give it an `MFunction` adapter (`FlowGraph` + `DomGraph`, `BlockId(index)`) and a `CycleInfo`. `spike-mf` needs an
  `llrm-mir` dependency (check `tools/test_layers.py`).
- Edge probabilities: IR `BranchProbabilityInfo` (`llrm-analysis/src/bpi.rs`) per IR block id; `MFunction` blocks carry the IR
  block id (`Block::id`). Blocks a pass made (cmov diamonds, split edges) default to even odds, as the pin's `addSuccessor`.
  Store them as `MFunction::edge_probability: HashMap<(u32, u32), f64>`.
- Loops, dominators: `spike-mf/src/cfg.rs`. Branch reversal: `Jump::reverse` (`sink.rs`).

## Translate (MachineBlockPlacement.cpp line numbers)
BlockChain (L260-), markChainSuccessors/markBlockSuccessors (L713-760), collectViableSuccessors (L762),
selectBestSuccessor + getBestNonConflictingEdge (L1077), hasBetterLayoutPredecessor (L1479), isTrellis (L1023),
selectBestCandidateBlock (L1778), getFirstUnplacedBlock (L1846), fillWorkLists (L1888), buildChain (L1920),
canMoveBottomBlockToTop (L2010), FallThroughGains (L2083), findBestLoopTopHelper (L2171), hasViableTopFallthrough (L2390),
rotateLoop (L2423), buildLoopChains (L2706), buildCFGChains (L2799), optimizeBranches (L2964), run (L3568).

Out, with DEVIATE notes: tail duplication (L840-1022, 1210, 3155-3480), `alignBlocks` (m68k has no alignment),
`rotateLoopWithProfile`, ExtTSP, `precomputeTriangleChains` until a row needs it.

## Apply
New order of `MFunction::blocks`; then per block fix the terminators: a `BRA` to the next block goes (`expand::falls_through`
already drops it), a conditional branch whose taken target is next is reversed with `Jump::reverse` and the `BRA` swapped.
The entry block stays first. Behind `Options::overlay` first; measure both bench sets.

## Tests (fail-first)
Loop with the exit after the latch (the partition shape), diamond, nested loop, irreducible CFG left alone, a branch with a
call between, entry stays first; run-checked under qemu for a loop with two exits.

## After it
Cross-block `(An)+` for partition on text with a real basic-block parse (`chain.rs`), negative tests: second latch, a
conditional arm that skips the step, a use of the pointer in the arm.
