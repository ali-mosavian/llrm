> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/constmerge.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/constmerge.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/constmerge.rs sha256:0db3694166cf94e8164d91eea905806ec563e60692c05a3c74acb6b368336178 -->

# constmerge

Identical constants of a module are one, as LLVM's `ConstantMerge`.

## How it works
A translation of LLVM
[`ConstantMerge.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/IPO/ConstantMerge.cpp)
at d1106deb71cc. A constant variable with an initializer, in the default
address space, in no section, not named by `llvm.used` or
`llvm.compiler.used` and not weak for the linker, is keyed by its
initializer. Of the variables with one key, the best is canonical: external
over local, then `unnamed_addr` over not; of equals the last wins
(`IsBetterCanonical`). Every other one with local linkage goes to the
canonical one when at least one of the two is `unnamed_addr` (the merged
variable loses `unnamed_addr` if the one it absorbed had none), and the
canonical variable takes the larger alignment. The step repeats while it
merges anything: variables whose initializers named two merged ones now
hold the same value.

## Example
```text
 before                                    after
 @a = private unnamed_addr constant        @b = private unnamed_addr constant
        [2 x i32] [i32 1, i32 2]                  [2 x i32] [i32 1, i32 2]
 @b = private unnamed_addr constant        ; a load of @a loads @b
        [2 x i32] [i32 1, i32 2]
```

DEVIATE 1: a local variable nothing uses is not deleted here (the pin's pass
does); `globaldce` ran before and ours has no stage after this.
DEVIATE 2: the alignment of an unstated one is the type's preferred
alignment, without the 16-byte raise of a large initialized global.
MIR keeps no comdat, thread-local flag or global metadata, so the pin's
refusals on those have nothing to refuse.

## Parity slot
`constmerge`, the last module pass of the pin's pipeline
(PassBuilderPipelines.cpp#L1714), after the function passes.

## Used by
Named by `pipeline`.
