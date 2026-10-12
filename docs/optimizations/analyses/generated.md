> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/generated.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/generated.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/generated.rs sha256:d59fa2065b1cfb77d470cc162cd51a149ef68078b422c09e41d5719c1355192c -->

# generated

Random loops for checking the recurrence form (`induction::Scev`) against
the interpreter.

## How it works
A case is one counted loop over 16-bit values, built from a seed:

```text
 b0:  invariants (expressions of the arguments a b c s x and constants)
 b1:  %i = phi [start, b0], [%i + step, b1]      start: %x or a constant
      %t = phi [0, b0], [%t + 1, b1]             step:  %s or a constant
      e0 .. en  random expressions of %i and the invariants
      select chain on %which: returns ej
      br (%t + 1 <u %n), b1, b2
```

Each expression is built up to three deep from add, sub, mul, shl by a
constant, `sext`, `zext` and `trunc`, with 16- and 32-bit widths. A
`Tracked` value records its name, width and degree in the counter (0
invariant, 1 affine, more not a recurrence) and whether an extension of a
varying value is on the way (affine only where ranges say it does not
wrap). `Tracked::affine` says a recurrence form should exist for it.

Evaluating the loop with `which = j` returns `ej` on the last trip, so
`observed` runs the module in the interpreter and any value's form can be
compared with what the program computes, on inputs from `Inputs::random`.
`Rng` is xorshift64* seeded per case, so a case is the same everywhere.

Seeds are replayable: `SCEV_SEED=k` runs seed `k` alone and prints its
text; `SCEV_SEEDS=n` runs `n` seeds instead of the default.

## Provides
`case`, `seeds`, `observed`, `Inputs`, `Rng`, `Tracked`, built only for
tests or the `testing` feature. Consumers: the tests of
[`crate::induction`](induction.md) and `expand` in `llrm-transforms`.
