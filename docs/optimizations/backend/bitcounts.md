> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/bitcounts.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/bitcounts.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/bitcounts.rs sha256:4ba4312025242689dcc7b2faa098cb54ff943ea458c4a60d3c8f901dedf9a8de -->

# bitcounts

Expands `llvm.ctpop`, `llvm.ctlz` and `llvm.cttz` into shifts, masks and
adds in MIR before instruction selection, for the targets whose description
lists them under `expand`.

## How it works
A count is made at a power-of-two width `W` of at least 8; a narrower
integer is zero-extended to it first and the answer truncated back.

- `ctpop` is the parallel count: `x - ((x >> 1) & 0x55..)`, then pairs of
  two-bit sums by `0x33..`, nibble sums by `0x0f..`, then the byte sums
  folded by shifts (`x + (x >> 8)`, `x + (x >> 16)`, ...) and the low
  `log2(W) + 1` bits kept. No multiply.
- `ctlz` smears the highest set bit down (`x | x >> 1`, `| x >> 2`, `| x >>
  4`, ...) and counts the zeros of the result as `ctpop (~smeared)`; a zero
  gives `W`. A narrower type subtracts the bits it was extended by.
- `cttz` is `ctpop (~x & (x - 1))`; a zero gives `W`, and a narrower type
  sets the bit just above its width first so that a zero gives its width.

The second argument of `ctlz` and `cttz` (a zero is poison) is not read: a
defined answer for a zero refines poison.

```text
 %n = call i16 @llvm.ctpop.i16(i16 %x)      ; shifts, ands and subs and adds
 %z = call i16 @llvm.ctlz.i16(i16 %x, i1 0) ; smear, then ctpop of the not
 %t = call i16 @llvm.cttz.i16(i16 %x, i1 0) ; ctpop of (~x & (x - 1))
```

## Example
`ctpop` of an `i8`:

```mir
define i8 @f(i8 %x) {
  %n = call i8 @llvm.ctpop.i8(i8 %x)
  ret i8 %n
}
```

After, in the order the instructions are made:

```mir
define i8 @f(i8 %x) {
  %0 = lshr i8 %x, 1
  %1 = and i8 %0, 85
  %2 = sub i8 %x, %1
  %3 = and i8 %2, 51
  %4 = lshr i8 %2, 2
  %5 = and i8 %4, 51
  %6 = add i8 %3, %5
  %7 = lshr i8 %6, 4
  %8 = add i8 %6, %7
  %9 = and i8 %8, 15
  %10 = and i8 %9, 15
  ret i8 %10
}
```

From unit test `a_population_count_is_shifts_and_masks` in `bitcounts.rs`.

## Runs
Once per module in `driver::optimized`, before the odd widths are promoted,
for each of `ctpop`, `ctlz` and `cttz` the target description lists under
`expand` (m16 and m32 x86: no `popcnt`, and `bsr` and `bsf` are not selected
yet). A call of one the target does not list reaches selection, which
refuses it by name.

## See also
LLVM's `TargetLowering::expandCTPOP`, `expandCTLZ` and `expandCTTZ`
([TargetLowering.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp)),
which this follows with the multiply-free `ctpop`.
