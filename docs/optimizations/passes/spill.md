> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/spill.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/spill.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/spill.rs sha256:67aea8f944f380e796b987eadcf888dfc6c8382a7a76f82458022c65bd7865ca -->

# spill

The one spill model every pricing of register pressure shares: what values
past the target's registers cost.

## How it works
The model does not allocate. It asks, at each point of a function, how many
values are live against how many registers the target has there, and spills
the cheapest of the excess.

Residents. A value takes an integer register where it is live (`integer_in`
is true). Floating values do not. A truth value only its own block's `br`
reads is flags, not a register. An address is folded (`folded_in`), and
takes none, when only loads, stores and `getelementptr`s that are such
addresses read it, in its own block; a frame object, a symbol, or a
constant offset into one is a displacement wherever it is read, and a frame
object indexed by a scale the target's address takes (`Room::index_scales`)
is `[bp+si*8+disp]` at each read. What a folded address is made of (the
runtime pointer and each variable index) is read into registers at the
access instead (`transient`). A far pointer of offset zero is a segment
register's (`segment_view`) where the target has any.

Cells. A spilled value lives in a cell. A counter and its constant step
(`p + c`, `p - c` taken back into phi `p`) share one (`cells`), as the
allocator's copy-joined web does, and a phi's own edges into its cell move
nothing.

Points. Each instruction has a point before it, with `Room::registers`
less what its address needs (`transient`) and less the copy a two-address
target makes when an operand stays live (`copied`). A call has a second
point across it, with the registers it keeps (`kept_across`: the callee's
own count for a direct call). Two further kinds have their own registers:
segment views (`Room::segments`) and values an address is built from
(`Room::addresses`).

Price. A cell's traffic (`Traffic`), each count weighted by its block's
frequency, is a store after each definition (a far pointer takes one store
per word), a load at each read, and a memory update where a counter is
stepped in place. Where one instruction rebuilds the value from nothing (a
frame or symbol address, or a far view of one) the price is the less of
that and the traffic.

Forecast. The points are taken in order. At each, the residents not yet
spilled that number past its registers are spilled, cheapest first, a
dearer tie to the larger cell, and they stay spilled at every later point.
The forecast is the total cost, the spilled set, and `peak`, the most
residents past a point's registers before any spill. `Pressure` caches
liveness per function; `View::run` and `View::forecast` step the live set
through each block and keep the residents ranked by price, so a point
costs what changed at it, not what is live.

```text
  live before each instruction            registers = 3
  i1   {a b c}        fits
  i2   {a b c d}      excess 1: spill the cheapest of a b c d (say d)
  i3   {a b c d e}    d is spilled already; e makes excess 1 again:
                      spill the cheapest of a b c e
  cost = price(victim at i2) + price(victim at i3)
```

## Example
```mir
define i16 @f(i16 %n) {
entry:
  %a = alloca [32 x i8]
  %p = getelementptr inbounds i16, ptr %a, i16 3
  %q = getelementptr inbounds i16, ptr %a, i16 5
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %next, %latch ]
  %v = load i16, ptr %p
  %at = getelementptr i16, ptr %a, i16 %i
  %u = load i16, ptr %at
  %more = icmp ult i16 %i, %n
  br i1 %more, label %latch, label %exit
latch:
  %t = add i16 %v, %u
  %w = add i16 %t, %i
  store i16 %w, ptr %q
  %next = add i16 %i, 1
  br label %loop
exit:
  ret i16 %n
}
```
Facts:
```text
Room { registers: 4, across_call: 2 }  =>  peak 0 (nothing spills)
```
`%n`, `%i`, `%v` and `%u` fit the four registers; `%a`, `%p` and `%q` are
`[bp+disp]`, so they take none.

From unit test
`test_a_constant_offset_into_a_frame_object_takes_no_register` in
`spill_tests.rs`.

`View::fit`, which lsr asks of every set it weighs, keeps the residents of
the step before when a step wants the same, and diffs by sets when it does
not.

## Provides
`Room` (what the target holds), `Traffic` and `traffic`, the free
functions `spilled`, `spilled_in` and `forecast` over `Point`s, and the
`Pressure` analysis with its `View`.

## Used by
[`crate::profit`](profit.md) (`spill_forecast`, `weighted`) prices every motion with
it; [`crate::lsr_pass`](lsr_pass.md), [`crate::gvn`](gvn.md) (partial redundancy elimination),
[`crate::hoist`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/licm/hoist.rs) and [`crate::cgpaddr`](cgpaddr.md) ask it
directly when they weigh a rewrite against register pressure. Not a pass:
it runs only inside those.

## Needs
`liveness` (`llrm_analysis`), the target's `registers`, `call_registers`,
`far_access_registers`, `segment_registers`, `address_registers`,
`two_address`, `address_forms` and `OperationCosts` (`llrm_mir::target`).

## Keeps
`Pressure` is a manager analysis: each version of a function has it once.
A target that states no registers (`Room::priced` false) leaves pressure
unpriced.
