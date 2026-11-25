# arm64 inline asm, flat route: scope (2026-11-25, handover)

M1 is done: branch `feat/arm64-asm-blocks` 222b6eb380 (llrm-mir: `llrm.a64.asm.` name, `AARCH64` tag in `AsmBlock.bits`, verifier takes x/w/v types).

## Why flat
The pin lowers inline asm in the backend (`visitInlineAsm`, `AsmPrinter::emitInlineAsm`, MC `AsmParser`). We have no arm64 text assembler (`llrm-arm64/src/mc` encodes MCInst only) and the IR parser has no `asm` callee. The x86 route works end to end: `llrm-cc` assembles the text with the pin clang, bytes and fixed registers become `llrm.<isa>.asm.*`, and the backend lowers one node (`selectiondag/inline_asm.rs` `lower_asm`, shared).

## M2: `llrm-cc` (inline_asm.rs, lib.rs ~1218)
- `Flat`/`translate_flat` are x86-shaped: `.code32`/intel header, `%reg` views, address-width pools, `-target i386|x86_64-unknown-none` + `objcopy -O binary`. arm64 needs: target `aarch64-unknown-none`, no `%`, operands `$0` with modifiers `${0:w}`, `:x`, `:s`, `:d`, `:q`.
- Register file: `Target::registers_text` of llrm-arm64; pool x9..x15 (caller-saved, outside the argument registers where possible), `w`/`x` view by operand type (`i32`/`i64`/`ptr`), `v` registers for float/vector.
- Constraints: `r`, `w`, digit ties, `{reg}`, multi-alternative sets by the same rule as x86 (register preferred; `m` indirect = address in a register). Pool overflow or an unsupported constraint = named error.
- Names: same collision rule as x86 (w12's block-name class).

## M3: backend
- A target ISD node for the block + selection to a pseudo (x86: `CALL_ASM` in `llrm-x86-backend/src/dag_function.rs`), `LoweringNodes.asm`, the clobber set read from the name (as a call), emit as bytes in the object and `.inst` words in text.
- Fail-first: a udiv/msub-shaped block compiles and runs under qemu-aarch64, red on base (`expected a constant, found asm`).

## Estimate and gates
4-6 days; one PR, three milestones. 126 arm64 modules md5 unchanged, x86 names unchanged. Row ARM64-INLINEASM-FLAT DEVIATE (flat route, the pin assembles in the backend).

## Related findings for w12 (x86 asm classes)
- `rm`/`g`/`imr` multi-alternative: done (#10799).
- Immediate single letters (`i`, `n`, `I`..`K`): still refused by name; LLVM picks the immediate class when the operand is a constant.
- Block names omit operand types: head ac4afcdcd7 (`.sN` suffix) pending vouch.
