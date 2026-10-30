# D-I: stable-load rematerialization is off by default (splitkit; 2026-10-25)

TL;DR: the spiller made a load of a cell nothing writes while the value lives again where it is read (DEVIATE D-I); the pin does it only for invariant loads. It is off by default (`-fllrm-stable-loads` turns it on); an incoming stack argument that nothing writes or takes the address of stays rematerializable, as in the pin. Exit 2026-11-13: say why the rest grows and translate it, or flip the default per target through `Profile::stable_loads`.

## Measured (parity -> this PR, with the argument rule; pair di2)
- Ledger d_faces -O2 m32: 922.53 -> 916.09 M Ir (-0.70%).
- QCport (65 modules): bytes +0.39% (O1), +0.38% (O2), +0.33% (O3), +0.42% (Os), 30-31 modules changed (d_alias +4%, d_sprite +1.5%: the x87 `fld` loads of globals, D-RFP4). Before the argument rule it was +1.2..+1.6%.
- objsweep: 6 of 568 changed; sizes.py m16 +9/+372/+353/+28 B (O1/O2/O3/Os).
- Rows: nbody_single -O2 code +12.6%, instructions +1.9%, clocks +1.2%; nbody_fixed -O2 code +1.5%, instructions +0.5%; nothing else moved. tests/run: no new failure.
- Earlier, before the argument rule (D-I off entirely): QCport +1.2..+1.6%, 236 bench counters worse (hanoi, queens, bintree, sieve, nbody_single).

## Rows (AM-DI)
| row | program | metric | change with D-I off |
|---|---|---|---|
| AM-DI-1 | nbody_single bq32/c -O2 | code, instructions | +12.6%, +1.9% |
| AM-DI-2 | nbody_fixed -O2 | code | +1.5% |
| AM-DI-3 | QCport d_alias | bytes | +4% (-O2) |
| AM-DI-4 | QCport d_sprite | bytes | +1.5% (-O2) |

AM-DI-1..5 of the first version (hanoi, queens, bintree, sieve) are closed by the argument rule below.

## Cause and the rule that stays
`hanoi` read its three stack parameters again from `[bp+6]`, `[bp+8]`, `[bp+10]` where it now stored them to slots and read the slots. X86 lowers a stack argument to an immutable fixed stack object and takes a load of one as invariant (`isLoadFromInvariantOrConstantMemory`), so that part is the pin's own rule: `remat::argument_loads` (a cell above the frame pointer that nothing writes and nothing takes the address of; one definition). With it, hanoi's output with D-I off equals the output with it on.

## What is still to find (exit 2026-11-13)
What remains is the loads only the per-range proof covers (the x87 `fld` of globals in nbody and QCport's d_alias/d_sprite, array descriptors, m16 `es` loads). The pin has no counterpart: either the facts become an invariant-load flag on the instruction (X86InstrInfo's rule with the flag set from the proof), or the default flips per target with the evidence.
