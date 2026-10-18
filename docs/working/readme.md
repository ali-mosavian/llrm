# Working on llrm

For contributors: running tests, finding a miscompile, measuring, and what has broken before.

| Doc | Holds |
|---|---|
| [testing](testing.md) | Running the tests |
| [gate](gate.md) | What CI and the gate run, the tiers, what must pass |
| [ci](ci.md) | CI as the gate: parity with `gate.py run`, private tools, instruction counters |
| [pgo](pgo.md) | the profile-guided llrm-c: how it is built, when its profile is renewed, how to measure with it |
| [debugging](debugging.md) | Finding a miscompile in a DOS program, the debug socket and debug symbols |
| [measurement](measurement/readme.md) | How we measure, the instruments, compile time, comparison with gcc |
| [fragility](fragility.md) | What broke, or hid a break, and what guards it |
| [figures](figures.md) | Drawing a figure as mermaid, and the generated pass pages |
| [codegen-improvements](codegen-improvements.md) | Backend ideas drawn from other compilers |
| [value-tracking-port](value-tracking-port.md) | Evidence for the ValueTracking, KnownBits and ConstantRange port: agreement with LLVM, deviations, owed work |

## Docs with the change

A change to a pass, analysis, flag, level, tool, target or runtime updates its doc in the same PR. A pass's `//!` header is its doc; after editing one, run `tools/fmt.sh` (it rewraps header comments), then `python3 tools/docsbuild/build.py --sync` and commit what it writes. The tests (`pytest tools/docsbuild`, `tests/header_mir.rs`) fail on a stale page, a missing section or a malformed MIR example.
