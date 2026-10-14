> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/hoist.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/hoist.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/hoist.rs sha256:34645f1636642177dc6ae8f51e4a07ca5c5255676551f150ab2ca97c5c14f6f5 -->

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
every block once.

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
Early, after `floatloop` and before `loopmotion`; and again after
`jumpthread`, at the end of the loop passes, when a loop entered at its
body runs it at least once and what it loads unchanged may leave. On from
-O1; `-fno-move-loop-invariants` turns it and `loopmotion` off.

## Needs
`memoryssa::Accesses`, `Summaries`, [`crate::profit`](profit.md), `cfg::Shape`,
`noreturn::TerminalSites`, `ranges` and `consts` facts.

## Keeps
Blocks, edges, dominators and loops; instructions keep their ids.

## See also
LLVM `LICM.cpp`; gcc `loop-invariant.cc`. Stores sink in
[`crate::loopmotion`](loopmotion.md). Tests: `hoist_tests.rs`.
