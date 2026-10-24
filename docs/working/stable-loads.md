# D-I: stable-load rematerialization is off by default (splitkit; 2026-10-25)

TL;DR: the spiller made a load of a cell nothing writes while the value lives again where it is read (DEVIATE D-I); the pin does it only for invariant loads. It is off by default (`-fllrm-stable-loads` turns it on); an incoming stack argument that nothing writes or takes the address of stays rematerializable, as in the pin. Exit 2026-11-13: say why the rest grows and translate it, or flip the default per target through `Profile::stable_loads`.

## Measured (D-I on -> off, before the argument rule)
- Ledger d_faces -O2 m32: 929.63 -> 916.11 M Ir (-1.45%); `_stable_loads_from` alone is 6.6 M.
- QCport (65 modules): bytes +1.41% (O1), +1.45% (O2), +1.17% (O3), +1.64% (Os); sizes.py m16 +589 to +921 B per level; objsweep 69 of 568 changed.
- Bench O2+Os, all languages: 236 counters worse, 19 better.

## Rows (AM-DI)
| row | program | metric | change with D-I off |
|---|---|---|---|
| AM-DI-1 | hanoi bas/bq16 -O2/-Os | memory operands, instructions | +15.8%, +6.1% |
| AM-DI-2 | queens bas/bq16 -O2/-Os | memory operands, code | +10.2%, +6.2% |
| AM-DI-3 | bintree bas/bq16 -O2 | memory operands | +7.4% |
| AM-DI-4 | nbody_single bq32/c -O2 | code | +11.6%, +6.2% |
| AM-DI-5 | sieve bq16 -O2 | code | +6.3% |

Better with it off: quicksort bas/bq16 instructions -5.8..-6.2%, sieve memory operands -7.6%, queens m32 clocks -7.2%, x_transpose -6%.

## Cause and the rule that stays
`hanoi` read its three stack parameters again from `[bp+6]`, `[bp+8]`, `[bp+10]` where it now stored them to slots and read the slots. X86 lowers a stack argument to an immutable fixed stack object and takes a load of one as invariant (`isLoadFromInvariantOrConstantMemory`), so that part is the pin's own rule: `remat::argument_loads` (a cell above the frame pointer that nothing writes and nothing takes the address of; one definition). With it, hanoi's output with D-I off equals the output with it on.

## What is still to find (exit 2026-11-13)
The numbers above were taken before the argument rule; the pair after it is in the PR. What remains is the loads only the per-range proof covers (array descriptors, m16 `es` loads). The pin has no counterpart: either the facts become an invariant-load flag on the instruction (X86InstrInfo's rule with the flag set from the proof), or the default flips per target with the evidence.
