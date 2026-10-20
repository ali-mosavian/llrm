> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/initializes.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/initializes.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/initializes.rs sha256:6c3025495ccc8be53d3589fe395e9969ae1a4b0813022dc003a0f6892bc1b92a -->

# initializes

`initializes(...)` of the pointer parameters of a function: the bytes it
writes before anything reads them, as `inferInitializes` states them
(FunctionAttrs.cpp:1118,
<https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/IPO/FunctionAttrs.cpp#L1118>).

## How it works
The uses of a parameter are walked through constant-offset GEPs
(`collectArgumentUsesPerBlock`); each user gets the bytes it writes, reads
or clobbers (`getArgumentAccessInfo`). A block's initialized ranges are the
intersection of its successors', then, from its last use to its first: a
write adds its bytes, a read removes them, an unknown use or a call that
also captures the pointer clears them. The entry block's ranges are the
attribute. No points-to solve: a use walk and one pass over the blocks.

DEVIATE I1: run after the SCC's argument attributes are final
(`addArgumentAttrs` runs it per parameter, before the argument graph is
settled), so a call inside the SCC sees the callee's final
`writeonly`/`nocapture`: the same or more precise ranges.
DEVIATE I2: a load or store is "simple" when it is not volatile; MIR has
no atomic ordering on them.

## Example
`fill` writes the first two bytes of `%p` and reads nothing:

```mir
define void @fill(ptr %p) {
b0:
  store i16 1, ptr %p
  ret void
}
```

`%p` gets `initializes((0, 2))`. From unit test
`a_store_through_the_parameter_initializes_its_bytes` in
`initializes_tests.rs`.
