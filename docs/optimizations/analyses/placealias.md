> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/placealias.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/placealias.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/placealias.rs sha256:0d5aa2154386dde93959c4a16f456b80536249852704247e161fb46b00ba84b2 -->

# placealias

Whether two memory accesses can touch the same byte, from what their
addresses name: the pin's `BaseIndexOffset::computeAliasing`, on facts a
caller has already read off its own IR (DAG nodes, machine operands). It
knows no IR: an `Object` is a kind and integers, a size a number.

## How it works
`alias_of` takes the byte distance between the accesses when the caller
knows it (same base and index), and each access's `Object`. A known
distance decides by the sizes. Otherwise accesses of different kinds of
object, two different globals, or two frame objects of which one is an
alloca, are apart; anything else is `MayAlias`.

## Used by
The DAG combiner's chain walk (`mayAlias`) and arm64's load/store pairing.
