> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/floatunhoist.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/floatunhoist.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/floatunhoist.rs sha256:59aac9c274b246ddcca347b4b598f39360e2c8b2533c52397d155a9aeba790de -->

# floatunhoist

A hoisted float load that a loop only reads is made again where it is read.

## How it works
LICM hoists `v = fld [cell]` out of a loop whose cell nothing in the loop
writes. On the x87 stack `v` then lies in a slot for every trip, and its
price is one `fld` and one `fstp` per entry of the loop. A loop that runs
two trips on average pays that for two uses that a memory operand reads at
the same price: `fld [x_j]; fsub st, st(2)` against `fld [x_j]; fsub [xi]`.

A value is made again in the loop when

- the registers its address names are read in the loop already (no live
  range grows: m16 has four address registers);
- one `fld` of a plain cell defines it, in the loop's only outside
  predecessor, with nothing after it there or in the loop that may write the
  cell, call, or change what the address reads;
- every read of it is in the loop, at most two, each the right operand of
  `fadd`/`fsub`/`fmul`/`fdiv` (the shape `FloatFold` folds to a memory
  operand), the register operand of such an operation that already has its
  memory operand (each read copies the value: `fld st(i)`), or a copy of it;
- no read costs more from the cell than from the stack, per the target's
  `x87_*` rows (a copy: `x87_load` against `x87_copy`; an operation:
  `x87_*_m` against `x87_*`).

Two reads at most keep the bytes: the memory forms add at most as many as
the hoisted `fld` and the `fstp` that ends it take.

DEVIATE M16-NBODY-O2: the pin keeps the value on the stack. Reason: the
entry's `fld` and `fstp` cost 6 clocks on the 486 per entry of the inner
loop (m16 nbody -O2: 600 entries of 2 trips). Owner llrm-target-x87, exit
2026-11-20.

## Example
```text
  pre:  fld [xi]               pre:
  loop: fld [xj]               loop: fld [xj]
        fsub st, st(2)    =>         fsubr [xi]
  exit: fstp st(1)             exit:
```

## Runs
After phi elimination, before `FloatFold` (which folds the new loads).
