# SelectionDAG builder: what the pin's `visit*` has that ours lacks

Survey of `SelectionDAGBuilder.cpp` at the pin (d1106deb71cc) against `selectiondag/builder*.rs`, 2026-10-23. The corpus (466 programs, m16 and m32, `tools/dagisel/builder_corpus.py`) refuses nothing now; this lists what a program outside it can still hit. Owner sdag-a for all rows.

## Translated

`visitPHI` (as register exports), `visitRet`, `visitUncondBr`/`visitCondBr`, `visitSwitch` with `lowerWorkItem`, jump tables and bit tests, `visitInvoke` and `visitLandingPad` (QB only, SD102), `visitUnreachable`, `visitBinary`/`visitUnary`/`visitShift`/`visitSDiv`, `visitICmp`/`visitFCmp`, `visitSelect`, every cast, `visitAddrSpaceCast`, `visitGetElementPtr`, static `visitAlloca`, `visitLoad`/`visitStore`, `visitCall` (direct, indirect, byval, sret), `visitInlineAsm` (as `LLRM_ASM`), `visitVAStart`/`VAEnd`/`VACopy`, `visitFreeze`, and in `visitIntrinsicCall`: mem*, funnel shifts, lifetime, min/max, bit counts, bswap, abs, float functions, lrint, plus llrm's `asm`/`code`/`window`/`ptrdiff`.

## Gaps

| Pin function | MIR source | Date | Note |
|---|---|---|---|
| `visitIntrinsicCall`: `*.with.overflow`, `smul.fix`/`sdiv.fix`, `scmp`/`ucmp`, `fmuladd` | `Intrinsic::{WithOverflow, Fixed, Compare, FMulAdd}` | 11-20 | the pipeline lowers them at -O1 and up; -O0 and other frontends can reach them |
| `visitIntrinsicCall`: `assume`, `expect`, `stacksave`, `stackrestore`, port in/out | `Intrinsic::{Assume, Expect, StackSave, StackRestore, PortIn, PortOut}` | 11-20 | `assume`/`expect` build nothing or the value; the rest are nodes |
| `visitAlloca`, dynamic | `Alloca` outside the entry block | 11-27 | `DYNAMIC_STACKALLOC`; absent from the corpus |
| `visitExtractValue`/`visitInsertValue` | `ExtractValue`, `InsertValue` | 11-27 | aggregates in registers; absent from the corpus |
| `visitUnaryFloatCall`/`visitBinaryFloatCall` | a call of `sin`, `fmod`, `copysign`, ... by name | 11-27 | the pin turns a libm call into `FSIN`/`FREM`/... when the declaration allows; ours keeps the call |
| `visitMemCmpBCmpCall`, `visitStrCpyCall`, `visitStrLenCall`, ... | calls of `memcmp`, `strlen`, ... | none | X86 has no `EmitTargetCodeFor*` for them, so the pin makes the same call |
| `visitMaskedLoad`, `visitVP*`, vector ops, `visitVectorReduce` | no vector MIR | deferred | vectors wait for 32-bit parity |
| `visitAtomic*`, `visitFence` | no atomic MIR | none | |
| `visitResume`, `visitCatchPad`, `visitCallBr`, `visitIndirectBr`, `visitStackmap`, `visitPatchpoint` | no MIR source, or QB's runtime handles it | none | |
| `visitVAArg` | no `va_arg` in MIR | none | frontends lower it |
| `lowerStartEH`/`lowerEndEH` | `EH_LABEL`s | none | SD102 |
| `visitDbgInfo` | debug values | none | no debug MIR in the DAG path |
