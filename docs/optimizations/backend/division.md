> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/division.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/division.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/division.rs sha256:62abe7a311d5e50a5f8481567200cda91033775ed3e5c100c3d5caabd70e01f4 -->

# division

Select a dword division by a constant as a multiply by its reciprocal,
where the CPU's prices make that cheaper.

## How it works
MIR keeps `sdiv`/`udiv`/`srem`/`urem` by a constant as divisions; this
module is asked by isel (`divide` in `isel.rs`) and answers with the
instruction sequence, or none to keep `idiv`/`div`. Only dword operands
qualify, and never when the profile is tuned for size (the magic number
is a 5-byte immediate; `cdq; idiv r` is 3 bytes).

`magic` (signed, after LLVM's SignedDivisionByConstantInfo) and
`unsigned_magic` (Hacker's Delight `magicu`) give the multiplier and
shift. The sequences, with `x` the dividend and `q`, `r` the results:

```text
  signed (divisor d, 1 < d < 2^31,       unsigned (3 <= d < 2^31, not 2^k)
  not 2^k: shifts, `power_of_two`)
    m = MAGIC                              m = MAGIC
    (lo,hi) = imul m, x                    (lo,hi) = mul m, x
    hi += x          if MAGIC < 0          t  = x - hi        } if the
    s  = hi >> 31 (logical)                t  = t >> 1        } multiplier
    hi = hi >> sh (arithmetic) if sh != 0  hi = t + hi        } is 33 bits
    q  = hi + s                            q  = hi >> sh (or sh-1)
    r  = x - q*d     (remainder wanted)    r  = x - q*d
```

`q*d` is built by [`arithmetic::scale`](arithmetic.md)'s shift/add/`lea` chain where one
exists, else by `imul q, d`. A negative divisor below -1 divides by its
magnitude and negates the quotient; the remainder is unchanged, since it
takes the dividend's sign. `-1` and `i32::MIN` stay divisions.

The profitability test prices both sides in clocks from the CPU profile.
The estimate sums the register copies, the multiply (`timing`'s
audited bounds, interpolated by the dividend's significant bits where the
CPU ends a multiply early), shifts, adds and the remainder's `q*d`. On a
target whose code is not 32-bit, each dword instruction adds one prefix
clock. The reciprocal is chosen when its estimate is below the divide's
minimum. A CPU with no audited bound for the multiply or divide keeps the
division.

The multiplier is placed in the accumulator and the dividend where the
486's early-out reads the operand's length, so the multiply is priced by
the dividend's bits and not the magic's 31.

## Example
`Tenth& = x \ 10` on a LONG: the divide is a multiply by `0x66666667`, a
shift by 2 and the sign. `x` is the loaded dword `v2`. `; inserted` notes
are dropped.

```lir
v2 := mov [None v1@unplaced]
v5 := mov 0x66666667
v6, v7 := imul v5, v2
v8 := shr v7, 0x1f
v9 := sar v7, 0x2
v4 := add v9, v8
```

From `ISEL_DUMP=1 llrm-qb -O2 -march=i386` of function `TENTH&`, stage
`FarIndirectCalls`.

## Used by
`isel.rs` (`divide`), for each division by a constant.

## Needs
[`crate::backend::arithmetic`](arithmetic.md) for the multiply chains and
[`crate::backend::timing`](timing.md) for the clocks.

## See also
GCC `expand_divmod`; `docs/machine/arithmetic-targets.md`
