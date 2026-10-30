> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/exitne.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/exitne.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/exitne.rs sha256:0b91f3c99a1f534383501f690494e6ff6395f0099afc53e0a38780d9f0ea4fc6 -->

# exitne

A loop's strict exit compare of a unit-stride counter becomes `!=` where the
counter provably never passes the bound. Beyond the pin: no LLVM pass
rewrites `ugt`/`ult` to `ne` on its own (`canonicalizeExitCondition` and
`CorrelatedValuePropagation` only turn signed compares unsigned).

## How it works
For each loop whose latch ends in a conditional branch that stays in the
loop on one side and leaves it on the other, with a compare of `X` against
a loop-invariant `C` that continues the loop on `X >u C`, `X >s C`,
`X <u C` or `X <s C`:

- `X` is `{S,+,-1}` for `>` (`{S,+,1}` for `<`) in this loop.
- The loop entry is guarded by `S >= C` (`S <= C`), same signedness.

Then `X >= C` holds on every trip: it holds on the first, and the loop goes
round again only when `X > C`, so the step cannot cross `C`. The compare
equals `X != C` there, and exits exactly when `X == C`. Nothing is poison:
the compare is the same bit before and after.

## Example
`n` is positive; `%i = phi [n, entry], [%i.next, loop]`, `%i.next = add %i,
-1`, `br (icmp ugt %i, 1), loop, exit` becomes `icmp ne %i, 1`.

## Used by
[`crate::indvars`](indvars.md) runs `strengthened_in` after LFTR. The m68k mid-end
runs `strengthened` before loop strength reduction.

## See also
[`crate::lftr`](lftr.md), `docs/architecture/scev-port.md`.
