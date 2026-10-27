> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/inferalignment.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/inferalignment.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/inferalignment.rs sha256:db4950fef7684ace80d1120f8d4d28370ef52ecaf1877820faa2cbf453fb1957 -->

# inferalignment

Loads, stores and memory intrinsics get the alignment their pointer is
known to have, as LLVM's `InferAlignment`.

## How it works
A translation of
[`InferAlignment.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/InferAlignment.cpp)
at d1106deb71cc. A first walk raises an alloca to the preferred alignment
of what a load or store through it accesses (`tryEnforceAlignment`). A
second walk, down the dominator tree, gives each load, store and
`memcpy`/`memmove`/`memset` pointer argument the alignment the low zero bits
of the pointer prove (`computeKnownBits`), or that another access to the
same base pointer higher in the tree proved. `ptrtoint p & mask` and
`trunc (ptrtoint p)` fold to what that alignment decides.

## Example
```text
 before                                after
 call @llvm.memcpy(ptr @b, ptr @a,     call @llvm.memcpy(ptr align 4 @b,
                   i32 100, i1 0)                        ptr align 4 @a, i32 100, i1 0)
```
with `@a` and `@b` `align 4`: the backend then copies in dwords.

DEVIATE 1: `tryEnforceAlignment` does not raise the alignment of a global
variable (it returns the global's own): a function pass cannot reach the
module's globals. Exit: a module-level step before the function passes.

DEVIATE 2: `masked.load`/`masked.store` are not looked at (MIR has no
vectors), nor are atomic memory operations (MIR has none).

DEVIATE 3: `isPassingUndefUB` reads the call's own parameter attributes,
not the callee declaration's.

## Parity slot
`infer-alignment@1362` and `infer-alignment@1498`
(PassBuilderPipelines.cpp#L1362, #L1498).

## Keeps
Only alignments, and the operands of the folds above, change: blocks and
edges are unchanged.

## Used by
Named by `pipeline`.
