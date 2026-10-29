# Calling conventions through llrm-cc

`x86_32-wat` compiles with `-fdefault-calling-conv=regcall` and the backend with `-mregcall-default`.

| Source | clang writes | Backend takes | Symbol |
|---|---|---|---|
| unmarked function | `x86_regcallcc`, `__regcall3__NAME` | the ABI's (`watcall32`, registers) | `name_` |
| `main` | `ccc` | the ABI's | `main_` |
| variadic function | `ccc` (clang never marks one) | the ABI's | `name_` |
| `__cdecl` | `ccc` | C's (`cdecl32`, stack) | `_name` |
| `__stdcall`, `__fastcall` | `x86_stdcallcc`, `x86_fastcallcc` | refused by name | none |

A variadic `__cdecl` function cannot be told from an unmarked one and takes the ABI's.

Rows without `-mregcall-default` (`x86_16-wat`, `x86_32-linux`, ...) keep the old rule: `ccc` is the ABI's.

# Other llrm-llc behaviour on clang IR

- **weak**: `weak`, `linkonce` and `extern_weak` are refused where the object format cannot say them (OMF), naming the symbol. No corpus program uses one.
- **Zero statics**: a non-constant zero static in the data space moves to `_BSS` (`zero_to_bss`), after `BSSB` under `-mbss-begin`. Constants and nonzero values stay in `_DATA`.
- **Foreign memory**: a load or store at a constant address the target lists as foreign (`foreign_span`: video, ROM, BIOS data) is apart from every global and local, so a global is not reloaded after a store to video.

# m16

`x86_16-wat` takes the target's default convention, regparm3, as before: unmarked functions are `_name@3` (callee clobbers CX and DX), variadic ones too, `main` is C's `_main`, `__cdecl` is the stack convention. The row once forced `-mabi=watcom`, whose callee saves BX, CX and DX in every function: -Os programs came out 16% larger. `-mabi=watcom` stays available for interop.

A C `static` function is entered by a near call at m16 (llrm-llc sets `nearcode` on every defined internal function; the backend keeps an address-taken one far). It is an m16 machine fact, not a pin pass: the pin has no near/far code.
