> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/availableexternally.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/availableexternally.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/availableexternally.rs sha256:5e3a535b5285ce3d68ba20fa5713002572a2914b5f2e071927947d51106ac022 -->

# availableexternally

LLVM's EliminateAvailableExternally: a body held only to inline from becomes
a declaration again once inlining is done.

## How it works
A function defined `available_externally` is given its body by a runtime's
description so the inliner can price it against the call (it is a candidate
at any size, since no copy of it is emitted). The inliner may replace the
call by the body. What it does not replace stays a call to the runtime's own
routine, so the body must not be emitted as a second copy.

For every such function with a body the pass makes the global a declaration
with external linkage; calls to it are untouched. It invalidates the program
analyses only if it changed something.

```text
 define available_externally @rt(..) { .. }       declare @rt(..)
 call @rt(..)  (not inlined)               ==>    call @rt(..)
```

## Example
Before:

```mir
define available_externally i16 @rt(i16 %x) {
b:
  ret i16 %x
}

define i16 @f(i16 %x) {
b:
  %v = call i16 @rt(i16 %x)
  ret i16 %v
}
```

After:

```mir
declare i16 @rt(i16)

define i16 @f(i16 %x) {
b:
  %v = call i16 @rt(i16 %x)
  ret i16 %v
}
```

The body is gone and the call stays a call of the runtime's routine.

From unit test `a_body_held_to_inline_from_is_a_declaration_after` in
`availableexternally_tests.rs`.

## Runs
A program pass. At -O1 and above it runs after the interprocedural step and
`globaldce`, before `calleepop`; at -O0 it is the only pass that runs.
