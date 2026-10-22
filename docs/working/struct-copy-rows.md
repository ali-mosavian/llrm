# Struct-copy rows R2 and R3 (#3621): cause

#3621 turned a C aggregate assignment into one `memcpy`. R2 (about 20 QCport modules, +1..+27 B) and R3 (`bitfield_long_long`, `int64_division`, `bitfield_overhang_init`) were priced above base. Hypothesis: the missing `SimplifyAnyMemTransfer` (a copy of at most 8 bytes is an integer load and store).

## Hypothesis tested (2026-10-23)

Translated as #3660 and measured on parity 1bd7d34afc (which holds #3621): QCport m16 changes one module (`sprite`, +5..+17 B), not about 20; objsweep changes one object per level. R2 is not the missing combine.

## Cause found for `mover` (R2's largest, +19/+27 B)

Our -O2 IR (`game/mover.c`) and the pin's `opt -O2` output of the same input are the same size of problem: 11 `memcpy` calls and 6 allocas on both sides. The mid-end matches; the 12-byte copies are above the combine's 8-byte limit in the pin too.

The difference is behind it. The pin's `llc -O2` on our IR gives 463 instructions for the module, ours 514 (+51, +11%). R2 is the backend's cost on this IR, a code-generation gap and not a struct-copy gap.

## Open

Per-function cause lines for the other R2 modules and for R3 (`int64_division`, `bitfield_overhang_init`) are not taken. `bitfield_long_long` m32 is an i64 `shl 0`/`or 0` left by the backend (#3660).

Owner w-4; per-function lines 2026-11-06.
