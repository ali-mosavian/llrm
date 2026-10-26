> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/capturetracking.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/capturetracking.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/capturetracking.rs sha256:6691ee1fe4cf85597a3f4622efdc8dcf2633057db1719306406647e1f0cbe985 -->

# capturetracking

`CaptureTracking.cpp` at d1106deb71cc: which components of a pointer a use
of it keeps (`DetermineUseCaptureKind`), and the walk over the uses of a
value that asks a tracker about each (`PointerMayBeCaptured`).

## How it works
The walk starts at a value's uses. Each use is classified by its user: a
load or a store's address captures nothing, a store of the pointer, a
`ptrtoint`, a `ret` or an unknown user captures it all, a `gep`, cast,
`phi` or `select` passes it on to its result (the result's uses are then
walked), a call keeps what its argument and its callee's parameter
attributes allow. A use that keeps something is handed to the
`CaptureTracker`, whose answer says whether to stop, go on, or go on
without following the user's result. Past 150 uses (`capture-tracking-max-
uses-to-explore`) the tracker is told `too_many_uses`.

DEVIATE C1: MIR has no `captures(...)` attribute: `nocapture` is
`captures(none)`, anything else is all components. DEVIATE C2: no
pointer-returning intrinsics (`launder.invariant.group`, `ptrmask`,
`threadlocal.address`), atomics, `va_arg`, `ptrtoaddr`, volatile memory
intrinsics or `!captures` metadata exist in MIR, so those cases are absent.
`PointerMayBeCapturedBefore` is not translated; `FindEarliestCapture` is
(`find_earliest_capture`), and [`crate::captureanalysis`](captureanalysis.md) answers
`getCapturesBefore` with it.

## Example
`%p` is stored to memory, so it is captured; `%q` is only loaded through:

```mir
define void @f(ptr %p, ptr %q, ptr %slot) {
b0:
  store ptr %p, ptr %slot
  %v = load i32, ptr %q
  ret void
}
```

From unit test
`a_stored_pointer_is_captured_and_a_loaded_through_one_is_not` in
`capturetracking.rs`.

## Provides
`CaptureComponents`, `UseCaptureInfo`, `CaptureTracker`,
`determine_use_capture_kind`, `pointer_may_be_captured_by`,
`pointer_may_be_captured`, `find_earliest_capture`.

## See also
LLVM `CaptureTracking.cpp`, `Support/ModRef.h` (`CaptureComponents`).
