> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopversion.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopversion.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopversion.rs sha256:108bf4e2344f6390195d4b72e0cf517ce79159b779738eedca6eb79e954b9cb7 -->

# loopversion

Loop versioning: a loop is copied for what is true of it only at run time,
and the copy is entered behind a test of that.

## How it works
A candidate is an innermost loop in canonical form
([`llrm_analysis::cfg::Canonical`](../analyses/cfg.md)) that has something to assume, each a
reason that implements `Assumption`: the test it makes in the loop's entry
and what it tells the copy's code.

- [`crate::versionstride`](versionstride.md): an index steps by an unknown stride `s` (`a[i *
  s]`): the test is `s == 1`, and the copy reads 1 for `s` (gcc's
  `-fversion-loops-for-strides`).
- [`crate::versionalias`](versionalias.md): pointers the loop reads and writes may be one
  object: the test is that their bytes do not meet over the loop's trips,
  and the copy's pointers are restrict (LLVM's `LoopVersioning` with
  `RuntimePointerChecking`).

The loop is copied ([`crate::loopclone`](loopclone.md)'s `versioned`): the entry makes the
tests, goes to the copy where they all hold (a branch weighed `LIKELY` to
1) or to the loop as it was, and both leave to the loop's exit, whose phis
take the copy's values.

```text
  for i: a[i * s] = 0              if s == 1: for i: a[i] = 0      <- a fill
                                   else:      for i: a[i * s] = 0
```

The copy is a candidate, as a candidate is:
[`crate::candidate`](candidate.md) runs the pipeline over it, with the loop's header
copied and what it loads unchanged hoisted
([`crate::pipeline::header_copied`](pipeline.md)) and the same done to the loop it is
judged against, so what the assumption allows is done by then (a fill, one
counter, a load read once). It is kept only when its price, spills included,
is 5 percent smaller (`MIN_PERCENT`): the loop twice and the test are code
for what they save. At most `MAX_TRIALS` loop is tried per body, of at most
`MAX_LOOP_OPS` operations in a body of at most `MAX_BODY_OPS`.

`LLRM_DEBUG=version` prints each loop versioned and why each other was not.

## Example
Before:
```mir
b2:
  %o = mul i16 %i, %s
  %p = getelementptr i16, ptr %a, i16 %o
  store i16 0, ptr %p
  %i.next = add i16 %i, 1
  br label %b1
```
After, in the entry `%one = icmp eq i16 %s, 1` and `br i1 %one, label %copy,
label %b1`, and in the copy:
```mir
3:
  %5 = mul i16 %4, 1
  %6 = getelementptr i16, ptr %a, i16 %5
  store i16 0, ptr %6
```
which the pipeline makes a fill of `%n` words.

From unit test `a_loop_that_steps_by_a_stride_is_copied_for_one` in
`loopversion_tests.rs`.

## Runs
Once per body, after `unswitch` and
at -O3 and -Omax (`-fversion-loops`, both
reasons: gcc's flag is the stride's, LLVM's has none). Not run where one
pass is selected by name. The module declares `llrm.noalias.p0` first
(`DeclareRestrict`), so that every pass reads a copy's restrict pointers;
`dead::markers_dropped` makes them their operands before selection.

## Needs
[`crate::candidate`](candidate.md), [`crate::loopclone`](loopclone.md) (`versioned`), [`crate::profit`](profit.md),
`induction`, [`llrm_analysis::access`](../analyses/access.md) (the reasons').

## Keeps
Nothing when it versions: the function is replaced by the candidate.

## See also
gcc `gimple-loop-versioning.cc` (`class loop_versioning`); LLVM
`LoopVersioning.cpp` (`LoopVersioning::versionLoop`).
