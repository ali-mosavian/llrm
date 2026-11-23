> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/trial.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/trial.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/trial.rs sha256:b398a897a1818efa234d578dbc21444ccfcf36175c5c11b14be4d5443aa4e4e6 -->

# trial

The state a target's driver and a pass share for a trial compile: the module
is compiled again with one decision of the pass reversed, and the driver
keeps whichever result it prices smaller or cheaper. Two trials use it,
UNROLL-TRIAL and ROTATION-TRIAL.

DEVIATE UNROLL-TRIAL (beyond the pin; owner 6502, exit 2026-12-05): the
state the target's driver and the pass share for a trial unroll.

The pin decides a full unroll by a threshold and by what the loop's own
body folds (`analyzeLoopUnrollCost`, at most ten trips). A loop whose
copies fold the loads of a later loop (an array filled by one loop and read
by the next) is seen by neither: the pin unrolls it only because its
threshold is large in its own units. Here the pass notes that it refused a
loop the target's trial size admits; the driver then compiles the program
again with every loop under that size unrolled and keeps whichever
assembly it prices cheaper.

## Example
A loop the pass refused to unroll is noted; the driver compiles again with
it unrolled and keeps the cheaper assembly.

```text
pass:   loop refused by the threshold -> noted (ELIGIBLE)
driver: compile 1 (loop kept) vs compile 2 (loop unrolled)
        -> keeps whichever assembly it prices cheaper
```

## Used by
Named by `assemble`, `inlinecost`, `jumpthreading`, `llc`, `looprotate`, `loopunrollpass`, `lsr_pass`, `price`, `process`, `simpleloopunswitch`, `switchlookup`, `target`, `tti`, `walk`.
