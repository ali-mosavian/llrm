> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/globalopt.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/globalopt.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/globalopt.rs sha256:b519d9eba081be9b321f5b2f183a0118e8c39195cc08da11ae8d16a02cdf1992 -->

# globalopt

Mark internal globals that are only ever loaded from as `constant`, drop
the stores of those only ever stored to, and make a global one function
always writes before it reads that function's stack slot.

## How it works
A variable qualifies when it is internal or private, initialized, not yet
`constant`, and not named by any other global's initializer. Every use of it
in every function must then be a load's address, reached through constant
`gep`s and `bitcast`/`addrspacecast`s of its address, and through the same
instructions at run time as long as all their users load. Any other use (a
store, a call argument, a compare, an escape into a value) disqualifies it.

A qualifying variable gets `constant = true`. A load of it is then its
initializer's bytes (`memory::constant_bits`), which the body passes fold.
Every defined function is reported as changed so each body is run through
its pipeline again.

`localize` (LLVM's `processInternalGlobal`, last step) turns an internal
variable of one value that a single function uses, every load of which a
store of that function precedes, into the function's own stack slot; the
per-body passes promote it. The function must not recurse (no call but to
intrinsics, or `norecurse`).

`WriteOnly`, after the pipeline has forwarded what loads it could, is the
other half (LLVM's GlobalOpt: "if the global is never loaded (but may be
stored to), it is dead"; gcc's ipa `write-only` flag): a variable that is
internal or private, not named by another global's initializer, and whose
every use in every function is a non-volatile store's address (through the
same `gep`s and casts) loses those stores. Nothing names it then, and
`globaldce` deletes it.

## Example
Before:

```mir
@c = internal global [2 x i16] [i16 5, i16 12]
@w = internal global i16 7

define i16 @f(i16 %x) {
b0:
  %at = getelementptr i8, ptr @c, i16 2
  %a = load i16, ptr %at
  %b = load i16, ptr @w
  %s = add i16 %a, %b
  ret i16 %s
}

define void @g(i16 %x) {
b0:
  store i16 %x, ptr @w
  ret void
}
```

After:

```mir
@c = internal constant [2 x i16] [i16 5, i16 12]
@w = internal global i16 7
; ... @f and @g unchanged
```

`@c` is only loaded and becomes `constant`; `@w` is stored by `@g` and stays
a variable. The body passes then fold the load of `@c` to 12.

From the fixture of unit test `a_global_only_loaded_is_its_initializer` in
`globalopt_tests.rs`, run through `GlobalOpt` alone.

## Runs
Once, as a module pass before the per-body pipeline and before the
interprocedural step. Not run at -O0.

## See also
LLVM's GlobalOpt, which also marks globals it finds never stored.
