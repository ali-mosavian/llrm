> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/cgpsinkops.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/cgpsinkops.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/cgpsinkops.rs sha256:20be4f5adae88dd82dcaa661146ed683b445b0fdf9ef920dd56f63e11d782ee8 -->

# cgpsinkops

CodeGenPrepare's `tryToSinkFreeOperands`: operands the target folds into
an instruction are made again beside it
([CodeGenPrepare.cpp#L7994](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L7994)).

## How it works
Isel works a block at a time. An operand made in another block (an
extension, a splat shuffle, a lane-0 insert) crosses as a register and the
instruction cannot fold it (a `v2i64` multiply of two `sext` is one
`smull` only with the `sext`s in view). The target's
`isProfitableToSinkOperands` names the uses to remake; each is cloned
before the instruction (before the dominating one, in a chain), the
original goes once nothing reads it.

```text
b1: %e = sext <8 x i16> %a to <8 x i64>     b2: %e = sext <8 x i16> %a to <8 x i64>
b2: %m = mul <8 x i64> %e, %s            =>     %m = mul <8 x i64> %e, %s
```

## Runs
In `cgpaddr`'s run, before `SinkCast`.

## Example
An operand the target folds into its user, made in another block, is made again
beside the user.

```text
b1: %e = sext <8 x i16> %a to <8 x i64>     b2: %e = sext <8 x i16> %a to <8 x i64>
b2: %m = mul <8 x i64> %e, %s            =>     %m = mul <8 x i64> %e, %s
```
