> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/globalbool.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/globalbool.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/globalbool.rs sha256:dca4bb204a01c4b1749b85f93bb91bab19c8198782d5d9c8abf3f3378b0a765e -->

# globalbool

Shrink an internal integer global that holds one of two values into an `i1`.

## How it works
`TryToShrinkGlobalToBoolean`
([GlobalOpt.cpp:1157](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/IPO/GlobalOpt.cpp#L1157)):
the only values ever stored into the variable are its initializer and one
constant. It becomes an `i1` starting false; a store writes `false` for the
initializer and `true` for the constant, and a load reads the `i1` and
selects between the two (a `zext` when the initializer is zero and the
constant one). A store of a value loaded from the variable copies the `i1`.

Every use must be a non-volatile load or store of the variable's own type
at its address, and some load must read it (a never-loaded variable is
`WriteOnly`'s).

DEVIATE 1: integer variables only; the pin also shrinks aggregates, whose
select MIR does not hold. Exit 2026-11-20.

DEVIATE 2: an `undef` initializer is left alone; the pin first turns the
stored constant into the initializer (`optimizeOnceStoredGlobal`'s
neighbour in `processInternalGlobal`), which is not translated.

## Example
Before:

```mir
@bad = internal global i32 0

define i32 @f(i32 %x) {
b0:
  store i32 1, ptr @bad
  %v = load i32, ptr @bad
  ret i32 %v
}
```

After:

```mir
@bad = internal global i1 false

define i32 @f(i32 %x) {
b0:
  store i1 true, ptr @bad
  %v.b = load i1, ptr @bad
  %v = zext i1 %v.b to i32
  ret i32 %v
}
```

## Runs
With `GlobalOpt`, once before the per-body pipeline.

## See also
[`crate::globalopt`](globalopt.md), [`crate::globalsra`](globalsra.md).
