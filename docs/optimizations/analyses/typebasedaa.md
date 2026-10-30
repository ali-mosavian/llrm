> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/typebasedaa.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/typebasedaa.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/typebasedaa.rs sha256:86d82aa2a4d4d59cc10afb12106c2f22a64fee4d2a985ecb006053435d0cd2d8 -->

# typebasedaa

`TypeBasedAliasAnalysis.cpp` at d1106deb71cc: two accesses whose `!tbaa`
types are unrelated cannot alias.

## How it works
A tag names a base type, an access type and an offset; a type node names
its parent, or its fields, up to a root. Two accesses may alias unless their
access types hang under one root and neither access may be to a subobject of
the other (`matchAccessTags`, `getLeastCommonType`,
`mayBeAccessToSubobjectOf`: the path from one base type, by offset, reaches
the other's base type). A type of another root, or an access with no tag,
says nothing. `TypeBasedAA` is an `AAResults`: `alias` answers `NoAlias`
for such a pair and `MayAlias` otherwise; a tag marked immutable makes its
location constant memory (`getModRefInfoMask`); a call that carries a
`!tbaa` tag of its own does not touch a location of an unrelated type
(`getModRefInfo(Call, Loc)`). Everything else is the top of the lattice, for
[`crate::aggregate::AggregateAA`](aggregate.md) to meet with the other providers.

## Example
An `int` and a `float` under one root: apart. An `int` and the
`omnipotent char` that is its parent: may alias.

```text
!0 = !{!"Simple C/C++ TBAA"}
!1 = !{!"omnipotent char", !0, i64 0}
!2 = !{!"int4", !1, i64 0}      !3 = !{!"float4", !1, i64 0}
load int4  vs load float4   NoAlias
load int4  vs load char     MayAlias   (char is int4's ancestor)
```

From unit test `unrelated_scalar_types_are_apart_and_an_ancestor_is_not` in
`typebasedaa_tests.rs`.

## Provides
`TypeBasedAA`, `aliases`.

## Needs
[`crate::aa`](aa.md) (`AAResults`, `AAInfo`), [`crate::aggregate`](aggregate.md).

## See also
LLVM `TypeBasedAliasAnalysis.cpp`, `llrm_mir::tbaa`.
