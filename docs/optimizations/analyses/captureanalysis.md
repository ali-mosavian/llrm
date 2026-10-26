> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/captureanalysis.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/captureanalysis.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/captureanalysis.rs sha256:e26bd764dcc42e18685318c3f5537162b4959debc85b6989a8520229db621a40 -->

# captureanalysis

`CaptureAnalysis` of `AliasAnalysis.h` at d1106deb71cc: how an object may be
captured before an instruction, with the two providers
`BasicAliasAnalysis.cpp` has.

## How it works
An object that is not a function-local identified one keeps its provenance:
anything may hold it. For a local one, `SimpleCaptureAnalysis` says what
`PointerMayBeCaptured` finds in the whole function, whatever the context
instruction. `EarliestEscapeAnalysis` finds once the instruction that
dominates every capture (`FindEarliestCapture`) and answers "none" for a
context instruction that cannot be reached from it, so a load before the
escape is not read by the escape. The answers are kept per object for the
life of the provider: a batch drops them with it.

A new provider implements `CaptureAnalysis`; `AAQueryInfo` holds one and
`BasicAA` asks it.

DEVIATE A1: `EarliestEscapeAnalysis` takes no `LoopInfo` or `CycleInfo`
(see [`crate::reachability`](reachability.md)); `isNotInCycle` is the successor walk.

## Example
`%slot` is read before the call that captures it, so a context before the
call sees no capture; one at the call sees it:

```mir
define void @f() {
b:
  %slot = alloca i32
  %v = load i32, ptr %slot
  call void @keep(ptr %slot)
  ret void
}
```

From unit test `a_load_before_the_escape_is_not_captured_before` in
`captureanalysis_tests.rs`.

## Provides
`CaptureAnalysis`, `SimpleCaptureAnalysis`, `EarliestEscapeAnalysis`.

## See also
LLVM `AliasAnalysis.h`, `BasicAliasAnalysis.cpp`,
[`crate::capturetracking`](capturetracking.md), [`crate::reachability`](reachability.md).
