---
architecture-index: PartiallyInlineLibCalls rows
architecture-order: 9110
---

# PartiallyInlineLibCalls rows

[`partialinline.rs`](../../crates/opt/llrm-transforms/src/partialinline.rs) translates the pin's `PartiallyInlineLibCalls` (`llvm/lib/Transforms/Utils/PartiallyInlineLibCalls.cpp`): a `sqrt` that may set errno becomes the instruction plus a guarded libcall on the NaN/negative path, where the target has a fast sqrt (x86, arm64, wasm).

## PIL-SIZE: bigger code on a program that calls sqrt

Worse than before the pass; the pin does the same (fast path inline, libcall kept on the cold path).

| program | level | off | on |
|---|---|---|---|
| sqrt_norm (`tests/`, not a corpus cell) | m32 -O2/-O3 | 897 B | 1297 B |
| sqrt_norm | m64 -O2 | 869 B | 1212 B |

Owner: llrm-greedy-anyisa. Exit: 2026-11-20, after the sqrt_norm m32 clocks are measured (vsgcc 486 model, a stub `sqrt` with no errno wrapper prices the off case low) and the identity of the pass against the pin on the sampled corpus is shown; the row is removed if the clocks gain.

The corpus has no sqrt call: the pass moves 0 of 566 sampled cells.
