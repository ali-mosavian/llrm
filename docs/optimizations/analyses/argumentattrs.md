> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/argumentattrs.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/argumentattrs.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/argumentattrs.rs sha256:0b8338422968c0aa94e72883587d96c42784d8732c0712d66e77979ba5b7b2d5 -->

# argumentattrs

`nocapture`, `readonly`, `writeonly` and `readnone` of the pointer
parameters of one call-graph SCC, as `addArgumentAttrs` infers them
(FunctionAttrs.cpp:1238).

## How it works
A parameter's uses are walked by capture tracking
([`crate::capturetracking`](capturetracking.md)) with a tracker that does not count a call to
a member of the SCC as a capture: it records the callee's parameter
instead, as an edge of the argument graph. A parameter with no such edge
is settled at once: `nocapture` if nothing keeps it. The rest are the
graph's strongly connected components, callees first: an SCC of arguments
keeps what the union of its members and of what they pass on to outside
the SCC keeps. `readonly`, `writeonly` or `readnone` come from the uses
that can still reach the pointer's memory (`determinePointerAccessAttrs`):
loads, stores, and calls by what their `memory(argmem: ...)` and the
parameter's attributes allow, with a pass to a member of the SCC assumed
harmless.

A parameter no use can free is `nofree` too (`IsFree`): a write through it
by a call whose site, callee and parameter do not say `nofree` may free it.

A parameter the entry block's guaranteed calls pass on to a `nonnull`
`noundef` parameter of their callee is `nonnull`
(`addArgumentAttrsFromCallsites`): the call runs on every entry, so the
value is non-null throughout. DEVIATE A4: `nofpclass` has no MIR spelling,
so only `nonnull` is propagated.

DEVIATE A1: `nofree_obj` has no MIR spelling, so only `nofree` is looked
for. DEVIATE A2: only `captures(none)` can be stated (see
[`crate::capturetracking`](capturetracking.md)), so a parameter that would be
`captures(ret: ...)` stays unstated. Not translated yet:
`returned`; `inferInitializes` is
[`crate::initializes`](initializes.md).

## Example
`get` reads through `%p` and never keeps it; `put` writes through it:

```mir
define i32 @get(ptr %p) {
b0:
  %v = load i32, ptr %p
  ret i32 %v
}
```

`%p` is `nocapture readonly`. From unit test
`a_load_only_parameter_is_nocapture_readonly` in `argumentattrs_tests.rs`.

## Runs
`functionattrs::post_order` once per SCC, callees first.

## See also
LLVM `FunctionAttrs.cpp` (`addArgumentAttrs`, `addAccessAttrs`,
`determinePointerAccessAttrs`, `ArgumentUsesTracker`),
`CaptureTracking.cpp`.
