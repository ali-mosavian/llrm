> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/cgpsplitgep.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/cgpsplitgep.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/cgpsplitgep.rs sha256:cc599061b0cf6beb0e68d4287973d24983d30e3d1c93a9a1cbde9bda4af00d1f -->

# cgpsplitgep

CodeGenPrepare's `splitLargeGEPOffsets`: the constant-offset GEPs of one
base that the target cannot fold into an address are made from one new base.
[CodeGenPrepare.cpp#L6902](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L6902)

## How it works
A load or store whose address is a `gep` of constant indices off an
argument, a global or an instruction that is no cast or `gep`, at a
positive offset the target's addressing mode does not take, is recorded
under that base (`matchOperationAddr`, the `EnableGEPOffsetSplit` arm). Per
base the `gep`s sort by offset; where they do not all share one, a new base
`gep i8, base, B` is made (at the base's definition, or in the entry block
for an argument or a global) and each `gep` becomes `gep i8, new, offset -
B`, as long as that fits the addressing mode: a farther one starts a new
base. `B` is the target's `getPreferredLargeGEPBaseOffset` of the smallest
and largest offset where it names one, else the smallest offset itself.

## Runs
In `cgpaddr`, for a target whose `shouldConsiderGEPOffsetSplit` is true
(arm64).

## Example
A struct field at 11936 and others at 12938 and 12944 become one `add` of
11936 and offsets 0, 1002 and 1008 from it, where each was a `mov` of its
own offset and a register-offset access.

```text
%a = gep i8, %s, 11936      %n = gep i8, %s, 11936
%b = gep i8, %s, 12938  =>  %b = gep i8, %n, 1002
%c = gep i8, %s, 12944      %c = gep i8, %n, 1008
```
