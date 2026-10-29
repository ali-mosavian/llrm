---
architecture-index: per-function register lists: design note
architecture-order: 460
---
# Per-function register lists: design note

Owner: clib-huge-fill-2256. Dated 2026-10-20. PRs: 2026-10-27 (target description), 2026-11-03 (MIR and llrm-cc), 2026-11-10 (the clib objects).

TL;DR: a function written `#pragma aux f __parm [__ax] [__cx __bx] __modify [__si]` gets a convention derived from a base one in the target description: the base states the registers it may name and what it keeps; the list picks arguments by position; `__modify` is what the callee may change. The derived convention reaches the backend as an ordinary `Convention`, carried as two string attributes in the shape of the pin's `no_callee_saved_registers`; no register name is spelled in backend code.

## What is refused today

llrm-cc refuses a function whose register list is not the default convention's ("has a register calling convention"). On clib that is `_DoINTR` (x2), `__raise`, `__do_exit_with_msg`, `__InitRtns`, `__FiniRtns`, `default_handler` (its address is taken) and `byteswap` (x2, static). The default-list case is landed (#3015).

## LLVM at the pin (d1106deb71cc)

Checked in `X86RegisterInfo.cpp`, `X86ISelLoweringCall.cpp`, `RegUsageInfoCollector.cpp`, `RegUsageInfoPropagate.cpp`, `TargetPassConfig.cpp`:

- **Calling convention by number**, with the register assignment in TableGen (`X86CallingConv.td`: `RegCall`, `X86_FastCall`). `preserve_most`, `preserve_all`, `preserve_none` are such numbers; each picks a fixed CSR list and a fixed `*_RegMask` (`getCalleeSavedRegs`, `getCallPreservedMask`, lines 246-275 and 403-415). No IR construct names argument registers per function.
- **Two string function attributes**, readable on the function and on the call site: `no_callee_saved_registers` (the callee preserves nothing: `CSR_NoRegs`, `X86RegisterInfo.cpp#L251`; on a call, `X86ISelLoweringCall.cpp#L2637` uses `CSR_NoRegs_RegMask`) and `no_caller_saved_registers` (the callee preserves all it writes: `CC = X86_INTR`'s list, `#L246`; on a call, `#L2109`).
- **The call's regmask operand** is machine-level only: it is made in `LowerCall` from the convention (or the attribute) and read by the allocator. IR carries no mask.
- **IPRA** (`-enable-ipra`, off by default): `RegUsageInfoCollector` stores, after allocation, the mask of registers a function really clobbers; `RegUsageInfoPropagate` rewrites the regmask of each call to a function whose mask it knows. Ours is `CalleeFacts` (`backend/calleefacts.rs`): same idea, same position (after the final body), already in; it is not changed here.

What is new: a list that picks argument registers by position, and a callee-changes set finer than the pin's two attributes (`__modify [__si __di]`). The pin's `no_callee_saved_registers` is our `modify` = everything, `no_caller_saved_registers` is `modify` = nothing. The representation follows the pin: string attributes on the function and on each call, on top of the function's own convention number, and no new convention number.

## Design

1. **Description.** `calling.toml` gains, on a base convention, `custom = { pool = [...], results = {...} }`: the registers a list may name for arguments (m16 watcall: `ax dx bx cx si di es` pairs), and the result registers by width. `preserved` of the derived convention is the base's `preserved` minus the list's `__modify`; `clobbered` is the base's plus `__modify`. Everything the backend needs is already a `Convention` field (`argument_registers`, `preserved`, `clobbered`, `results`), so `Convention::derived(base, list, modify)` fills them. A pair `[__cx __bx]` is one argument in two registers (`wide_pairs` style).
2. **MIR.** The function keeps its own convention number (the base). Two string attributes, on the function and on every call site as the pin's `no_callee_saved_registers` is: `llrm-parm-registers` = `ax;cx bx;dx` (one group per argument, `;` between) and `llrm-modify-registers` = `si di es`. Verifier: both parse against the target's register table, and a call's attributes equal its callee's. A call through a pointer carries them on the call.
3. **Resolution.** `isel::entry()` maps the base number plus the two attributes to a derived `Convention`, interned per distinct pair (O(1) per list, one allocation, as `Calling::named`). `calleefacts`, the frame and the call lowering read an ordinary entry: unchanged.
4. **llrm-cc.** `cleanup()` returns the derived pair instead of refusing; `lists_default` (#3015) stays as the case where the derived convention equals the base. Register names come from `c.toml [frontend.registers]` through `Profile.hardware`, as the inline-code path does.
5. **Static, non-escaping.** A `static` function used only by direct calls in its module takes the default convention, whatever its list: the list is unobservable. Not applied when the address is taken, the name is used from inline code or the function is exported. This lives in convention resolution, not in a per-function case (`qsort`'s `byteswap`).

## Tests

Fail first, in the PR that adds each part: (1) `Convention::derived` of m16 watcall with `[ax][cx bx] modify [si]` has the expected registers, preserved and clobbered; a list naming a register outside the pool is refused; (2) a MIR round trip and a verifier case for each failure; (3) llrm-cc: a function and a call with a custom list compile and move the arguments into the named registers (run test in `tests/run/c`, DOSBox, `x86-m16`); the same function under the default list is unchanged; (4) the static rule: address taken keeps the list.

## Cost and risks

O(parameters) per function or call, plus one intern per distinct list. Risks: a list that names a register the base keeps (`bx` in watcall16) forces the callee to save it: derived `preserved` must drop it, and the prologue pushes it (tested with `__modify [__bx]`). Interrupt handlers (`default_handler`) are called from the OS, so the list there is a promise about the caller: not checked.

## Order

(1) description + `Convention::derived`; (2) the two attributes, verifier, `isel::entry`; (3) llrm-cc and the static rule; (4) the clib objects and their run tests. The tools/clib PR (the inventory, library and module-swap scripts that produced the numbers above) is independent: 2026-10-24.
