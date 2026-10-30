# Register pressure for the pre-RA scheduler (codegen row 65, pressure side)

TL;DR: `machinesched.rs` counts pressure as one number against one limit (DEVIATE MS3). The pin counts it per pressure set, with per-set limits, live-through values and the `RegExcess`/`RegCritical`/`RegMax` comparisons. This is the pressure half, in `backend/regpressure.rs`; w2 keeps the scheduling loop in `machinesched.rs` and calls it.

## The pin, at d1106deb71cc

- `RegisterPressure.cpp`: `RegPressureTracker` (upward and downward `advance`, `getUpwardPressureDelta`, `getDownwardPressureDelta`, `RegisterOperands`, `PressureDiff`), `RegPressureDelta` {`Excess`, `CriticalMax`, `CurrentMax`}.
- Limits: `RegisterClassInfo::getRegPressureSetLimit` = `computePSetLimit`: the target's table value minus the reserved registers' weight (`RegisterClassInfo.cpp#L263`).
- X86 tables (from `X86GenRegisterInfoTargetDesc.inc`, dumped once): 36 sets (`getRegPressureSetLimit` table, e.g. GR32_BPSP 6, GR32_BSI 6, GR8_NOREX 10, RFP32 7, GR16 66), every register unit weight 1, class -> sets in `RCSetsTable`.

## Pieces

1. Data: the pressure sets of the targets in use (class -> sets, per-set limit adjusted for reserved registers) as a table with its provenance, a test that pins each limit to the dump. No behaviour change.
2. `RegPressureTracker` over LIR: `new(live_out)`, `advance_upward(insn)`, `delta(insn) -> RegPressureDelta`, `max_set_pressure`, with live-through counted. Unit tests on hand-built blocks, red first.
3. The comparisons: `tryCandidate`'s `RegExcess`, `RegCritical`, `RegMax` as functions on two deltas. w2 swaps `pressure_after`/`peak` in `bottom_up` for these; MS3 goes when it does.

Rows: 71 programs static + AM-BT with `OBJSWEEP_FLAGS=--isel=dag`, clocks and bytes before/after per piece.
