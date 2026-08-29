# Measuring

Three numbers, and each answers a different question. Quoting one for another
is how the project acquired figures it later had to withdraw.

## Static, from the manifest

Regions found and taken, bytes before and after, instructions before and after.
Exact, host-side, no emulator, no noise. This is the number that gates a commit.

    uv run python -m qbopt.rewrite FILE.OBJ --report

## Modelled, from qbopt/cycles

Published latencies for 486, P5, P6, K5, K6, K7 and Core.

    uv run python -m qbopt.price FILE.OBJ

**A ranking, not a measurement.** The costs are approximate and are the part of
the tool most worth arguing with; `qbopt/cycles/timings.py` says so itself.
Never quote one of these as a speedup.

## Dynamic, from DOSBox

**DOSBox charges per instruction and models no latency.** A ratio from it is an
executed-instruction ratio, and it answers for an in-order machine and nothing
else. It will report widening as a win at exactly the instruction-count ratio,
on every emulated CPU, forever.

Runs use `conf/pinned.conf` and nothing else. `core=normal` makes the PIT
advance with emulated cycles rather than host time, so two runs of the same
binary must produce identical readings -- if they do not, the machine was not
pinned and no number from it is quotable.

`conf/inherited.conf` reconstructs what the predecessor's parity benchmark ran
on. It is reconstructed, not recorded; nothing in the inherited documents says
what its DOSBox configuration was.

## The timer

`TIMER` ticks at 18.2 Hz, so every reading is a multiple of 54.9 ms. The
predecessor's sections ran 380 ms, which put one tick at 14 per cent of a
reading and 0.2 of error into every ratio -- and produced two figures that were
quoted before being withdrawn.

Read the 8253 instead: latch channel 0 with `OUT &H43, &H00`, read `INP(&H40)`
twice, and combine with the BIOS tick at `0040:006C`, retrying if the tick
changes between the two reads. That is 838 ns rather than 54.9 ms. Sections of
at least five seconds and five repetitions on top, reported as a median with
its spread and the quantisation bound beside it.

## What makes a number quotable

Every row in `docs/numbers.md` carries: the dosbox-x version string, the sha256
of the conf, the sha256 of BC.EXE, LINK.EXE and the runtime library, the
configuration tag, the qbopt git revision, the host, and the date. **A number
without that stamp is not quotable.** That rule is the successor to "treat any
parity figure older than fb9331c as unreliable", which is what the absence of
one cost last time.
