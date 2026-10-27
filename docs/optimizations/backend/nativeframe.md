> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/nativeframe.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/nativeframe.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/nativeframe.rs sha256:19854c200174ee3afc4e004f6b6458376a7690fc611e0567aa3bcf4f4f55828b -->

# nativeframe

The frame plan of a procedure whose prologue already exists (native-code
input): where it reserves the frame, where it releases it, and which
registers hold values on entry.

## How it works
A `Plan` holds the established reservation (`entry.floor`,
`entry.reserve_at`, the registers saved), the instructions that release the
frame (`releases`), `(instruction, register)` pairs for values that arrive
in registers (`registers`), the outgoing stack-argument cells (`outgoing`),
whether the procedure keeps a frame register (`framed`) and the stack depth
at a return (`return_depth`, not read anywhere).

`pins` reads the plan against the body and gives each value held at one of
those instructions that register, so the allocator keeps it there.

A plan changes the other frame phases:

```text
  frame::of     refuses a body that reaches below entry.floor; leaves the
                outgoing cells out of the floor; keeps pins in native_pins
  Frame::slot   refuses a spill when the procedure is not framed
  prologue      reserves at entry.reserve_at and releases at `releases`,
                not at the entry and the returns; shifts outgoing cells
  assemble      no second frame (relayout)
```

No caller builds a plan: `assemble` passes none to `frame::of`, so
`Frame::native` is always `None` in the compiler and the paths above are
not taken.

## Example
No honest example: no caller builds a plan, so no LIR is ever shaped by it.
Read `frame::of` for where a plan would enter.

## Used by
`frame` (`Frame::native`), `prologue`, `relayout`, `assemble` and
`flow::machine_with` (which adds `native_pins` to the allocator's pins).
