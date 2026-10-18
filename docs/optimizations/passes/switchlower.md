> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/switchlower.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/switchlower.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/switchlower.rs sha256:f3948232e4f621259761b7cf41ad424d22600ce2bf82c1e8bf5bfbf13bbcc385 -->

# switchlower

A switch lowered as LLVM's SelectionDAG lowers one: the likeliest range
peeled off, jump tables and bit tests for what is dense, and a balanced
tree of range tests over the rest. Switch conversion (lookup tables) is the
`switchconv` pass.

## How it works
`visitSwitch` ([`switchcg`](switchcg.md) holds the decisions, a
translation of `SwitchLoweringUtils.cpp`): cases to one target that follow
one another are one range, values ordered signed; `peelDominantCaseCluster`
takes a range of at least 66% out ahead; `findJumpTables` and
`findBitTestClusters` replace runs by tables and bit tests.

`splitWorkItem` splits more than three clusters at the pivot that balances
their probability, `value < pivot` to the one side, the other to the rest;
at most three are `lowerWorkItem`: the likeliest first, each falling to the
next and the last to the default, two values one bit apart one `(x | bit)
== both` compare, a range `x - low <= span`.

A jump table is the range test of `x - low` and a `switch` over the indices
`0..n`, every index a block that only jumps on; the backend makes it a jump
through a table of code addresses. Bit tests are `1 << (x - low)` against
one mask per target (`visitBitTestCase`: a single bit or all but one is a
compare). Where the default is `unreachable` the range test goes.

Switch conversion (gcc's `-ftree-switch-conversion`, -O2 and up; LLVM's
`SimplifyCFG::SwitchToLookupTable`). Every case target is the join `J` or
an empty block that jumps to it, and every phi of `J` takes a constant from
each. Where bit tests are not better (`switch_conversion::expand`), the
phis become loads from a private constant array indexed by `x - min`,
behind one `x - min <= span` test; a hole holds the default's constant.
One decision makes it: at least three case values at a density of 10% of
the span (40% tuned for size; LLVM's `isSwitchDense`), and the lookup
priced under the tree it replaces by the target's costs. A path of the
tree is the average depth of a balanced tree of compare and branch, and a
move for each phi; the table's the range test and a load for each phi, a
widening where its cells are narrower. Tuned for size the costs are bytes
and the whole tree is set against the code and the data of the table.

