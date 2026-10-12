> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/availableexternally.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/availableexternally.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/availableexternally.rs sha256:8bead6c322dac1ee4c0e9382e01c780faf82d7613a8a6cccfac8ff1943980771 -->

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

## Runs
A program pass. At -O1 and above it runs after the interprocedural step and
`globaldce`, before `calleepop`; at -O0 it is the only pass that runs.
