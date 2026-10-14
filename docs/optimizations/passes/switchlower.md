> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/switchlower.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/switchlower.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/switchlower.rs sha256:277689b5bb4648b0db6d5a655805e0d27ad5067393c89ae17eed0dff630f0ded -->

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

Jump tables (`jump_table_cluster`), over the plain clusters left: the fewest
runs whose span is at most 8 times (3 tuned for size) the comparisons they
replace, and at least the target's `jump_table` cases (gcc's
`case_values_threshold`, 5 on x86; 0: no table jump). Such a run is `x -
low` behind its range test and a `switch table` over the indices `0..n`,
every index a block that only jumps on; the backend makes it a jump through
a table of code addresses. `-fno-jump-tables` leaves them in the tree.

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
