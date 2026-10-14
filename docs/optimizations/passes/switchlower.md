> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/switchlower.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/switchlower.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/switchlower.rs sha256:9a666a55b1f99ae27e2d3ab0a5957dd19a4b638850720bcf8eb8157861c21954 -->

# switchlower

A switch lowered as gcc 13.4 lowers one: a lookup table where every case
only picks constants, bit tests for few targets in a narrow range, and
otherwise a balanced tree of range tests.

## How it works
The cases are sorted and the ones that share a target and follow one
another are made one range (`group_case_labels`); cases to the default
are dropped. The values are ordered signed or unsigned, whichever makes
the narrower span: a `-1, 0, 1` switch is three adjacent values.

Switch conversion (`tree-switch-conversion.cc` `switch_conversion::expand`,
gcc's `-ftree-switch-conversion`, -O2 and up). Every case target is the
join `J` or an empty block that jumps to it, and every phi of `J` takes a
constant from each. With more than two distinct targets, a span of at most
8 values per case label (a range counts twice:
`param_switch_conversion_branch_ratio`), and not better as bit tests, the
phis become loads from a private constant array indexed by `x - min`, behind
one `x - min <= span` test; a hole holds the default's constant.

Bit tests (`bit_test_cluster`). A run of clusters narrower than a word with
at most 3 targets, and 3 cases for one target, 5 for two or 6 for three, is
`1 << (x - min)` tested against one mask per target.

The tree (`switch_decision_tree`). What is left is balanced by count and
tested as `emit_case_nodes` does: a single value by equality, a range by
`x - low <= span` when it is a leaf, a node with children by the compares
`x > high`, `x >= low`.

gcc's jump tables are not made: the backend has no table jump to lower
to, so those cases stay in the tree.

## Runs
Last of the function passes, as gcc's `pass_lower_switch` follows its
optimizations; every level that optimizes. Switch conversion is
`-ftree-switch-conversion`, -O2 and up.

## Example
Before:

```mir
b0:
  switch i32 %k, label %d [ i32 0, label %a
                            i32 1, label %a
                            i32 2, label %a
                            i32 40, label %b ]
```

After, the first three one range and the tree two ranges:

```mir
b0:
  %t = icmp ugt i32 %k, 2
  br i1 %t, label %m, label %a
```
