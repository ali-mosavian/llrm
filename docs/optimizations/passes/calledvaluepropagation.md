> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/calledvaluepropagation.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/calledvaluepropagation.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/calledvaluepropagation.rs sha256:65fe1baae77a6c644d4eaf8bf2a1e37b343e582be9097261359e7c09f921adb1 -->

# calledvaluepropagation

LLVM's `CalledValuePropagationPass`: `!callees` on an indirect call whose
targets are few and known.

## How it works
A sparse solver over a lattice of function sets (at most four functions,
then overdefined) in three groups: SSA registers, function returns and the
memory of a global variable only loaded and stored whole. Arguments of a
local function whose address is not taken are tracked through its direct
calls; returns of a function with an exact definition through its callers.
Control flow is followed as `SparseSolver` does: only executable blocks and
feasible edges are visited. After it settles, each indirect call whose
called value is a non-empty function set gets `!callees` listing them, by
name.

## Example
```text
@slot = internal global ptr @only        %f = load ptr, ptr @slot
%f = load ptr, ptr @slot                 call void %f(), !callees !{ptr @only}
call void %f()
```

## Runs
After IPSCCP (PassBuilderPipelines.cpp:1232), inside the early
interprocedural step.

## See also
[CalledValuePropagation.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/IPO/CalledValuePropagation.cpp),
[SparsePropagation.h](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/include/llvm/Analysis/SparsePropagation.h),
[ValueLatticeUtils.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/ValueLatticeUtils.cpp).
