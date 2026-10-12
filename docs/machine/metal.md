# The operand-size prefix, and what only hardware can answer

For maintainers pricing code for the 486 and P5. Every clock figure the compiler uses is a model: `timings.times` holds published latencies, and DOSBox charges per instruction with no latency at all. Neither says what a `66h` operand-size prefix costs on a real 486 or Pentium.

## What is at stake

The `prefix` row of `crates/target/llrm-x86-m16/src/timings.times` is the clocks spent decoding each operand-size prefix: 0 for the 386, 1 for the 486 and P5, 0 for P6 and later (which instead stall on `partial_stall` and `lcp_stall`). m16 emits 32-bit forms under `66h` throughout, and the multiply and divide selectors count the prefix ([arithmetic-targets](arithmetic-targets.md)), so the row decides whether a 32-bit form beats two 16-bit ones. The values are modelled, not measured. The table below is empty until someone measures it.

## Protocol

Build a self-contained DOS program that needs no toolchain on the test machine. For each shape, run the 16-bit form and the prefixed 32-bit form back to back and print both.

| shape | 16-bit | 32-bit under 66h |
|---|---|---|
| load, and, store a long | `mov ax,[a] / mov dx,[a+2] / and ax,[b] / and dx,[b+2] / ...` | `mov eax,[a] / and eax,[b] / ...` |
| chain of four | the above, four operations | four 32-bit operations |
| compare | two `cmp` | `mov eax,[a] / cmp eax,[b]` |
| multiply | helper call | `mov eax,[a] / imul eax,[b]` |

Time with `RDTSC` where the part has it (Pentium and later), the 8253 otherwise (the PIT sequence is in [measurement](../working/measurement/readme.md)). Warm the caches, take the median of five and report the spread.

## What to record

Per machine: the CPU as `CPUID` reports it, or the part number off the chip; the clock; the memory timing if the BIOS shows it; whether caches are enabled; DOS version; the date. Without that a figure is an anecdote.

## Results

| CPU | clock | long and, 16-bit | long and, 32-bit | chain of four, 16-bit | chain of four, 32-bit |
|---|---|---|---|---|---|
| 386DX | | | | | |
| 486DX2 | | | | | |
| Pentium | | | | | |
| Pentium II | | | | | |
