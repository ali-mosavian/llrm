> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/float2int.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/float2int.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/float2int.rs sha256:f0db5e781c56d3040b68dd73a5343bebee8ed9e10129d0b77f0fb5947c1a7a87 -->

# float2int

Floating-point arithmetic that only ever meets integers is done on
integers, as LLVM's `Float2Int`.

## How it works
A translation of LLVM `d1106deb71cc` `Float2Int.cpp`, each function linking
the lines it translates. The roots are the `fptoui`, `fptosi` and the
`fcmp` whose predicate has an integer counterpart. `walk_backwards` walks
the use-def graph up from them and joins the instructions that meet into
one class; `walk_forwards` works out, for each, the range of integers it
can hold, from `uitofp`/`sitofp` inputs and the constants; a class whose
range fits the mantissa of its float type and is not poison is rewritten
on the smallest legal integer type that holds it.

```text
  before                          after
  %f = sitofp i8 %a to float      %e = sext i8 %a to i32
  %g = fadd float %f, 1.0         %g = add i32 %e, 1
  %i = fptosi float %g to i32     (the root's uses take %g)
```

DEVIATE (anything MIR or Rust lacks), numbered after `tailcallelim`'s:
29. Float constants are read as `f64`: `float` and `double` constants are
    exact in it, so `roundToIntegral`, the finite and negative-zero tests
    and `convertToInteger` of an integral value into 65 signed bits are
    `round_ties_even`, `is_finite`, `is_sign_negative` and a range test. An
    `x86_fp80` constant is held by MIR as the nearest double, so it is
    refused as `badRange` (`float_constant_is_exact`).
30. `IRBuilder` folds an operation on constants; MIR has no folder, and
    every operand of the new operation is a constant only where the original
    was foldable and earlier passes have folded it.
31. `convert` is iterative, not recursive: a long chain of `fadd` would
    overflow the Rust stack where C++'s is trusted to the machine's. The
    order the new instructions are made in differs (operands last to first
    on the stack), the order of `ConvertedInsts` stays operands before
    users, which `cleanup` relies on.
32. A `uitofp`/`sitofp` of an integer wider than 65 bits is `badRange` at
    once: `castOp` gives it a range of its own width, and `validateRange`
    turns any range wider than 65 bits into `badRange`.
33. `EquivalenceClasses` is a union-find over instruction ids; classes are
    visited in the order their first member was joined, where the C++ set is
    ordered by address.

## Example
`(int)((float)a + 1.0f)` for `a` of `i8` becomes an `add` of `i32` and no
float; see `float2int_tests.rs`.

## Runs
As `float2int@1594`, in the late list after the second `simplifycfg` slot
and before `instsimplify`, at every level
(`PassBuilderPipelines.cpp#L1594`).

## Needs
The dominator tree, to leave unreachable blocks alone.

## Keeps
The dominators, loops and shape: the CFG is not touched.
