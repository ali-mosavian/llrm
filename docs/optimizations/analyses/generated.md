> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/generated.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/generated.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/generated.rs sha256:41214c8ecc4377af0ae7c2034051b4d3251a844fcdef8dbb2340ce0f739d50e9 -->

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

## Example
The case of seed 7: one invariant expression `%e0`, selected by `%which`:

```mir
define i32 @f(i16 %x, i16 %s, i16 %a, i16 %b, i16 %c, i16 %n, i16 %which) {
b0:
  %e0 = mul i16 30415, %c
  br label %b1

b1:
  %i = phi i16 [ %x, %b0 ], [ %inext, %b1 ]
  %t = phi i16 [ 0, %b0 ], [ %tnext, %b1 ]
  %z0 = zext i16 %e0 to i32
  %p0 = icmp eq i16 %which, 0
  %r0 = select i1 %p0, i32 %z0, i32 0
  %inext = add i16 %i, %s
  %tnext = add i16 %t, 1
  %go = icmp ult i16 %tnext, %n
  br i1 %go, label %b1, label %b2

b2:
  ret i32 %r0
}
```

From unit test `test_generated_recurrences_match_the_interpreter` in
`induction_tests.rs`, run with `SCEV_SEED=7` and `--nocapture`.

## Provides
`case`, `seeds`, `observed`, `Inputs`, `Rng`, `Tracked`, built only for
tests or the `testing` feature. Consumers: the tests of
[`crate::induction`](induction.md) and `expand` in `llrm-transforms`.
