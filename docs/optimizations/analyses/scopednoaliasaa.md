> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/scopednoaliasaa.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/scopednoaliasaa.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/scopednoaliasaa.rs sha256:39e739feb4864932519b9ce5d03384d7d4129482462e2e659b770d5f69984936 -->

# scopednoaliasaa

`ScopedNoAliasAA.cpp` at d1106deb71cc: two accesses are apart when, in some
scope domain, every scope one is in is named `!noalias` by the other.

## How it works
`!alias.scope` lists the scopes an access is in and `!noalias` those it
promises not to alias. A scope is a node `!{self, domain, name}`. Accesses
`A` and `B` do not alias if, for some domain, all of `A`'s scopes in it are
among `B`'s `!noalias` ones, or the other way round. An access with either
list missing says nothing. A call or fence carries the lists on itself.
`ScopedNoAliasAA` is an `AAResults`; what it does not decide is the top of
the lattice, for [`crate::aggregate::AggregateAA`](aggregate.md) to meet with the others.

## Example
A load in scope `!1` and a store with `!noalias !{!1}`: apart.

```text
%a = load i32, ptr %p, !alias.scope !2        !2 = !{!1}
store i32 0, ptr %q, !noalias !2              !1 = distinct !{!1, !0}
```

## Provides
`ScopedNoAliasAA`, `may_alias_in_scopes`.

## Needs
[`crate::aa`](aa.md) (`AAResults`, `AAInfo`).

## See also
LLVM `ScopedNoAliasAA.cpp`, `inlinemetadata` and the inliner's
`AddAliasScopeMetadata`, which make these lists.
