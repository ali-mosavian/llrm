> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/invariantsink.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/invariantsink.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/invariantsink.rs sha256:9797cda46696f42e3f445e0e9cfb744162c9745b1b440fb42cfb1790fa531f75 -->

# invariantsink

A value a loop's preheader computes and the loop never reads, moved to the
loop's exit: LLVM's `IndVarSimplify::sinkUnusedInvariants`.

## How it works
For a loop with one preheader and one exit block, the preheader is walked
backwards. An instruction that has no side effects, does not read memory
and has no user in the preheader or the loop (a phi's use counts at its
incoming block) moves before the first non-phi instruction of the exit. The
walk is backwards, so a chain of such values moves together.

A counter's increment that an inner loop's preheader holds is such a value:
only the outer latch reads it, and it moves there instead of living across
the inner loop.

```text
 outer:  %i.next = add %i, 1           outer:  br label %inner
         br label %inner         ==>   inner:  ...
 inner:  ...                           latch:  %i.next = add %i, 1
 latch:  %go = icmp %i.next, %n                %go = icmp %i.next, %n
```

## Example
From unit test `test_an_increment_only_the_latch_reads_moves_there`.

## Used by
Named by `indvars`.
