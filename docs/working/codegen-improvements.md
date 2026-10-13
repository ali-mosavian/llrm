# Code generation improvements

For contributors choosing backend work: the open ideas, each tracked by an issue. The issue holds the evidence and the plan; this page only lists them.

| Idea | Where it pays | Issue |
|---|---|---|
| Keep loop-invariant x87 values on the FPU stack instead of reloading them each trip; merge adjacent `sub esp` | floating loops (`nbody`) | [#1086](https://github.com/ali-mosavian/llrm/issues/1086), [#649](https://github.com/ali-mosavian/llrm/issues/649) |
| Emit a far call to a callee in the caller's own code segment as `push cs` + near call | about 18 clocks against about 6 per call on a 486 | [#1517](https://github.com/ali-mosavian/llrm/issues/1517) |
| Price spills by what the allocator splits, not whole ranges | every allocation decision built on the spill forecast | [#563](https://github.com/ali-mosavian/llrm/issues/563) |
| Split a value live across a loop nest at the loop borders, so trivial phis go before routes fork | loop nests under register pressure | [#1380](https://github.com/ali-mosavian/llrm/issues/1380) |
| Fold a memory operand for a loop-invariant spilled value instead of reloading at the latch | loops with spilled invariants | [#972](https://github.com/ali-mosavian/llrm/issues/972) |
| Keep the allocator's facts between splits and spills instead of rebuilding them | compile time | [#559](https://github.com/ali-mosavian/llrm/issues/559) |

Other compilers' passes are the reference for each: LLVM's `X86FloatingPoint.cpp`, `SplitKit.cpp`, `SpillPlacement.cpp` and `InlineSpiller.cpp`; Open Watcom's `CacheTemps` for the x87 loop case.
