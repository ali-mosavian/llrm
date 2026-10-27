> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/aggregate.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/aggregate.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/aggregate.rs sha256:eca72d9ee928a25201af62f0d3bdbd61598241ddb1f29c9089c4122dfa313fae -->

# aggregate

`AAResults::addAAResult` of `AliasAnalysis.cpp` at d1106deb71cc: several
alias analyses asked as one.

## How it works
An `AggregateAA` holds providers in the order they were added and is
itself an `AAResults`. A pair of locations is put to each in turn and the
first answer that is not `MayAlias` is the answer; none gives `MayAlias`.
What a call or instruction may do to memory is the meet of the providers'
answers (`ModRefInfo` and `MemoryEffects` intersect), and the walk stops at
the first answer that leaves nothing. A provider that knows little answers
the top of the lattice and costs the others nothing.

A new analysis (type-based, scoped no-alias, globals) is a provider added to
the aggregate; nothing in a client or in another provider changes.

DEVIATE G1: LLVM's `AAResults` also holds the `getModRefInfo(Instruction)`
dispatch by opcode and `callCapturesBefore`; here those are the trait's
methods, each provider answering them, and the aggregate meets the answers.
Exit: the dispatch moves into the aggregate when a second provider needs it.

## Example
Two providers, the first unsure and the second sure the pointers are apart:
the aggregate says `NoAlias`. Asked of `[no, unsure]` it says `NoAlias` as
well (the first answer that is not `MayAlias`); asked of no providers,
`MayAlias`.

```text
aggregate [unsure, apart]   alias(a, b) = NoAlias
aggregate [apart, must]     alias(a, b) = NoAlias   (the first decides)
aggregate []                alias(a, b) = MayAlias
```

From unit test `the_first_answer_that_is_not_may_alias_decides` in
`aggregate_tests.rs`.

## Provides
`AggregateAA`.

## See also
LLVM `AliasAnalysis.cpp`, [`crate::aa`](aa.md), [`crate::basicaa`](basicaa.md).
