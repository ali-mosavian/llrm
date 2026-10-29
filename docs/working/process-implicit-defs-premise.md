# ProcessImplicitDefs: premise

Pin d1106deb71cc, [`ProcessImplicitDefs.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/ProcessImplicitDefs.cpp). Slot-table row 59. Premise first: what the pass does in the pin, and whether ours differs in effect.

## What it does in the pin (measured)

`llc -mtriple=i386-pc-linux-code16 -frame-pointer=all -O2 -print-before=processimpdefs -print-after=processimpdefs` on the `addresssink` dump of each QCport module built at -Os -fno-inline-functions (`tools/copywhy.py` method; 56 of the 65 modules ran, 9 did not):

- the MIR before the pass has `IMPLICIT_DEF` in 47 of 56 modules (1424 occurrences);
- the pass changes the MIR in the same 47: `%15 = IMPLICIT_DEF; %14 = INSERT_SUBREG %15(tied-def 0), %0:gr16, sub_16bit` becomes `%14 = INSERT_SUBREG undef %15(tied-def 0), %0:gr16, sub_16bit`. It is the widening of a 16-bit value into a 32-bit register: the upper half is free. The row's "62 of 77 IRs" in codegen-ablation.md is the same effect, counted on the next pass that prints.

## Ours

LIR has `implicit_def` (isel) and `Insn::undefs`; a phi reads an undefined value as an `implicit_def` in PHIElimination (P2), and no code is made for it. A 16-bit value that a 32-bit address form reads is widened explicitly: `promote` in `addressforms.rs` makes its definition a `movzx`, because the 67h forms read whole dwords and need the upper half zero.

## Whole effect, one instrument

Extends in the final code of the same 56 modules: the pin's asm has 2147 lines starting `movz` (AT&T `movzwl`, `movzbl`), ours 437 `movzx` (listing.asm of `llrm-cc -Os -fno-inline-functions`, md5 45b29c601799: feat/parity 28191940c1 plus the copywhy probe). Ours has fewer, not more; the two counts are not the same measure (the pin's include byte extends and its own address forms), so the claim is only that no gap shows on this axis. Not measured: bytes attributable to the widening, and the same count at m32.

## Decision

No code. The row is DEVIATE (numbered, no pin function to translate): LIR has no undef lanes; the widening is explicit and needed for the 67h forms. Exit: MIR undef reaching LIR (then `INSERT_SUBREG undef` has an analogue and the pass can be translated); owner mir-undef; review 2026-11-14.
