> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/basicaa.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/basicaa.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/basicaa.rs sha256:4b197797bff3f2407418fa9d021ce74476106fadb6ae82c324176b5f5d4c8bcb -->

# basicaa

BasicAA, a translation of `BasicAliasAnalysis.cpp` at d1106deb71cc behind
the interface of [`crate::aa`](aa.md) (stage S1/S2 of
docs/architecture/basicaa-port.md). Per query: nothing is solved for the
whole function, and what one query learns lives in its `AAQueryInfo`.

## How it works
Translated: `aliasCheck` (empty locations, casts stripped, the same value,
the underlying objects, identified objects, arguments against function-local
objects, an escape source against a non-captured local object, an access
larger than the whole object), `DecomposeGEPExpression` (constant offsets,
variable indices by value and scale, through `GetLinearExpression`),
`subtractDecomposedGEPs` and `aliasGEP` as far as it needs no value ranges:
the inbounds and constant-difference rules, nuw, and the GCD test. Not yet
(deviations in the plan page): `analyzeVariableOffsets`'
ranges, `aliasPHI`, `aliasSelect`, and the mod/ref queries
other than `getModRefInfoMask` and `getModRefInfo(Call, Loc)` (they answer
the top of the lattice). `getMemoryEffects(Call)` and `getArgModRefInfo`
read the attributes.

## Example
Two allocas never alias, and a call cannot touch an alloca nobody holds a
pointer to:

```text
%a = alloca i32
%b = alloca i32        ; alias(%a, %b) = NoAlias
call void @g()         ; getModRefInfo(call, %a) = NoModRef
```

## Provides
`BasicAA`, an [`crate::aa::AAResults`](aa.md). No client reads it yet.
