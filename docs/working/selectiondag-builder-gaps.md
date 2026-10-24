# SelectionDAG builder: what the pin's `visit*` has that ours lacks

Survey of `SelectionDAGBuilder.cpp` at the pin (d1106deb71cc) against `selectiondag/builder*.rs`, 2026-10-23. The corpus (466 programs, m16 and m32, `tools/dagisel/builder_corpus.py`) refuses nothing now; this lists what a program outside it can still hit. Owner sdag-a for all rows.

## Translated

`visitPHI` (as register exports), `visitRet`, `visitUncondBr`/`visitCondBr`, `visitSwitch` with `lowerWorkItem`, jump tables and bit tests, `visitInvoke` and `visitLandingPad` (QB only, SD102), `visitUnreachable`, `visitBinary`/`visitUnary`/`visitShift`/`visitSDiv`, `visitICmp`/`visitFCmp`, `visitSelect`, every cast, `visitAddrSpaceCast`, `visitGetElementPtr`, static `visitAlloca`, `visitLoad`/`visitStore`, `visitCall` (direct, indirect, byval, sret), `visitInlineAsm` (as `LLRM_ASM`), `visitVAStart`/`VAEnd`/`VACopy`, `visitFreeze`, and in `visitIntrinsicCall`: mem*, funnel shifts, lifetime, min/max, bit counts, bswap, abs, float functions, lrint, plus llrm's `asm`/`code`/`window`/`ptrdiff`; `*.with.overflow` and `extractvalue` (#4058), `scmp`/`ucmp`, `fmuladd`, `smul.fix`/`sdiv.fix`, `assume`/`expect`, `stacksave`/`stackrestore` (#4061), dynamic `alloca` (#4062); far and huge pointers, QB `invoke`/`landingpad`, `freeze`, `memcpy` from constant data, float call results.

## Gaps

| Pin function | MIR source | Status | Note |
|---|---|---|---|
| `visitUnaryFloatCall`/`visitBinaryFloatCall`, `copysign` | a call of `sin`, `fmod`, `copysign`, ... by name | open, no corpus use | the pin makes `FSIN`/`FREM`/... of a call to a declared libm function that is no local, known to `TargetLibraryInfo`, and (for `copysign`) `onlyReadsMemory`; the middle-end canonicalizes most to intrinsics, which are built; ours keeps the call |
| `visitIntrinsicCall`: port in/out | `Intrinsic::{PortIn, PortOut}` | open, none in the corpus | llrm's own (`llvm.x86.*` are the pin's analogue); a node pair like `LLRM_ASM` |
| `visitMemCmpBCmpCall`, `visitStrCpyCall`, `visitStrLenCall`, ... | calls of `memcmp`, `strlen`, ... | none | X86 has no `EmitTargetCodeFor*` for them, so the pin makes the same call |
| `visitMaskedLoad`, `visitVP*`, vector ops, `visitVectorReduce` | no vector MIR | deferred | vectors wait for 32-bit parity |
| `visitAtomic*`, `visitFence` | no atomic MIR opcode and no `__atomic`/`__sync` in the Watcom front end (checked 2026-10-24): no program can exist | none | |
| `visitResume`, `visitCatchPad`, `visitCallBr`, `visitIndirectBr`, `visitStackmap`, `visitPatchpoint` | no MIR source, or QB's runtime handles it | none | |
| `visitVAArg` | no `va_arg` in MIR | none | frontends lower it |
| `lowerStartEH`/`lowerEndEH` | `EH_LABEL`s | none | SD102 |
| `visitDbgInfo` | debug values | none | no debug MIR in the DAG path |
