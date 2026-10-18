> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/loopaccess.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/loopaccess.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/loopaccess.rs sha256:25f602e3615c6df1a68d2493542077e60a666756b74d0c0f53eba59a10141d63 -->

# loopaccess

`getPointersDiff` and `isConsecutiveAccess`.

## How it works
A translation of LLVM `d1106deb71cc` `LoopAccessAnalysis.cpp`: the
distance between two pointers in units of an element, from their constant
offsets off a common base, else from ScalarEvolution's constant
difference of the two.

## Example
`getelementptr i32, ptr %p, i64 1` is consecutive after `%p`.
