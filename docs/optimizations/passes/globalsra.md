> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/globalsra.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/globalsra.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/globalsra.rs sha256:16c5adf1774a5d7d5dca26028385c3848852eb0142d9bfff5d5531dcdbaf88be -->

# globalsra

Split an internal variable into the pieces its loads read, then give a
piece one function writes before it reads to that function as a stack slot:
LLVM's `SRAGlobal` and the localizing branch of `processInternalGlobal`
(GlobalOpt.cpp, d1106deb71cc).

## How it works
A variable qualifies when it is internal, initialized, not `constant`, not
externally initialized and not named by another global's initializer, and
every use in every body is a non-volatile load's or store's address, found
through geps and casts with only constant indices. Each load is a piece
(offset and integer type); two loads of one offset with two types, or
overlapping loads, leave the variable alone, as `collectSRATypes` does. A
store of the piece's type is a store of the piece. A store of a constant
that covers whole pieces is one store of each (DEVIATE GSRA-1, below); a
store that covers no piece is dead. A piece that is loaded and stored
beyond 16 leaves the variable alone. Each piece is a new internal variable
`name.N` initialized from the original's bytes.

A variable with one accessing body that does not recurse, whose every load
a store of at least its size dominates
(`isPointerValueDeadOnEntryToFunction`, at most 100 load-store pairs), is
replaced by an `alloca` in that body's entry, stored with the initializer
first. The body passes then forward the slot. The pieces are made first, so
a piece localizes.

DEVIATE GSRA-1: the pin merges consecutive stores in the DAG at selection,
after its second GlobalOpt, so its SRA sees `store i16`; the `Merge` slot
runs in MIR before the walk ends, so this one sees `store i32` over two
`i16` loads. A constant store over several pieces is split into one store
per piece (`collectSRATypes` requires one type per offset). Exit: store
merging at its place in selection, or a GlobalOpt slot before `Merge`.
Owner w-8, 2026-11-05.

## Example
Before:

```mir
@c = internal global [4 x i16] zeroinitializer

define i16 @f() {
b0:
  store i32 65537, ptr @c
  %a = load i16, ptr @c
  %b = load i16, ptr getelementptr inbounds (i8, ptr @c, i32 2)
  %r = add i16 %a, %b
  ret i16 %r
}
```

After (`@c.0`, `@c.1` are the pieces, then slots):

```mir
define i16 @f() {
b0:
  %c.0 = alloca i16
  store i16 0, ptr %c.0
  %c.1 = alloca i16
  store i16 0, ptr %c.1
  store i16 1, ptr %c.0
  store i16 1, ptr %c.1
  %a = load i16, ptr %c.0
  %b = load i16, ptr %c.1
  ...
}
```

## Runs
A stage of the interprocedural step, `globalsra.`, after the bodies are
stamped (a body's `norecurse` is stated there); the bodies it changed go
back through the pipeline.

## See also
[`crate::globalopt`](globalopt.md), LLVM's `SRAGlobal`, `collectSRATypes`.
