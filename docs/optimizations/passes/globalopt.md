> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/globalopt.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/globalopt.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/globalopt.rs sha256:d82de11c7382672c77395adf501ad3bc79df3fea53342c7b99a797c594eade27 -->

# globalopt

Mark internal globals that are only ever loaded from as `constant`.

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

```text
@w = internal global i16 5       @w = internal constant i16 5
load i16, ptr @w          =>     load i16, ptr @w   (reads as 5)
```

## Runs
Once, as a module pass before the per-body pipeline and before the
interprocedural step. Not run at -O0.

## See also
LLVM's GlobalOpt, which also marks globals it finds never stored.
