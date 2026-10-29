# Comparisons with other compilers

For maintainers who need to know what llrm is compared with, by which tool, and what that tool cannot say. Each document gives the commands, the flow, the benchmarks and the caveats of one family of competitors. No result is kept in these documents: a number appears only where a committed baseline file holds it, and is cited by file; otherwise the document gives the command that prints it.

## Matrix

| Competitor | Compile time | Generated instructions and size | Run time |
|---|---|---|---|
| gcc | [vs-gcc-clang](vs-gcc-clang.md#compile-time) | [vs-gcc-clang](vs-gcc-clang.md#generated-instructions-and-size) | not measured: nothing runs gcc's output on hardware or timed; the 486 clock column is an estimate |
| clang | [vs-gcc-clang](vs-gcc-clang.md#compile-time) (`levels_time.py`, `scaling.py`; not `scoreboard.py`) | [vs-gcc-clang](vs-gcc-clang.md#generated-instructions-and-size) | not measured, as gcc |
| BC (QuickBASIC 4.5) | not measured: no tool times BC | [vs-basic](vs-basic.md#generated-instructions-and-size), `[bc.*]` | [vs-basic](vs-basic.md#run-time): DOSBox-X RDTSC, follows the instruction count |
| BC of PDS 7.1, VBDOS | not measured | [vs-basic](vs-basic.md#generated-instructions-and-size), `[pds71.*]`, `[vbdos.*]` | [vs-basic](vs-basic.md#run-time) |
| Turbo C 2.01 (`tc`) | not measured | [vs-borland-watcom](vs-borland-watcom.md#generated-instructions-and-size), `[tc.*]` | [vs-borland-watcom](vs-borland-watcom.md#run-time) |
| Turbo C++ 3.0 (`tcpp`) | not measured | [vs-borland-watcom](vs-borland-watcom.md#generated-instructions-and-size), `[tcpp.*]` | [vs-borland-watcom](vs-borland-watcom.md#run-time) |
| BCC 3.1 | not measured | [vs-borland-watcom](vs-borland-watcom.md#generated-instructions-and-size), `[bcc.*]` | [vs-borland-watcom](vs-borland-watcom.md#run-time) |
| Open Watcom | not measured | [vs-borland-watcom](vs-borland-watcom.md#generated-instructions-and-size), `[ow.*]` | [vs-borland-watcom](vs-borland-watcom.md#run-time) |
| Tiny C Compiler (tcc) | not measured: no tooling; `tcc` in the repository is Borland's `TCC.EXE`, the driver of Turbo C and C++ | not measured | not measured |

Where the table says run time for a DOS compiler, the clock is DOSBox-X's, which charges about one cycle per instruction. It is a second view of the instruction count, not a measurement of a machine.

## Instruments

```
                 what is run                          what is counted
 gcc, clang      vsgcc/run.sh: ELF + OMF, unicorn     executed instructions, memory ops,
 (m32)                                                486 clocks (estimated), code bytes
 bcc, tc, tcpp,  tools/bench: OBJ + jwlink, unicorn   executed instructions, memory ops,
 ow, bc, pds71,  (real mode), DOSBox-X for --time     code/data/bss bytes, kernel_ms
 vbdos (m16)
 compile cost    perf stat instructions:u             user instructions of one compile
 (llrm vs gcc,   (levels_time, scoreboard, scaling)
  clang only)
```

Two processors are in play: the 32-bit comparison counts a protected-mode 486 stream, the 16-bit one a real-mode stream. The same 486 clock table (`cost()` in `crates/target/llrm-x86-m32/vsgcc/harness.py`) prices both; it is an estimate in each.

## Documents

- [vs-gcc-clang](vs-gcc-clang.md): llrm's `m32` output and compile cost against gcc and clang. Tooling: [vsgcc](../../crates/target/llrm-x86-m32/vsgcc/readme.md), [scaling](../../crates/target/llrm-x86-m32/vsgcc/scaling.md).
- [m32-vs-m16](m32-vs-m16.md): llrm's `m32` against its `m16` on the same programs: the requirement that m32 is no worse. Tooling: `vsgcc/against_m16.py`.
- [vs-borland-watcom](vs-borland-watcom.md): llrm-cc's 16-bit output against BCC, Turbo C, Turbo C++ and Open Watcom, by `tools/bench`.
- [llvm](llvm/readme.md): why LLVM's compile is cheap and llrm's is not, on identical inputs: the ladder, per-pass and cache profiles, mechanism pages with LLVM and llrm source excerpts, and the ranked proposals. Tooling: `tools/ladder`.
- [vs-basic](vs-basic.md): llrm-qb against the BC compilers of QuickBASIC 4.5, PDS 7.1 and VBDOS: counts, sizes, output.

Other instruments, and what each answers: [measuring](../working/measurement/readme.md). Compile-time method and the per-pass tools: [compile-time](../working/measurement/compile-time.md).

The gaps above are tracked in [#1534](https://github.com/ali-mosavian/llrm/issues/1534).
