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
| D64-0 | generator emits an x86-64 table (`Is64BitMode` predicates, `ADD64rr`...) and `Cpu::X64`; register classes come from `registers.regs` (the dword-rooted file, `Regs::root`), not `X86GenRegisterInfo` `GR64`/`GR32` | `X86GenDAGISel.inc` | `(add GR64, GR64)` selects `ADD64rr` on a `TestDag` |
| D64-1 | `x86_addr.rs`: 64-bit base/index/scale/disp32, RIP-relative `X86ISD::WrapperRIP`, `selectLEAAddr`, `selectLEA64_32Addr`; closes SD35 for 64-bit | `X86ISelAddressMode` | `base + index*4 + 8` selects `LEA64r`; fails with the 64-bit arm off |
| D64-2 | `TargetLowering::x86_64` (i64 legal, `is64Bit` arms of the ctor), `BuilderTarget::m64`, adapter ids | `X86TargetLowering` ctor | an i64 add legalizes to `ADD` with no expansion |
| D64-3 | SysV calling convention in the call and formal-argument lowering, read from `calling.toml` (`sysv64`; no register named in the code); `selected_in` accepts bitness 64. x86-64-spike owns the data and reviews | `X86CallingConv.td` | a 6-argument call passes the registers `calling.toml` lists |
| D64-4 | scalar SSE (`f32`/`f64` in `xmm`), conversions; sdag-c owns it, the spike's SSE slices on the old selector are dropped | `X86InstrSSE.td` | `fadd double` selects `ADDSDrr` |

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

## The old selector's m64 facts the DAG path needs

Owner sdag-c unless stated; the spike reviews. Gate for each: `tools/llc_check.sh`,
`llc_backend_diff.sh`, and m16/m32 identity.

| Fact | Where | Date |
|---|---|---|
| `cmov`/`select` | D64-2 (`LowerSELECT` with `CMOV64`) | 12-01 |
| SysV `va_start` register save area | D64-3 (`VASTART` custom) | 12-08 |
| GOT and `rip` externals | D64-1/D64-3 (`WrapperRIP`, `GOTPCREL`) | 12-08 |
| dynamic `alloca` and `stacksave` | D64-3 | 12-08 |
| `rax:rdx` aggregate returns | D64-3 | 12-08 |

## D64-1 design (2026-10-24)

Pin: `X86ISelAddressMode` and its users in `X86ISelDAGToDAG.cpp` (permalinks at d1106deb71cc).
The 64-bit arms are few; the matcher already mirrors the rest.

| Arm | Pin | Change in `x86_addr.rs` |
|---|---|---|
| `Flat64` mode, pointer `i64`, 64-bit base and index | whole file | `AddrMode::Flat64`; the `Flat32` guards (scaled shifts, `MUL_IMM` 3/5/9) become "not `Real16`" |
| RIP-relative symbols | `matchWrapper` L1973-2045: `X86ISD::WrapperRIP`, `AM.setBaseReg(RIP)` unless the code model is large or a base or index already exists | `Wrapper`/`WrapperRIP` both reach `match_wrapper`; `base_reg = RIP` |
| RIP-relative offsets | `foldOffsetIntoAddress` L1883-1910 (`isInt<32>`), L2689 "RIP + 32-bit displacement" | 64-bit value range check; RIP forbids a second register (L2200-2207, L2851) |
| Displacement safety | `isDispSafeForFrameIndexOrRegBase` (the `!is64Bit \|\|` at L2735) | the 64-bit form of the check |
| LEA | `selectLEAAddr` L3329-3379: at 64 bits always a `LEA` for RIP-relative (L3370), `LEA64_32r` for 32-bit results | `lea64addr`/`lea64_32addr` complex patterns beside `lea16addr` |

Test (fail-first): on a `TestDag`, `(add (shl i, 2), base)` plus a displacement selects
`LEA64r` with scale 4 and 64-bit registers; with the `Flat64` arm off it is `ADD64rr`.
Needs D64-0 (the table) first; D64-0 is 11-17, D64-1 11-24 as above.

## D64-4 design (2026-10-25)

Scalar SSE on the DAG path. Each data-file change is its own PR with a fail-first test; the old
selector gets none of it (it goes away). Order:

| Slice | Change | Fail-first test |
|---|---|---|
| S1 forms | shape letter `x` (an xmm register) in `x86.instr` and the parser; `select.rs` places it, `masm` prints `xmmN`; rows for `movss/movsd/movaps` (rr, rm, mr), `add/sub/mul/div/sqrt ss/sd`, `ucomiss/sd`, `cvt` between `ss/sd` and `si32/si64`, `xorps/andps/andnps/orps/pxor` | `addsd xmm0, xmm1` encodes `F2 0F 58 C1`; without the row the form is unknown |
| S2 class + ABI | xmm class from #3258 (a 16-byte value is confined to xmm); `calling.toml` `float_argument_registers = xmm0..xmm7`, `float_result = xmm0`; `al` already named (`variadic_vector_register`) | a `double` argument and result land in xmm0/xmm1 |
| S3 DAG | `TargetLowering` for `X64`: `f32`/`f64` legal in FR32/FR64, `FADD..FSQRT`, `FNEG`/`FABS` (xorps/andps with a pool mask), `FP_EXTEND/ROUND`, `SINT_TO_FP`, `FP_TO_SINT`, `SETCC`/`SELECT` through `UCOMIS`; constants from the pool (rip-relative); the matcher already holds the `X86InstrSSE.td` patterns; emit arms by pin name | `fadd double` selects `ADDSDrr`; `tests/llc` float programs run |
| S4 varargs | save `xmm0-7` under `al != 0` (the half of the register save area left unwritten, SD238); `al` set at a variadic call | `printf("%f")` through `va_arg(double)` |
| S5 corpus | `tests/llc` gains float/double programs (nbody-like, sums, conversions, compares incl. NaN); native scoreboard vs clang | 0 miscompiles |

Long double stays x87 (`f80`, as in the pin). Aggregates in `xmm` (a `{double,double}` return) join S3 after the
integer aggregates (`rax:rdx`), which are D64-3.

## QB at m64 (post-parity row, 2026-10-26)

`llrm-qb --dialect qb45 --runtime qb45 -m64 --isel=dag -fobject-format=elf -O2` over `tests/run/qb` (136 programs, tree = parity + #4847):
2 compile, 134 do not. Off the parity bar (m16 -Os and m32 gate main); unowned.

| First error | Programs | Where |
|---|---|---|
| `@$stringN: a relocation past its data` | 84 | object writer, QB string constants; same with `--isel=old` |
| `overlapping or pre-added relocations` | 24 | same writer |
| `QB runtime routine whose stack cleanup the convention does not model` | 14 | `modelled()` in `dag_function.rs` |
| `QB runtime routine with no contract` | 4 | `abi.contract` at m64 |
| front-end rejections (`DEF SEG` non-constant, `ON` not a statement, `REDIM`, ...) | 8 | qbfront |

NIBBLES.BAS and GORILLA.BAS stop in qbfront: `invalid HIR: pointer concat is not INTEGER:INTEGER to 16:16, or INTEGER:LONG to a flat pointer`.
`fpext float to x86_fp80` is `CannotSelect("FpExtend")` while `sse2` is off at m64.
