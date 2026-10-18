> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/hoist.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/hoist.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/hoist.rs sha256:1946fc4629135cc2d29881f9c9a5546957e668f4c8b0f76f324e40f07e4ac8b3 -->

# hoist

Moves work whose operands do not change in a loop, and loads nothing in it
writes, to before the loop, so it runs once and not every trip: LLVM's LICM
hoisting and gcc's `-fmove-loop-invariants`.

## How it works
Loops are taken inner first, so what leaves one may leave the next. A loop
is skipped unless exactly one block outside it enters its header (its
preheader). For each loop the pass grows an invariant run to a fixed point:
an instruction joins the run when every operand is defined outside the
loop or earlier in the run, and it is movable. Movable are pure arithmetic,
casts, compares, `gep`, `select`, `freeze`, aggregate extract/insert, and a
non-volatile load that nothing in the loop may write (`memoryssa::Accesses`;
a load from a `noalias readonly` parameter or marked invariant needs no such
proof). A call in the loop blocks only the loads it may write.

What may fault moves only where the loop certainly runs it, as the preheader
would otherwise run it on a path that did not. That is a load of what is not
known dereferenceable and that the target says may trap at its alignment
(`Machine::load_may_trap`), or a division by anything but a constant that is
neither 0 nor -1 (`_cannot_fault`). It is certain when it is in the header
behind an unconditional entry and before any call that cannot return
(`_guaranteed`), or when `ranges` shows its index inside the object on the
edge into the loop (`_bounded_inside`). Floating arithmetic is pure here.

Moving values out lengthens their live ranges. Where the target's registers
are priced and the function is in SSA, the moved function is priced against
the kept one (`profit::motion_price`: work by block frequency plus the spill
forecast). While it prices higher, the values crossing the loop that the
spill model spills, and what reads them, stay; a displacement the accesses
fold, or a value rebuilt at each read, is not charged. A floating value held
across the loop is charged its release. At -Os the run is priced in bytes,
every block once. The moved function's liveness is the kept one's with the
values the run moved solved again (`profit::spill_forecast_after`;
`LLRM_CHECK_VIEW` holds it to the forecast of the function on its own).
The LLVM order's slots do not price registers (`Hoist::priced` off): LLVM's
LICM has no such model, and a spilled invariant is left to rematerialisation
(x_life -Os ran 1.94x the instructions with the forecast keeping its address
math in the loop).

A hoisted load is priced by what it saves each trip. A floating load its one
reader takes from memory (x87 `fmul m32`) is not an instruction of the loop
(`OperationCosts::float_load_folded`, the fold's own extra cost), so
hoisting it saves that and costs a load and a release before the loop: it
stays unless the loop's trips pay for both. And the class holds only so many
floating values (`Machine::float_registers`): of those the run holds across
the loop, what the loop's own values leave room for are hoisted and the rest
stay. A 4x4 matrix read through a pointer in a loop of six trips was hoisted
whole, 13 of its 16 values spilled to the frame as doubles, 40 instructions
each entry for a loop gcc runs with the matrix in memory operands.

## Example
Before:

```mir
define i16 @f(i16 %n, i16 %a) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b2 ]
  %s = phi i16 [ 0, %b0 ], [ %s1, %b2 ]
  %c = icmp slt i16 %i, %n
  br i1 %c, label %b2, label %b3

b2:
  %k = mul i16 %a, 3
  %v = add i16 %k, 1
  %s1 = add i16 %s, %v
  %i1 = add i16 %i, 1
  br label %b1

b3:
  ret i16 %s
}
```

After:

```mir
b0:
  %k = mul i16 %a, 3
  %v = add i16 %k, 1
  br label %b1

b2:
  %s1 = add i16 %s, %v
  %i1 = add i16 %i, 1
  br label %b1
; ... b1 and b3 unchanged
```

`%k` and `%v` read only `%a`, which the loop does not define; they leave in
order into the preheader `b0`.

From unit test `test_invariant_arithmetic_leaves_its_loop_in_order` in
`hoist_tests.rs`.

## Runs
In `llvm_order`, first without speculation (only what the loop certainly
runs moves), then after `rotate`'s guard with it. Otherwise early, after
`floatloop` and before `loopmotion`; and again after `jumpthread`, at the
end of the loop passes, when a loop entered at its body runs it at least
once and what it loads unchanged may leave. On from
-O1; `-fno-move-loop-invariants` turns it and `loopmotion` off.

## Needs
`memoryssa::Accesses`, `Summaries`, [`crate::profit`](profit.md), `cfg::Shape`,
`noreturn::TerminalSites`, `ranges` and `consts` facts.

## Keeps
Blocks, edges, dominators and loops; instructions keep their ids.

## See also
LLVM `LICM.cpp`; gcc `loop-invariant.cc`. Stores sink in
[`crate::loopmotion`](loopmotion.md). Tests: `hoist_tests.rs`.
