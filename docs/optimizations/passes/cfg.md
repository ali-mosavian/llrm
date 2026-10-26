> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/cfg.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/cfg.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/cfg.rs sha256:3263142ab45d0c45ef85b811b559ee02ace721ac7ed4e858e5917e79076165a9 -->

# cfg

Control-flow cleanup: a block that jumps unconditionally to a block only it
reaches takes that block's instructions (LLVM's SimplifyCFG
`MergeBlockIntoPredecessor`).

## How it works
`merged` visits the blocks in layout order and, for each block `first`,
merges its successor into it while it can. The successor `second` merges
when

- `first` ends in an unconditional `br` and `second` is its only successor;
- `first` is `second`'s only predecessor, and `second` is neither the entry
  nor `first`;
- every phi of `second` has `first` as its only incoming block, and the
  values they take are not themselves phis of `second` (a swap cycle is
  refused before anything changes).

Then the `br` is erased, each phi's value is replaced by what it received
from `first`, the phis go, the instructions of `second` move to the end of
`first`, uses of `second` as a block name `first`, and `second` is erased.
`first` is asked again, since its new terminator may reach another
single-entry block. A two-way branch or any other terminator is never
merged.

```text
  first:  ...             first:  ...
          br second   ==>         <instructions of second>
  second: %p = phi [%v, first]    <terminator of second>
          ...
          <terminator>
```

The merged block keeps `first`'s place in the layout.

## Example
Before:
```mir
define i16 @f(i16 %x) {
b0:
  %v = add i16 %x, 7
  br label %b1

b1:
  %joined = phi i16 [ %v, %b0 ]
  ret i16 %joined
}
```
After:
```mir
define i16 @f(i16 %x) {
b0:
  %v = add i16 %x, 7
  ret i16 %v
}
```
`b1` had `b0` as its only predecessor, so its phi became `%v` and its `ret`
joined `b0`.

From unit test `test_single_entry_phi_is_replaced` in `cfg_tests.rs`.

## Runs
A library function, called by `decide` after it decides branches and by
`rotate` and `lsr` after their rewrites. The pass `Merged` wraps
it; no pipeline stage runs it alone.

## Keeps
Blocks change: every analysis is invalidated when anything merged.
