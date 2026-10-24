# The DAG path at m64: LEA through the address-mode matcher

Owner: sdag-c. Written 2026-10-24. Plan due 2026-11-10 (with x86-64-spike).

## Why

`llrm-llc -m64` forms no LEA (ours 0, the pin 7 in a 7-function leaf), so `X86FixupLEAs` PR 5
(#4308) has no input. The DAG path builds m16 and m32 only (`selected_in`: "only m16 and m32 are
built so far"); no table, address mode, legalizer table or call convention exists for 64 bits.
Selecting LEAs is `selectLEAAddr` / `matchAddress` over 64-bit registers, which needs the first
three items below, not the whole mode.

## Pieces

| Id | Piece | Pin | Test (fail-first) |
|---|---|---|---|
| D64-0 | generator emits an x86-64 table (`Is64BitMode` predicates, `GR64`/`GR32` classes, `ADD64rr`...) and `Cpu::X64` | `X86GenDAGISel.inc` | `(add GR64, GR64)` selects `ADD64rr` on a `TestDag` |
| D64-1 | `x86_addr.rs`: 64-bit base/index/scale/disp32, RIP-relative `X86ISD::WrapperRIP`, `selectLEAAddr`, `selectLEA64_32Addr`; closes SD35 for 64-bit | `X86ISelAddressMode` | `base + index*4 + 8` selects `LEA64r`; fails with the 64-bit arm off |
| D64-2 | `TargetLowering::x86_64` (i64 legal, `is64Bit` arms of the ctor), `BuilderTarget::m64`, adapter ids | `X86TargetLowering` ctor | an i64 add legalizes to `ADD` with no expansion |
| D64-3 | SysV calling convention in the call and formal-argument lowering; `selected_in` accepts bitness 64 | `X86CallingConv.td` | a 6-argument call passes `rdi..r9` |
| D64-4 | scalar SSE (`f32`/`f64` in `xmm`), conversions | `X86InstrSSE.td` | `fadd double` selects `ADDSDrr` |

## Dates

| Date | PR |
|---|---|
| 11-10 | this plan |
| 11-17 | D64-0 |
| 11-24 | D64-1: the matcher forms LEA at m64 (what #4308 waits on) |
| 12-01 | D64-2 |
| 12-08 | D64-3 |
| 12-15 | D64-4; `select_shadow.py --m64` rows against the pin's `llc -mtriple=x86_64` |

D64-1 is tested on a `TestDag` and needs none of D64-2..4; the leaf's 7 LEAs appear in
`llrm-llc -m64` only once D64-2/3 let it select functions through the DAG path (12-08).
Until then `llrm-llc -m64` runs the old selector; forming LEA there is the old selector's own
address-form matchers, which are not mine and are being deleted.
