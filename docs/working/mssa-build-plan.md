# MemorySSA build cost: the pin builds as often, 11x cheaper per build

Owner w-3. d_faces `-O2`, ledger command, dist `c7013c0cb` (ours) against the pin `d1106deb71cc` (`clang -O2 -c`, same flags).

| | builds | total Ir | per build |
|---|---|---|---|
| pin `MemorySSAAnalysis::run` | 25 | 2.97 M | 119 K |
| ours `MemorySsa::run` | 28 (LICM 10, MemCpyOpt 11, EarlyCSE 7) | 38.1 M | 1.36 M |

Ours per build: `PlainAccesses` 32.8 M (run 21.5 over 27: `Annotated` 8.1, `Writes` 7.6, `Accesses::plain` 5.8; update 11.1 over 14) and `Mssa::build` 5.3 M (190 K, the pin's order). The pin's build makes no per-instruction reference and asks no alias question.

Measured and dropped, all on dist, same object: lazy `Annotated` per instruction +1.7 M (the consumers force most entries); lazy liveness in copy propagation +0.02 M; LICM handing its updated graph back +0.006 M (every build is cold: the pass before it edited the body). The count is not the lever; the cost of one build is.

## Steps (one PR each, objsweep and QCport unchanged, dist Ir quoted)

1. `AliasQuery::location_of` takes the location from `MemoryLocation::get(inst)`, not `Accesses::references` (alias.rs: 1 site).
2. A load's or store's reads and writes come from its opcode (`own_bytes`); every load and store has a reference, so the lists are never empty. `Accesses` keeps only calls.
3. `effect_of` and `call_effects` take a call's effect from `AAResults::getModRefInfo(Call, nullptr)` over the attributes and GlobalsAA, as `buildMemorySSA` does; `Writes` is then read by no MemorySSA client.
4. The consumers' remaining `Accesses` reads move to `MemoryLocation` and the stack: MemCpyOpt 14 sites, DSE, LICM 3.
5. `MemorySsa::run` asks no `PlainAccesses`; `Annotated` and `Writes` lose their last MemorySSA reader (S3c deletes `Annotated`).

Gain if all five hold: up to the 32.8 M minus what the other readers of `Annotated` and `Writes` still pay (they are shared: IndVars, `FloatFacts`, `ThroughMemory`). Step 3 is the risk: a call's footprint (`Writes`) is llrm-only and decides `unmodeled_write`; the shadow compares `effect_of` both ways over the corpus before the switch.
