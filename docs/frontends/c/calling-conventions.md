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
