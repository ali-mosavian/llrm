# Docs

| Folder | Holds |
|---|---|
| [architecture](architecture/readme.md) | The pipeline, the MIR boundary, HIR, source layout, packages |
| [frontends/nib](frontends/nib/readme.md) | Nib: language spec, frontend, codegen plan, language server |
| [frontends/qb](frontends/qb/readme.md) | The QuickBASIC source frontend |
| [semantics](semantics/switches.md) | Switches that change what a program means: bounds, overflow, floating point |
| [optimizations](optimizations) | One note per optimization or blocker, each measured on a program |
| [machine](machine) | x86 real mode: ABIs, prefixes, relocations, object formats |
| [measurement](measurement/readme.md) | How we measure, the numbers, and per-program targets |
| [qrender](qrender/readme.md) | The qrender correctness gate and its contract profiles |
| [history](history) | Handover and takeover logs, the inherited plan, the Python port map |
| [examples](../examples) | Nib, BASIC, Pascal and C interop sample programs |

[roadmap.md](roadmap.md) is the ordered plan; [testing.md](testing.md) says how to run the tests.
[codegen-improvements.md](codegen-improvements.md) ranks backend ideas from LLVM, gcc-ia16 and Open Watcom.
