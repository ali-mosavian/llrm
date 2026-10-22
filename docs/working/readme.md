# Working on llrm

For contributors: running tests, finding a miscompile, measuring, and what has broken before.

| Doc | Holds |
|---|---|
| [testing](testing.md) | Running the tests |
| [pr-rules](pr-rules.md) | The goal, what a PR carries, how it lands, how to measure, safety |
| [gate](gate.md) | What CI and the gate run, the tiers, what must pass |
| [ci](ci.md) | CI as the gate: parity with `gate.py run`, private tools, instruction counters |
| [pgo](pgo.md) | the profile-guided llrm-c: how it is built, when its profile is renewed, how to measure with it |
| [debugging](debugging.md) | Finding a miscompile in a DOS program, the debug socket and debug symbols |
| [measurement](measurement/readme.md) | How we measure, the instruments, compile time, comparison with gcc |
| [fragility](fragility.md) | What broke, or hid a break, and what guards it |
| [figures](figures.md) | Drawing a figure as mermaid, and the generated pass pages |
| [codegen-improvements](codegen-improvements.md) | Backend ideas drawn from other compilers |
| [fold](fold.md) | What the Fold pass costs and carries, and the plan to remove it |
| [fold-split](fold-split.md) | The plan splitting the fold work between valuetracking-port and fold-poc |
| [value-tracking-port](value-tracking-port.md) | Evidence for the ValueTracking, KnownBits and ConstantRange port: agreement with LLVM, deviations, owed work |
| [undef-table](undef-table.md) | MIR `undef`: each pass and pin function that treats it apart from poison, and its state |
| [qb-float-rows](qb-float-rows.md) | The 41 QB float rows above main after BC's per-assignment rounding (#3352): the numbers, owner and exit |
| [qcport-os-rows](qcport-os-rows.md) | The QCport modules bigger at -Os after #3369 and after its re-land: numbers, owner and exit |
| [phi-pre-ra-translation](phi-pre-ra-translation.md) | PHIElimination, TwoAddressInstruction and the pass order before greedy, against the pin: same, differs, missing |
| [twoaddress-translation-plan](twoaddress-translation-plan.md) | Translating TwoAddressInstructionPass: the copy-count premise, the data LIR lacks, seven pieces in the pin's order |
| [process-implicit-defs-premise](process-implicit-defs-premise.md) | ProcessImplicitDefs: what the pin's pass does (IMPLICIT_DEF to undef on 16 to 32 widening), ours, the extends compared |
| [vreg-class-field-plan](vreg-class-field-plan.md) | A register class on each virtual register: pin sites, touched types, caller counts, order |
| [local-reassign](local-reassign.md) | `canReassign` in the greedy allocator: the QCport -Os modules that change, vs parity and main |
| [qcport-vs-main](qcport-vs-main.md) | QCport -Os and -O2, parity against main, per module, with the owner of each module above main |
| [compile-ledger-w1](compile-ledger-w1.md) | Plan for the regalloc-glue and scalar buckets of the compile ledger: ours vs the pin per pass, callees, levers and dates |
| [unroll-o1-rows](unroll-o1-rows.md) | x_life and x_isqrt against main: the -O1 unroll mechanism and the select diamond, both the pin's |

## Docs with the change

A change to a pass, analysis, flag, level, tool, target or runtime updates its doc in the same PR. A pass's `//!` header is its doc; after editing one, run `tools/fmt.sh` (it rewraps header comments), then `python3 tools/docsbuild/build.py --sync` and commit what it writes. A page of `docs/architecture/` joins its readme's Contents list by front matter (`architecture-index:` and `architecture-order:`, tools/docsbuild/archindex.py); `build.py --sync` rewrites the list, and `--check` fails on a stale one. The tests (`pytest tools/docsbuild`, `tests/header_mir.rs`) fail on a stale page, a missing section or a malformed MIR example.
