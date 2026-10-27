> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/switchcg.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/switchcg.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/switchcg.rs sha256:61737052eba20956c46ed4571cf305ca729f6ac49e5fc7ccc75dd752d5c33d52 -->

# switchcg

LLVM's switch clusters: which cases are one range, a jump table or a bit
test, and where the tree of what is left splits.

A translation of `SwitchLoweringUtils.cpp` at LLVM `d1106deb71cc`
([source](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SwitchLoweringUtils.cpp)),
as pure decisions over a cluster vector: nothing here edits a function.
The emission is a separate step that reads the clusters.

## How it works
`sort_and_rangeify` makes one cluster of the neighbouring cases that share a
target. `peel_dominant_case_cluster` takes out a cluster that holds two
cases in three (`peelDominantCaseCluster`). `find_jump_tables` partitions
the clusters into the fewest dense runs (Kannan and Proebsting), a run of at
least `min_jump_table_entries` becoming a table unless bit tests suit it
better. `find_bit_test_clusters` does the same for runs that fit a word with
at most three targets. `split_work_item` picks the pivot of the balanced
tree over what is left.

Values are signed, as LLVM's `slt`: a case is its value sign-extended from
the switch's width.

## Example
Cases 0..=5 alternating between targets `a`, `b` and `c` are six
clusters; the whole range is suitable for a table, but three targets with
six comparisons suit bit tests (`isSuitableForBitTests`), so the table is
refused and the run is one bit-test cluster.

## Deviations
- DEVIATE S1: a case's `BranchProbability` is `1 / (cases + 1)`, what
  `BranchProbabilityInfo` gives each edge of a switch with no weights and no
  unreachable target; MIR carries no edge probabilities.
- DEVIATE S2: `buildJumpTable` asks `isSuitableForBitTests` before it fills
  the table, not after. The answer does not read the table.
- DEVIATE S3: the jump table's successor probabilities (`JTProbs`,
  `normalizeSuccProbs`) are not kept; MIR has no successor probabilities.
  The cluster keeps its summed probability.
- `findJumpTables` is quadratic in the clusters, as the pin is. It is exact,
  so the decisions match; the switch it runs behind is off.

## Used by
Named by `builder`, `mergedcond`, `switch`, `switches`, `switchlower`.
