> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/selectconstants.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/selectconstants.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/selectconstants.rs sha256:4943212cc4a7326cef7919fb929975919111d2c94ee953dca739d816b73900f6 -->

# selectconstants

A select between two constants made arithmetic on its condition.
Translated from `DAGCombiner::foldSelectOfConstants` and
`foldSelectOfConstantsUsingSra`
([DAGCombiner.cpp#L13011](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L13011)),
with X86's `convertSelectOfConstantsToMath` as the target's answer.

## How it works
`select c, C1, C2` of an integer wider than one bit is, by the constants:

```text
 1, 0           zext c                    -1, 0          sext c
 0, 1           zext (c ^ 1)              0, -1          sext (c ^ 1)
 C1, C1-1       (zext c) + C1-1           C1, C1+1       (sext c) + C1+1
 2^k, 0         (zext c) << k             -1, C          (sext c) | C
 C, -1          (sext (c ^ 1)) | C
 C1, -1         (x >>s w-1) | C1   when c is  x > -1
 C1, 0          (x >>s w-1) & C1   when c is  x < 0
```

The first four need no target answer; the arithmetic ones ask the target
(`Tti::convert_select_of_constants_to_math`), and X86 says yes for every
scalar integer. The sign-bit forms apply only when the compare has no other
reader and compares a value of the select's own type. What no row covers
stays a select, which `selects` turns into a branch where the target has no
conditional move.

A select of one-bit values is `foldBoolSelectToLogic` (`boolean`): `select
c, c|1, f` is `c | f`, `select c, t, c|0` is `c & t`, `select c, t, 1` is
`!c | t`, `select c, 0, f` is `!c & f`. The pin freezes the other operand,
which no machine value needs, so it is left out.

## Example
`%r = select i1 %less, i32 4, i32 0` is `%z = zext i1 %less to i32` and
`%r = shl i32 %z, 2`: at m32 i486 `setl al; shl eax, 2`, no branch.

## Runs
Once per program in `driver::optimized`, in front of `selects::lowered`
where the target has no conditional move, and at every -O level.
