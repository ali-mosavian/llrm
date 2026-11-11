# M68K-KNOWN-WRONG-ALIGN: an over-aligned local is silently misaligned

Owner: m68k. Exit: a named refusal lands with the PR that follows #8457 (2026-11-11), then stack realign 2026-11-14: m16's realign is shared code (`over_aligned`/`realign_of`, `Op::AndSp`, due 11-13; design `~/scratch/handoff/m16-realign-design.md`); m68k supplies its own `and.l #-A,%sp`, locals addressed from `%sp` and parameters from `%a6`.

TL;DR: today an `alloca` aligned past what the frame keeps (4) is neither realigned nor refused, and the optimizer still folds `ptr & (align - 1)` to 0 from the IR alignment. Wrong code, no message.

## Executed evidence (qemu-m68k, 2026-11-09, build = release/next be6d8f5bd2 + #8453)
- `tests/data_rt/aligned16_local.c` (`long m[8] __attribute__((aligned(16)))`, address escapes) called by `aligned16_main.c` at 16 stack depths: off a 16-byte boundary at 12 of 16 depths, 68000 and 68020, O1 and O2.
- `alloca [64 x i8], align 64`: 15 of 16 depths misaligned, both CPUs.
- `alloca double, align 8`, `ptr & 7` returned as the result: folded to 0 (`moveq #0,%d0`), the real value non-zero at 8 of 16 depths.
- The corpus program `over_aligned_local_after_even_pushes` read "ok" at 6 of 6 cells and proved nothing: its `& 15` is folded away (no `#15` in the O2 assembly).

## Why
The frame rounds to 4 (M68K-STACK-ALIGN), the datalayout says `S16` (2 B), and nothing realigns. The pin's subtarget keeps 8 and realigns with a frame pointer.

## Fix (general, shared target fact; landed in the PR that sets the two values)
`kept_stack_align` 4 and `max_alloca_align` 8 on the shared target fact (#8457): at pipeline entry an alloca aligned at most 4 is unchanged, 4 < A <= 8 is clamped to 4, A > 8 is refused by name. The refusal makes `over_aligned_local_after_even_pushes` a visible CCFAIL cell (no `!m68k` tag). Then stack realign (prologue `sp, -A` with a frame pointer, via `kept_stack_align`) turns the refusal into correct code.
The harness is the fail-first for the refusal: it must exit non-zero today and the compile must refuse or the exit be 0.
