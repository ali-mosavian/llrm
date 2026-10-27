> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/regpressure.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/regpressure.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/regpressure.rs sha256:3ce590ea74b7b62b84f646f61d6b4819000ba635a30583c1ca2082043c011864 -->

# regpressure

Register pressure per pressure set, for the pre-RA scheduler:
`RegPressureTracker` of the pin (RegisterPressure.cpp) as `GenericScheduler`
asks it of a bottom-up region.

## How it works
Every value counts its class's weight in each pressure set the class is in
(`pressure.sets`, `llrm_target::pressure`). The tracker holds the pressure
of each set at the point reached, the most each has been, and what passes
through the region (`LiveThruPressure`, which raises the limit of the set it
is in). `Tracker::advance_upward` is `recede`: the instruction's defs stop
being live above it and its uses start. `Tracker::upward_delta` is
`getUpwardPressureDelta`: what placing an instruction next would do, as the
first set (in set order) that goes over its limit (`Excess`), over a
critical set's max so far (`CriticalMax`), or over the region's max pressure
(`CurrentMax`).

## Example
One set of limit 2, `a` and `b` live: an instruction using a third value `c`
and defining nothing live takes the pressure to 3, one over: `Excess` is
(set 0, +1). An instruction that defines `a` and uses `c` leaves it at 2.

## Not translated
DEVIATE RP1: the pin's value class is exact (instruction selection set it);
a value's class here is the one its allowed registers pick out. DEVIATE RP2:
the dead defs and lane masks the pin tracks (x86 values have one lane; a
dead def does not change `getUpwardPressureDelta`, only the max `recede`
keeps). The downward direction (`TopRPTracker`) waits for a top-down pass.

## Used by
Named by `machinesched`.
