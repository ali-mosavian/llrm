> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/alwaysinline.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/alwaysinline.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/alwaysinline.rs sha256:9b859a381e1b4835f463aba4b40fa5c6ff71cf57214b6aaff634a3943dc1ac25 -->

# alwaysinline

`AlwaysInlinerPass(InsertLifetimeIntrinsics=true)`: every call of an
`alwaysinline` callee is replaced by a copy of it, before the inliner
prices anything.

## How it works
`PassBuilderPipelines.cpp` l.1326 runs it as a module pass just ahead of the
CGSCC inliner. Each body is gone through once for the direct calls the
callee's or the call's `alwaysinline` names (`inline_viable`: no recursion,
no `returns_twice`), and the copies' own such calls follow. A callee left
with no call and no address is dropped, as the pass deletes it.

What the copy leaves is the point: `hot_op(S, hot_i_and)` passes
`@hot_i_and` as a value, and only the copy makes the call to it direct. The
inliner then prices that call with the callee's other uses already gone
(its last call, `isSoleCallToLocalFunction`) and walks the callees first.

## Runs
Once per compile, before `Walk`.

## Used by
`pipeline`.
