> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/hooks.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/hooks.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/hooks.rs sha256:2e1b55143a98eeead7242a8adcfbecbd0796907ce69ef524b7d59d40ac5da1ad -->

# hooks

What the shared backend asks of a target's own backend code.

A target whose code sits above this crate (`llrm-x86-backend`) installs
itself once; the shared passes reach it here, as LLVM's passes reach a
target through `TargetRegistry` and not by name.

## Used by
Named by `assemble`, `builder`, `dag_select`, `shadow`.
