> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/icmpchain.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/icmpchain.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/icmpchain.rs sha256:87c02d258bdcaff4738d47dd359a5d9efedc8e9e366a4bd92b987117b9da6efd -->

# icmpchain

A branch on an or/and chain of compares of one value with constants,
turned into a switch (or a range check): LLVM's
[`SimplifyCFGOpt::simplifyBranchOnICmpChain`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L5469)
with `ConstantComparesGatherer`
([L580](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L580)).

```text
  %a = icmp eq i16 %x, 1                 switch i16 %x, label %f [
  %b = icmp eq i16 %x, 5                   i16 5, label %t
  %c = or i1 %a, %b                        i16 1, label %t ]
  br i1 %c, label %t, label %f
```

One clause that is not a compare of the value is allowed: it is tested by
a branch of its own first (`switch.early.test`), frozen unless it is known
not to be undef or poison.

| # | DEVIATE |
|---|---|
| D1 | A pointer constant (`null`, `inttoptr`) is not a compared integer (`getConstantInt`'s pointer arm), so no value is a pointer and `ptrtoint` is never made. |
| D2 | No branch weights; `!unpredictable` is kept. |
| D3 | `!llvm.loop` of the old branch is not copied, as `IRBuilder::CreateCondBr` does not. |
