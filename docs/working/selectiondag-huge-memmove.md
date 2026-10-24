# memmove of huge pointers in the DAG path

Programs: `huge/copyw`, `huge/huge_runs` (`__huge` arrays past 64 KiB). The DAG path refuses ("memmove of far
pointers: the runtime's routine takes near ones"); the old selector copies them forward or backward by windows
(`isel.rs`, `memmove_loads_then_stores` refuses huge, so it uses the proved `llrm.forward`/`llrm.backward`
direction and `rep movs` per window). Owner: sdag-a. Written 2026-10-24; decision due 10-30.

## What the pin does

The pin has no huge pointer. `SelectionDAGBuilder::visitIntrinsicCall` lowers `llvm.memmove` to
`getMemmove` (loads, then stores, when the size is a constant within the inline limit), else a call to the
target's `memmove`, whatever the address space (`SelectionDAG::getMemmove` falls to `getMemmoveLoadsAndStores`
or `TargetLowering::LowerCallTo` of `memmove`). So the pin's answer for a huge pointer is a libcall on the
pointer as the ABI passes it; our ABI passes a huge pointer as selector:offset, and the callee must carry
across the 64 KiB boundary.

## Options (decide 10-30)

1. A runtime routine `_memmove_huge` (cdecl16/regparm3, far pointers in, length as `unsigned long`) that
   normalises per 64 KiB window, copying backwards when dst is above src. Mirrors the pin's libcall; one
   routine, ~40 lines of asm, a test by execution (`copyw`, `huge_runs`) and a unit test that every `libcalls`
   name has its routine (exists: `every_libcall_a_c_toml_names_is_in_its_runtime`).
2. Expand in the builder as a loop of window-sized `memmove`s at the MIR level (like `memsetpattern`): no new
   routine, more code at each site.

Leaning to 1 (the pin calls; code size). The builder side is small: pass the pair as the two register words
and the length widened to 32 bits; the routine's name is a `libcalls` entry.
