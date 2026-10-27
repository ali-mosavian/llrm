> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/lsr_use.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/lsr_use.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/lsr_use.rs sha256:b845cb41d4a1544daf85184650ef214e04005f8a23d9d70f5e6a51526ac1aeec -->

# lsr_use

The uses and the price of LLVM's loop strength reduction: `LSRFixup`,
`LSRUse`, `Cost` and the addressing-mode checks they rate by
(LoopStrengthReduce.cpp at `d1106deb71cc`, l.1070-2130).

## How it works
An `LsrUse` is one thing the loop computes (an address, a compare against
zero, a plain value) with the `Formula`s that could compute it. `Cost` rates
a formula against the registers already chosen for the other uses: registers
first, then add recurrences, IV multiplies, base adds, scale, immediates and
the setup in the preheader. The target answers what an address mode folds
(`Tti`).

## Example
`p[i] = 0` in a loop is an address use with one fixup at offset 0; its
formula `{%p,+,4}` costs one register and one recurrence (1 instruction).

## Runs
Nothing runs it yet: stage 2 of the translation of LSR (the old pass in
[`crate::lsr_pass`](lsr_pass.md) is the pass that runs them).

## Limits
DEVIATE LU1: `Cost::rate_formula` adds the target's `scaled_index_insns`
(m16: the `lea` that forms the scaled register, and the prefixed form's
own cost) to `Insns` once for each scaled register (an `Address` formula
with scale above 1), beyond the pin. The pin has no real-mode model: its x86
prices a scaled address as part of the address form, and `ScaleCost` is the
last key of the comparison, so a scaled index always beat a pointer
induction variable (m16 listed `lea bx,[ecx+ecx]` for one array) or lost to
six of them (particle's six far arrays, which the narrowing then left only
the pointer forms of). 0 elsewhere, so the pin's rule. Owner
laa-runtime-checks-loadelim; exit: when the target description prices the
prefixed `lea` itself or the pin gains a real-mode model; reviewed
2026-11-30.

Known row of LU1: QCport `d_faces.c` -Os, `_clip_w` (six near float arrays
on one index, an x87 loop) is 6262 B against main's 6208. The model rates
the scaled solution at 10 instructions (9 registers, 2 recurrences) and the
pointer recurrences at 13 (9 registers, 9 recurrences), so it keeps the
scaled index, whose bases then reload from the frame; particle's `ADVANCE`
(six far arrays) wins by 3 more instructions at the same charge. Neither a
segment register class (the bases are near), an address-register budget
(both forms need the same) nor sparing each use's cheapest formula in the
narrowing (tried: the pointer form was kept and still lost) separates the
two; only a tuned charge (7 to 8) does, which is not derived. Owner
laa-runtime-checks-loadelim; exit: a fill cost for near pointer bases past
the four addressing registers; reviewed 2026-11-30.

Its -O2 cost is measured, not counted statically (static counts said
smaller): QCport's 61-frame BENCH run in the dosrun emulator, every object
main's but `d_faces.obj` (the DFACES-DYN harness: build.py, run.py).
Executed instructions: main 485,835,518; parity +2,383,382 (+0.49%); head
+2,816,018 (+0.58%), so this charge adds +432,636 (+0.09%). The parity loss
is row DFACES-DYN-1, not LU1's.

DEVIATE: the target hooks have no access type or address space on the
legality call, and no per-instruction query (`LSRWithInstrQueries`); x86 has
neither. `getPreferredAddressingMode` is none on x86, so the pre- and
post-indexed branches of `RateRegister` never apply and are not ported. A
scalable `Immediate` does not exist (see [`crate::lsr_formula`](lsr_formula.md)).
