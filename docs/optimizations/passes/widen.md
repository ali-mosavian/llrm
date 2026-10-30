> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/widen.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/widen.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/widen.rs sha256:acd5d5e488c2accc6c342d76f4fdda7a50a5caa4b90fcd9e54e5f7c8bdee7c5d -->

# widen

A loop counter read through `sext` or `zext` becomes a counter of the wider
type, and the extensions go. LLVM's `WidenIV` (`SimplifyIndVar.cpp`).

## How it works
`WidenIv::create_wide_iv` takes a header phi `i` that ScalarEvolution models
as an add recurrence and that some `sext`/`zext` in the loop widens to a
legal type. It expands the extended recurrence (`sext {0,+,1}` is
`{0,+,1}` of the wide type when the narrow counter does not wrap) as a new
header phi, then walks the users of `i` with a worklist:
- a user that is itself an extension is replaced by the wide value;
- a user whose widened form is an add recurrence of the loop (an add, sub or
  mul of the counter and an operand whose extension keeps it a recurrence)
  is cloned in the wide type and followed;
- a compare is rewritten on the wide counter (`widenLoopCompare`);
- an add, sub or mul that only extended users read is widened in place
  (`widenWithVariantUse`);
- any other user reads a `trunc` of the wide counter, which isolates the
  narrow phi so `dead` removes it.

A narrow definition known not to be negative may use either extension.
Where an increment `x + c` (nsw, c >= 0) is guarded by a compare of `x`
inside the loop, the range that compare leaves the increment is kept per
use (`calculatePostIncRanges`), so a use below the guard proves
non-negative.

## Example
`for (short i = 0; i < n; i++) a[i]` reads `sext i` to the index width on
each trip. Widened, the loop counts `i` as a wide recurrence, the `sext`
is replaced by it, and the compare is on the wide counter.

## Used by
[`crate::simplifyindvar`](simplifyindvar.md) (`simplifyAndExtend`), from [`crate::indvars`](indvars.md).

## See also
LLVM `WidenIV`, `createWideIV`, `visitIVCast`;
`docs/architecture/scev-port.md`.
