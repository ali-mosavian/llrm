# C calling conventions

For contributors to `llrm-cc`. How a C function gets a convention, and what a C programmer gets from `-mabi=` and the calling-convention keywords. The register and stack layouts are in [x86-m16](x86-m16.md) and [x86-m32](x86-m32.md).

Clang writes the calling convention on each function (`ccc`, `x86_regcallcc`, ...) and `llrm-llc` maps it to the target's ABI; the mapping is in [calling-conventions](../frontends/c/calling-conventions.md). `-mabi=FAMILY` picks the ABI the unmarked functions take.

## Families

`cdecl` and `registers` are the `calling.toml` conventions an explicitly `__cdecl` function and an unmarked one take.

| Target | `-mabi=` | Unmarked | `__cdecl` |
|---|---|---|---|
| x86-m16 | `cdecl` | `cdecl16` | `cdecl16` |
| | `watcom` | `watcall16` | `cdecl16` |
| | `ia16` | `ia16` | `ia16` |
| | `regparm3` (default) | `regparm3` | `cdecl16` |
| x86-m32 | `watcom` (default) | `watcall32` | `cdecl32` |
| | `sysv` | `sysv32` | `sysv32` |

Under `-mabi=watcom` on x86-m16, `__watcall` keeps Open Watcom's meaning, which Borland's runtime does not have: the runtime's declarations are cdecl. Under `regparm3` the Borland runtime's declarations and every function that names a convention stay cdecl.

A symbol's name is decorated by its convention and the object format (`_name`, `name_`, `NAME`, `_name@3`; tables in [x86-m16](x86-m16.md#the-conventions) and [x86-m32](x86-m32.md#the-conventions)). `#pragma aux f "NAME"` spells the symbol as written.

## Distance and interrupts

A function is far when it is `__far` or in a far code model, interrupt when it is `__interrupt`, near otherwise. On x86-m32 `__far` and `__huge` pointers are near, with a warning: one address space.

An interrupt handler's parameters are the saved registers, named in Borland's order, each its slot of the frame ([x86-m16](x86-m16.md#interrupt-handlers)); what the handler writes to one is what it returns to. A target with no interrupt frame refuses the handler.

## Aggregates and variadic functions

A struct result and a struct argument follow the convention ([x86-m16](x86-m16.md#the-conventions), [x86-m32](x86-m32.md#the-conventions)); one over 16 bytes is a `byval` pointer ([aggregate-arguments](../reference/semantics/aggregate-arguments.md)). A variadic function: the caller pops and a variadic callee keeps its arguments in their stack slots, so `va_start` walks them; a call of a register convention's variadic function is made in the convention `variadic` names.

## Tests

[conformance](conformance.md) lists the tests.
