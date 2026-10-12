> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/hoist.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/hoist.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/hoist.rs sha256:1ddb6339e4df32ea77bcf404e3ff6092eb0f7603c298fb7a4beacd0fa756d5fe -->

# hoist

Loop-invariant work done once, before the loop: LLVM's LICM hoisting and
gcc's `-fmove-loop-invariants`.

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

```text
 b0:                        b0:
   br b1                      %k = mul i16 %a, 3
 b1:                          %v = add i16 %k, 1
   %i = phi ...      =>       br b1
   %k = mul i16 %a, 3       b1:
   %v = add i16 %k, 1         %i = phi ...
   ...                        ...
```

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
