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
the tool most worth arguing with; `src/cycles/timings.rs` says so itself.
Never quote one of these as a speedup.

**And it prices only what is in the object.** BC's side of an absorbed call is
three instructions; the routine behind the call is in the runtime library and is
not counted. An absorbed call therefore reads as a large loss and is not one.
`python -m qbopt.cycles.cycles` carries the routine bodies and prices them.

## Dynamic, from DOSBox

**DOSBox charges per instruction and models no latency.** A ratio from it is an
executed-instruction ratio, and it answers for an in-order machine and nothing
else. It will report widening as a win at exactly the instruction-count ratio,
on every emulated CPU, forever.

Runs use `conf/pinned.conf` and nothing else. `core=normal` makes CPU
execution itself cycle-exact, but `tools/bench.py` found that DOSBox-X's own
BIOS tick bookkeeping is not tied to those cycles closely enough to make a
PIT-precision reading of the same binary repeat exactly -- see "The timer"
below. What repeats exactly is base's own bytes read back byte-for-byte
across runs, and the manifest's static counts; a *timing* reading carries an
absolute noise floor regardless.

`conf/inherited.conf` reconstructs what the predecessor's parity benchmark ran
on. It is reconstructed, not recorded; nothing in the inherited documents says
what its DOSBox configuration was.

## The timer

`TIMER` ticks at 18.2 Hz, so every reading is a multiple of 54.9 ms. The
predecessor's sections ran 380 ms, which put one tick at 14 per cent of a
reading and 0.2 of error into every ratio -- and produced two figures that were
quoted before being withdrawn.

Read the 8253 instead: latch channel 0 with `OUT &H43, &H00`, read `INP(&H40)`
twice, and combine with the BIOS tick at `0040:006C`. That is 838 ns rather
than 54.9 ms in principle -- in `dosbox-x` it is not, in practice. Retrying
when the tick changes between the two reads is not enough: `tools/bench.py`'s
first version, `bench/nbody.bas`'s first version, measured a one-tick-period
tear anyway, on the *same binary, same pinned conf*, repeated. Masking IRQ0 at
the 8259 around the read (BASIC has no `CLI`) did not close it either, which
says the tear is not guest-side -- DOSBox-X updates that memory location on a
schedule of its own, not by actually delivering IRQ0 through a handler a mask
could hold off. A guard band refusing any reading within 12,000 of either edge
of the counter's own period cut the *rate* of one-tick tears but did not
remove them: over 11 repetitions of one binary, readings still spread across
a full 65,536-unit range, and not bimodally -- continuously, which is DOSBox-X
scheduling noise of about one 18.2 Hz tick's worth, not a snapshot bug to fix
further.

That noise floor is absolute, not relative: a longer section shrinks it as a
*fraction* of the reading without shrinking it in ticks. Measured going from a
~5 s section to a ~14 s one: spread fell from 1.1-1.2 per cent of the median to
0.08-0.15 per cent. Sections of at least five seconds and five repetitions on
top, reported as a median with its spread beside it -- and the spread is the
honest error bar, not the quantisation bound the PIT's own resolution would
suggest.

## What makes a number quotable

Every row in `docs/measurement/numbers.md` carries: the dosbox-x version string, the sha256
of the conf, the sha256 of BC.EXE, LINK.EXE and the runtime library, the
configuration tag, the llrm git revision, the host, and the date. **A number
without that stamp is not quotable.** That rule is the successor to "treat any
parity figure older than fb9331c as unreliable", which is what the absence of
one cost last time.
