# The 66h prefix, and what only hardware can answer

Every cycle figure this project has is a model. `qbopt/cycles` gives published
latencies; DOSBox charges per instruction and models no latency at all. Neither
can say what a `66h` operand-size prefix costs on a real 486 or Pentium, and on
those parts it may cancel the widening win outright.

Nothing here settles it. This is the protocol for when hardware appears, written
now so that it costs an afternoon rather than a project.

## What is at stake, and what is not

The pass makes two claims, and only one is at risk.

**Call absorption** removes a far call and the routine behind it -- eleven
instructions for a comparison. No plausible prefix cost touches that. It wins on
a 386 and on a Core.

**Pair widening** turns two 16-bit ALU instructions into one 66h-prefixed 32-bit
one. That is the part a 486 or P5 might not like.

They are separable on purpose: `--no-widen` keeps absorption and drops widening.
If the metal says the prefix is too expensive, that is a configuration change
rather than a rewrite.

## The experiment

Build `suite/metal.bas` plus a timing stub into a self-contained `METAL.EXE`
that needs no toolchain on the target machine. For each shape below, run the BC
form and the widened form back to back and print both.

| shape | BC | widened |
|---|---|---|
| load, and, store | `mov ax,[a] / mov dx,[a+2] / and ax,[b] / and dx,[b+2] / ...` | `mov eax,[a] / and eax,[b] / ...` |
| chain of four | as above, four operations | four widened operations |
| compare | `push / push / call B$CPI4` | `mov eax,[a] / cmp eax,[b]` |
| multiply | `push / push / call B$MUI4` | `mov eax,[a] / imul eax,[b]` |

Time with `RDTSC` where the part has it (Pentium and later) and the 8253
otherwise -- see `docs/measurement.md` for the PIT sequence. Warm the caches,
take the median of five, and report the spread.

## What to record

Per machine: the CPU as `CPUID` reports it, or the part number off the chip
where there is no `CPUID`; the clock; the memory timing if the BIOS shows it;
whether caches are enabled; DOS version; and the date. Without that a figure is
one anecdote rather than a measurement.

## Results

None. The table below is empty on purpose: it is the shape the answer goes in.

| CPU | clock | widening, load/and/store | widening, chain of four | absorption, compare | absorption, multiply |
|---|---|---|---|---|---|
| 386DX | | | | | |
| 486DX2 | | | | | |
| Pentium | | | | | |
| Pentium II | | | | | |
