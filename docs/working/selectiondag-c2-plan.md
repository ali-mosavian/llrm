# C2: m16 patterns for the DAG selector

Plan only; 2026-10-22, sdag-c. Pin d1106deb71cc. Piece C2 of [selectiondag-plan](selectiondag-plan.md) (due 2027-01-14): what real mode needs on top of the m32 table and address matchers (C0, C1).

## What the pin gives, and what it lacks

- The pin's x86 table is 32/64-bit. In 16-bit mode (`In16BitMode`, [X86InstrPredicates.td L212](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86InstrPredicates.td#L212)) it still matches `GR16` arithmetic and memory operands through the same `addr` complex pattern, so word `mov`/`add`/`cmp` patterns carry over once the address matcher answers with 16-bit components.
- `LES16rm` and `LDS16rm` ([X86InstrSystem.td L349, L363](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86InstrSystem.td#L349)) exist with an empty pattern list: nothing selects them.
- `REP_MOVS*`/`REP_STOS*` exist for `ESI/EDI/ECX` and `RSI/RDI/RCX` only ([X86InstrCompiler.td L389](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86InstrCompiler.td#L389)); no `SI/DI/CX` form, no segment override on the string source.

## How the missing patterns are written

Not by a second Rust reader. `tools/dagisel/m16.td` includes the pin's `X86.td` and adds records in the pin's own syntax; `regen.sh` runs the pin's tblgen on that file and writes a third table, `x86_m16.rs`, with the same decoder. A pattern the pin lacks is then a `Pat<>`, pruned and tested like any other, and every record carries its SD9 block number.

| Piece | Premise test (fail first) | Needs |
|---|---|---|
| C2a address mode (SD3) | a word `p->x` with `p` in a register is `mov ax, [bx+disp]`; `a[i]` of words is `[bx+si]` with a `shl` left as a register (no scale); a displacement over 16 bits stays a register | the 16-bit matcher in `x86_addr` (same `AddressMode`, a `Components16` policy: base from {BX, BP}, index from {SI, DI}, scale 1, disp wraps at 16 bits, `foldOffsetIntoAddress` L1926 branch) |
| C2b register classes (SD4) | a word used as an address component has the base or index class on its virtual register; AX and DX are never offered | [vreg-class-field-plan](vreg-class-field-plan.md) landing; the emitter (sdag-d) reads `OperandDesc.pointer_class` and picks the base/index class per mode |
| C2c far loads | `p->x` through a far pointer is one `les` and `[es:di+disp]`; a far store is `mov [es:di+disp], r` | SD2 expansion in the type legalizer (B0d): a `far` is two `i16` results, so `les` is a node `X86ISD::LES` with results `(i16, i16, chain)` the legalizer or lowering makes from the load; the pattern `(X86les addr:$src)` selects `LES16rm`; `LDS16rm` the same |
| C2d string and segment forms | `memcpy` of a constant word count is `rep movsw` on SI/DI/CX with `DS` or an override on the source; a `far` copy sets `DS` and `ES` | records for `REP_MOVSW_16`, `REP_STOSW_16` with `Defs`/`Uses` on SI/DI/CX and the segment operand; the DAG nodes come from the builder's `memcpy` expansion (sdag-a) |

Order: C2a first (the matcher; no table change), C2c next (one record), then C2b with the class field, then C2d. Each is its own PR with the fail-first test in its commit.

## Numbered DEVIATEs this plan takes

SD3 (address components) and SD4 (classes) are already registered as the plan's. New ones are added by the PR that first uses a number: the `m16.td` block (SD9), the 16-bit `rep` forms (record in `m16.td`, number then), and the choice that a scaled index never folds in real mode (reason: no scale in the encoding; exit: none).

## Risks

1. The pin's tblgen has a closed list of value types: the `far` type cannot be one, which is why `les` is a two-result node of `i16`s. If sdag-b's expansion shapes the node otherwise the pattern follows its shape.
2. Real mode has two segments live per access; the DAG has no segment operand on loads except through `X86ISD` nodes. C2c/C2d assume sdag-a gives memory nodes an address space the matcher turns into the `segment` operand (the pin's `X86AS` hook in `selectAddr`).
3. Pruning for m16 differs from m32 in the CPU features only (i386 as the floor); the table size and walk cost should match the m32 figures (priced in the plan, risk 1).
