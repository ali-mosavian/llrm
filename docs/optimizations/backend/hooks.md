> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/hooks.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/hooks.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/hooks.rs sha256:7e961d13e6f6024254edb468fc42f2165d49d724d8bafe372111e5ee219c0595 -->

# hooks

What the shared backend asks of a target's own backend code.

A target whose code sits above this crate (`llrm-x86-backend`) installs
itself once; the shared passes reach it here, as LLVM's passes reach a
target through `TargetRegistry` and not by name.

## Example
The x86 backend installs its hooks once, and a shared pass then reaches it
through `installed` without naming the target.

```text
llrm_x86_backend:  hooks::install(&X86_HOOKS)
shared pass:       hooks::installed()?.select_function(module, name, ...)
```

## Used by
Named by `assemble`, `builder`, `dag_select`, `shadow`.
