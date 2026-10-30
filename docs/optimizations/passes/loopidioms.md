> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopidioms.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopidioms.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopidioms.rs sha256:eabb5369d823934f5353d151d37defc7b56e6691dae96025f9508d3475651d09 -->

# loopidioms

Loop idiom recognition and store merging: a counted loop that fills or
copies memory becomes one `llvm.memset`, `llvm.memcpy` or `llvm.memmove`
(LLVM's LoopIdiomRecognize; gcc's loop distribution patterns), and
straight-line adjacent stores become one memset or wider stores (LLVM's
MemCpyOpt `tryMergingIntoMemset`, MergeConsecutiveStores; gcc's store
merging).

## How it works
`LoopIdioms` rewrites one loop at a time, then looks again, with two shapes
of loop: tested before its trips, or tested after them (a loop entered
behind a copy of its test, `-ftree-ch`). The loop has one latch, a
straight-line body, and a trip count `induction::counted` proves; what it
does besides arithmetic is stores, loads and memsets that follow one of
these forms:

- A fill: a store, or a memset, of a loop-invariant value that is one byte
  repeated, through an address `induction` derives as a GEP off an invariant
  pointer, striding by the element's size. A cell whose bytes do not repeat
  is `llvm.experimental.memset.pattern` (DEVIATE: the pin forms none off
  Darwin,
  [L1214](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L1214);
  here it is priced from the target's table, so a word fill is `rep stosw`
  where its clocks beat the loop's at speed, and its bytes at `-Os`; m16
  `fillw` of 100 words is 407 clocks against the loop's 500, break-even
  near 16 trips with the segment setup; owner w-5). A loop with several
  fills to bytes no other reaches is a memset each; fills that tile adjacent
  bytes with the same byte are one.
- A copy: a load and a store of one cell each trip, the stored value being
  the loaded one, from one pointer to another with the same step. Where the
  ranges are apart it is `llvm.memcpy`; where they overlap but each trip
  reads what the earlier trips left untouched (writes trail the reads going
  up, or going down) it is `llvm.memmove`, tagged with its direction. Any
  other overlap is a smear and stays.

The count in bytes must not wrap the index: a byte's trips never do, and
wider cells need `inbounds` GEPs or the proof's `maximum`. The counter must
not be wider than the index. A huge pointer, which carries into its
selector, is taken like any other: the intrinsic carries its address space,
and instruction selection splits the fill at each 64K segment
(`isel/huge.rs`; Owner: clib-huge-fill-2256). A volatile access, a call, or
a value the loop changes keeps the loop.

A loop with one fill of constant bytes is rewritten always. A pattern, a
copy or a loop of several fills is priced (`_pays`, `_cheaper_by`) against
what the loop costs for its trips (the proven count, or the profit model's
estimate): the target's string-instruction setup and per-cell cost against
the loop's operations, and a few cells are left as stores or loads and
stores. At -Os the comparison is in code bytes of the loop against the
call's.

```text
  header: %i = phi [0,pre],[%n,body]      pre:   %cnt = trips (or n)
          br (%i < n), body, exit                call memset(&a[0], 0, %cnt)
  body:   store i8 0, &a[%i]       ==>     header: (one trip kept)
          %n = add %i, 1                   exit:   phis read the counters'
          br header                                exit values
```

The loop is kept as one trip (the latch goes to the exit, the header's phis
are replaced by their starts), guarded by the header's test unless the count
is proven positive, and its counters leave with their exit values computed
in the preheader (`counting::Seeds`). Later passes delete what is left. The
memset is declared in the module where it has none, through the pass
manager's `Declared`.

`Merge` is a separate pass over straight-line code in each block. A run of
at least two non-volatile stores of one repeated byte to adjacent bytes of
one object (no other memory access between them) is one memset placed where
the last stood, if profitable (LLVM's `isProfitableToUseMemset`: four stores
or 16 bytes, or fewer only where the memset needs fewer widest-integer
stores). The other stores, of constants and of loaded values, to adjacent
cells are wider ones (`mergestores`). `Merge` runs
after the scalar passes have settled: no pass asks a memset what a cell
holds, so merging sooner would lose a store's value to the forwarding after
it.

## Example
Before:

```mir
b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %c = icmp slt i16 %i, 10
  br i1 %c, label %b2, label %b3

b2:
  %p = getelementptr inbounds [64 x i16], ptr @buf, i16 0, i16 %i
  store i16 0, ptr %p
  %next = add i16 %i, 1
  br label %b1

b3:
  %e = phi i16 [ %i, %b1 ]
  ; ...
```

After:

```mir
b1:
  %c = icmp slt i16 0, 10
  br label %b2

b2:
  %p = getelementptr inbounds [64 x i16], ptr @buf, i16 0, i16 0
  %0 = add i16 0, 10
  call void @llvm.memset.p0.i16(ptr %p, i8 0, i16 20, i1 false)
  %next = add i16 0, 1
  br label %b3

b3:
  %e = phi i16 [ %0, %b2 ]
  ; ...
```

Ten word stores are one memset of 20 bytes; the loop stays as one trip and
the counter leaves with its exit value 10.

From unit test `a_counted_word_loop_is_one_memset_of_its_bytes` in
`loopidioms_tests.rs` (first case; the printed text was taken from a run).

## Runs
Last in the function fixed point: `LoopIdioms`, then `Merge`, after the
unroller. On with `-ftree-loop-distribute-patterns`; off at -O0 and -Og,
on from -O1. `size` is set at -Os and -Oz.

## Needs
`induction` (`counted`, `trips`, `recurrences`, `derived` addresses),
`consts` facts through `Standing`, aliasing (`MemRef`) and the target's
costs (`profit`, `OperationCosts`).

## Keeps
Blocks and edges change (a loop becomes one trip): every analysis is
invalidated when anything was rewritten.

## See also
[`crate::counting`](counting.md) builds the count and exit values;
[`crate::loopunrollpass`](loopunrollpass.md), [`crate::looppeel`](looppeel.md) precede it.
