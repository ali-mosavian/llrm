> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/lsr_use.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/lsr_use.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/lsr_use.rs sha256:3616f9a13c39d0f0b016c08e6a3e18db75dc5dee451667839fb7cf44bf217824 -->

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

DEVIATE LU6: a stack slot's address that is the only base register of an
address formula takes no register where `Tti::frame_address_is_free` (x86:
`[bp+disp]`, `[esp+disp]`); the pin counts it. With the loop-carried budget cut
(LU3b), the slots of two frame arrays cost two registers and LSR walked two
pointers over one index (fpbench nib, +4 B). Exit: the pin's cost of a frame
index once its x86 TTI is checked against `isLegalAddressingMode`, by
2026-11-20.

DEVIATE LU8 (in `generate_end_biased`, with `unmanaged_address_pairs`): of the
two registers of a base-plus-index address one must be a base register, and the
target has `Tti::address_base_registers` of them (m16: bx). The end-biased form
pairs the exit counter with each array's base; where the loop's other accesses
pair two registers none of which is the counter (an index that is no
recurrence), and the base registers are fewer than the pairs, the form is not
made: each pair no base register holds one of is swapped in and out around its
access, two instructions a trip (grep nib -O2: +2.02M clocks). The pin has no
end-biased form and prices every register alike. Exit: none, an addressing-mode
fact of the target; a general register-class model replaces the greedy cover
when a second class needs it. (A cost term for the same pairs was tried and
benched worse: it moved loops the form never touched.)

DEVIATE: the target hooks have no access type or address space on the
legality call, and no per-instruction query (`LSRWithInstrQueries`); x86 has
neither. `getPreferredAddressingMode` is none on x86; the post-indexed
branch of `RateRegister` is ported (m68k prefers it), the pre-indexed one
is not, nor are the post-indexed sites in the other LSR modules. A
scalable `Immediate` does not exist (see [`crate::lsr_formula`](lsr_formula.md)).
