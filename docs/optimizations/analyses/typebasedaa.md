> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/typebasedaa.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/typebasedaa.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/typebasedaa.rs sha256:9209305da6ccd72ddfe948b8ed93d6241b1fb7767b26b04985d2f79f4f53b50b -->

# typebasedaa

`TypeBasedAliasAnalysis.cpp` at d1106deb71cc: two accesses whose `!tbaa`
types are unrelated cannot alias.

## How it works
A tag names an access type; a type node names its parent, up to a root.
Two accesses may alias unless their types hang under one root and neither
type is an ancestor of (or equal to) the other (`matchAccessTags`,
`getLeastCommonType`). A type of another root, or an access with no tag,
says nothing. `TypeBasedAA` is an `AAResults`: `alias` answers `NoAlias`
for such a pair and `MayAlias` otherwise; a tag marked immutable makes its
location constant memory (`getModRefInfoMask`); a call that carries a
`!tbaa` tag of its own does not touch a location of an unrelated type
(`getModRefInfo(Call, Loc)`). Everything else is the top of the lattice, for
[`crate::aggregate::AggregateAA`](aggregate.md) to meet with the other providers.

DEVIATE F1: only scalar tags are understood: base type equal to the access
type, offset 0, and a type node of a name, a parent and an offset of 0.
The C front end emits only these, as clang does for C. A struct-path tag
or a type node with fields answers `MayAlias`, and a test pins that.
Exit: a front end that emits struct-path tags.

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
