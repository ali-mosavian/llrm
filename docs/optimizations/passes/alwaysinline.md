> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/alwaysinline.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/alwaysinline.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/alwaysinline.rs sha256:c0d4a740555ebe553891378f6c4b5586a74173bfef1b351bac7520bfe75e12db -->

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

## Example
A call of an `alwaysinline` function is replaced by a copy of its body, and the
function is dropped once nothing calls or addresses it.

```text
define internal i32 @f(i32 %x) alwaysinline { %r = add i32 %x, 1 ; ret i32 %r }
%v = call i32 @f(i32 %a)      ; becomes  %v = add i32 %a, 1, and @f is deleted
```
