> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/cgpbranch.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/cgpbranch.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/cgpbranch.rs sha256:c301ba906f0a561d86c0e5df5595c00b6f9178f7252ea48f6280db0b53054d75 -->

# cgpbranch

CodeGenPrepare's `optimizeBranch`: a branch on a compare with a constant
becomes a branch on a compare with zero of a value a successor computes
anyway, where the target prefers one
([CodeGenPrepare.cpp#L8878](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L8878)).

## How it works
Only where `Lowering::prefer_zero_compare_branch` says so (x86 does,
`X86ISelLowering.cpp` L3805). For a block that ends in a branch on an
`icmp` of `%x` with a constant, read by nothing else, each user of `%x`
in the block or in a successor with one predecessor is looked at:
- `icmp ult %x, 2^k` and `lshr`/`ashr %x, k`: the shift is compared with
  zero (`icmp eq`);
- `icmp eq|ne %x, c` and `add %x, -c`, `sub %x, c` or `xor %x, c`: that
  value is compared with zero by the same predicate.

The user moves to just before the branch (its poison-generating flags
dropped: it runs on paths it did not), the new compare is made there and
takes the old one's place. One conversion per branch.

```text
 b0: %c = icmp ult i16 %x, 8            b0: %t = lshr i16 %x, 3
     br i1 %c, label %b1, label %b2         %c.0 = icmp eq i16 %t, 0
 b1: %t = lshr i16 %x, 3                    br i1 %c.0, label %b1, label %b2
```

## Example
`test_an_unsigned_compare_with_a_power_of_two_becomes_a_compare_of_the_shift_with_zero`
in `cgpbranch_tests.rs` is the sketch above.

## Runs
In `cgpaddr`'s run, before the compare sinks.
