> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/slpcompares.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/slpcompares.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/slpcompares.rs sha256:3c1195d91af2cbf19e5964bc705d1dad51f9241378d0b66b023ffd95890a5cfa -->

# slpcompares

Compares of adjacent bytes against constants become one wide compare: the
part of LLVM's `SLPVectorizer` that matters here, where a vector of
compares is `reduce.and`ed, `VectorCombine` and `InstCombine` then make
it a compare of one integer.

## How it works
A conjunction (`and`, or `select c, x, false`, of one block) whose
conjuncts include `icmp eq (load p+k), c_k` for adjacent cells of one
pointer is `icmp eq (load p), C`, `C` the constants little-endian. The
leaves are the one-use compares of a one-use load; a leaf stands for the
whole conjunction only through `and`s, so a leaf under an `or` is left
alone. The widest chunk of 8 or 4 bytes is taken from the lowest cell,
the rest tried again. Nothing between the first and the last load of a
chunk may write memory, so the wide load can stand at the first. The first
leaf of a chunk is replaced by the wide compare and the others by `true`,
which the combine drops.

## Example
```text
  %a = icmp eq i8 %b0, 67                %w = load i32, ptr %p
  %b = icmp eq i8 %b1, 79                %r = icmp eq i32 %w, 1297044291
  %c = icmp eq i8 %b2, 77        ==>     (the chain is %r)
  %d = icmp eq i8 %b3, 83
```

MIRROR: the outcome of `SLPVectorizerPass` (`PassBuilderPipelines.cpp`
l.1455) on an `and` reduction of equalities of consecutive loads, with
`VectorCombine` and `InstCombine` after it. Chunks of fewer than four
bytes are not made: the pin's cost model keeps two and three byte chains
scalar (`two_bytes_stay_bytes` in `llrm-arm64`'s `slp_compares.rs`, measured
against the pin).

DEVIATE 1: no vectors. A chain the pin makes a `<16 x i8>` compare is
two 8-byte compares here. Exit: vector types in the backend.

DEVIATE 2: only equalities under `and`; `ne` under `or` is the same fact
the other way. Exit: the dual.

DEVIATE 3: the wide load's alignment is the narrow load's; a target is
asked only whether it has vector registers (`vector_register_bits`), not
whether a misaligned access is fast.

## Runs
At the pin's `slp-vectorizer` slot, after `simplifycfg` has made the
chain one block.

## Used by
`pipeline` (Late).
