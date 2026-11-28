> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/looptermfold.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/looptermfold.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/looptermfold.rs sha256:8c44b870e913e6bac71457d03c63438e32b527597ed7f2560cc7e9a648920141 -->

# looptermfold

`LoopTermFold`: the loop's last induction variable use, its exit compare, is
given to another induction variable (`LoopTermFold.cpp` at `d1106deb71cc`).

## How it works
For an innermost loop in simplify form with a known trip count whose latch
branches on `icmp (%i.next = op %i, step), invariant`, where the counter
`%i` is read only by its increment and that compare: another header phi
that is an affine add recurrence (no self wrap, non-zero step) gets the
test. Its value after the last trip is expanded once in the preheader, and
the latch tests `%j.next == that`, leaving `%i` and its increment dead.
The expansion must cost at most `min(2 * 4, the small constant maximum trip
count, the estimated trip count)` operations.

```text
 loop:  %i = phi [0, pre], [%i.next, loop]        loop:  %p = phi [%a, pre], [%p.next, loop]
        %p = phi [%a, pre], [%p.next, loop]  ==>         %p.next = getelementptr i8, ptr %p, 4
        %p.next = getelementptr i8, ptr %p, 4            %done = icmp eq ptr %p.next, %end
        %i.next = add nuw i32 %i, 1                      br i1 %done, label %exit, label %loop
        %c = icmp ne i32 %i.next, %n
        br i1 %c, label %loop, label %exit      pre:   %end = (value of %p after the last trip)
```

## Runs
In the strength slot right after `lsr`, for a target whose TTI enables it
(`llrm_mir::tti::Tti::enables_loop_term_fold`); the pin enables it for
RISCV, AVR and Mips.

DEVIATE LTF1: `getLoopEstimatedTripCount` reads the latch branch's weights
only; the pin's `llvm.loop.estimated_trip_count` attribute is not read, no
pass here writes it. Owner m68k. Exit: when a pass writes the attribute.

## See also
`canFoldTermCondOfLoop`, `RunTermFold`, `isAlmostDeadIV`.

## Example
The exit compare of a loop moves from a counter to another induction
variable, and the counter dies.

```text
 loop:  %i = phi [0, pre], [%i.next, loop]        loop:  %p = phi [%a, pre], [%p.next, loop]
        %p = phi [%a, pre], [%p.next, loop]  ==>         %p.next = getelementptr i8, ptr %p, 4
        %p.next = getelementptr i8, ptr %p, 4            %done = icmp eq ptr %p.next, %end
        %i.next = add nuw i32 %i, 1                      br i1 %done, label %exit, label %loop
        %c = icmp ne i32 %i.next, %n
        br i1 %c, label %loop, label %exit      pre:   %end = (value of %p after the last trip)
```
