> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/narrowcycle.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/narrowcycle.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/narrowcycle.rs sha256:a2a9deaadf144f2157c42fe3616da42e41dc856ee948d33849ff3513bd7d1b1e -->

# narrowcycle

A cycle of integer operations through a phi whose values only matter at a
narrower native width is carried at that width. Beyond the pin:
`TruncInstCombine` starts from a `trunc` and wants every value of its graph
unused outside it, and instcombine demotes a `trunc` of one operation; a
phi cycle read by `and` masks and compares of masked bits has no `trunc`
and has users outside any graph.

DEVIATE NARROW-CYCLE (owner m68k, exit 2026-11-20): the gap is named by
`TruncInstCombine.cpp`'s header (graphs "post-dominated by TruncInst" whose
instructions "must not have users outside the graph") and by
[`crate::narrowiv`](narrowiv.md), which narrows a counter and not a data cycle. Reason:
crc16's `crc` is read through `& 0xFFFF` and `& 0x8000` and carried in a
long; a word `add` or `eor` is half the 68000's clocks, and a word cycle is
the shape `add.w %d0, %d0; bcc` wants (m68k `cmpcarry`).

## How it works
For a legal width `w` (8, then 16), `narrowed` finds the set `S` of
`phi`, `and`, `or`, `xor`, `add`, `sub`, `shl` by a constant and `select`
instructions wider than `w` whose result is read in `w` bits or fewer: the
demanded width of `computeRecurrenceType` (`demanded_width`, over
`DemandedBits`, which is the fixpoint around the cycle), or a value known
zero above `w` bits, which a wide reader sees zero-extended.

A connected piece of `S` with a phi is made again at `w` bits when
`Tti::narrowing_gain` finds none of its operations dearer and the whole
cheaper on an axis and dearer on none, a doubling whose old top bit the
piece tests counting `shift_out_test_gain`, the `zext` at each outside use
that reads the wide value costing `extension_cost`.
Operands from outside are truncated (constants folded); uses outside read
a `zext` of the narrow value; the `and` masks become a narrow `and` and a
`zext`. The `nuw`/`nsw` flags are dropped: they were about the wide result.

```text
  %p = phi i32 [ %a, %e ], [ %r, %l ]
  %s = shl i32 %p, 1
  %r = select i1 %c, i32 %x, i32 %s

  %p = phi i16 [ %a.n, %e ], [ %r, %l ]
  %s = shl i16 %p, 1
  %r = select i1 %c, i16 %x.n, i16 %s
```

## See also
[`crate::narrowiv`](narrowiv.md), [`crate::trunc_inst_combine`](trunc_inst_combine.md).

## Example
A 32-bit phi cycle that is only read in 16 bits is carried as a 16-bit
cycle.

```text
%p = phi i32 [ %a, %e ], [ %r, %l ]       ; read only through & 0xFFFF
%s = shl i32 %p, 1
=>
%p = phi i16 [ %a.n, %e ], [ %r, %l ]
%s = shl i16 %p, 1
```

## Used by
Named by `indvars`.
