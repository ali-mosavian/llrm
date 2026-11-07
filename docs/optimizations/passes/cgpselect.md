> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/cgpselect.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/cgpselect.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/cgpselect.rs sha256:b00e3a1b9e202b4052ebdbcb67459eacf1bfec9cd5869daa467c604023ec6ae5 -->

# cgpselect

A float-to-integer conversion, or a float compare, that only one arm of a
select with a constant arm needs, made on that arm's path only.

## How it works
`%r = fptosi (select c, K, x)`, where the select has no other user, is
`br c, join, other` with `%f = fptosi x` in `other` and `%r = phi [K', b],
[%f, other]` in `join`, `K'` the conversion of `K` made here. The path that
takes the constant converts nothing. Applied only where the conversion costs
more than twice a branch (`float_to_int` against `branch` in the target's
costs): on x87 `fistp` with the control word loaded around it is 43 clocks
against 3, so the half of the iterations that take `K` save about 21; where
one `cvttsd2si` converts, the select stays.

```text
 %s = select c, -3.0, %x            br c, %join, %other
 %r = fptosi %s                 other: %f = fptosi %x ; br %join
                                join:  %r = phi [-3, %b], [%f, %other]
```

The same for a compare: `select c1, K, (zext (fcmp ..))` is `br c1, join,
other` with the compare and its extension in `other` (`a < b ? -1 : a > b`:
the second `fcom; fnstsw; sahf` runs on the path that needs it, 9 clocks at
486, against the branch). Applied where `float_compare` is more than twice
a branch.

A clamp is a select of a select; the diamond made for the outer one holds
`fptosi (select ..)` again and is taken in the next round.

DEVIATE CS1 (exit 2026-11-20: the pin's
`SelectOptimize`/`optimizeSelectInst` priced by `getCastInstrCost`): the pin
keeps one conversion after the select (`FoldOpIntoSelect` is skipped when
the compare has the select's type) and its x86 has no price that makes a
conversion dear. gcc converts the constant at compile time (x_ldcmp -O2:
2020 `fistp` against 1180).

The condition is branched on as it is (no `freeze`): a poison condition
made a poison select, and now a branch on poison, which the pin's
`optimizeSelectInst` guards with `freeze`.

## Used by
Named by `cgpaddr`, `pipeline`.
