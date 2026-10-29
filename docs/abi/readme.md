# ABI

For contributors to frontends, the backend and the targets, and for anyone linking llrm output with another compiler's objects. This section states the calling conventions the code implements: what is passed where, who removes it, which registers survive a call, and how a convention is chosen for a function and a call.

Each target's `calling.toml` is the one statement of its conventions; the documents here read it and the code that applies it.

## Which document for which program

| Language | x86-m16 (real-mode DOS) | x86-m32 (flat, DOS/32A) |
|---|---|---|
| C (`llrm-cc`) | default `regparm3`; `-mabi=cdecl`, `watcom`, `ia16`, `regparm3`: [x86-m16](x86-m16.md), [c](c.md) | default `watcom` (`watcall32`); `-mabi=sysv`: [x86-m32](x86-m32.md), [c](c.md) |
| Nib (`llrm-nib`) | marks `@export` and `@extern` name the convention: [x86-m16](x86-m16.md), [nib](nib.md) | the same marks: [x86-m32](x86-m32.md), [nib](nib.md) |
| QuickBASIC family (`llrm-qb`) | Pascal order, far calls, by-reference arguments: [basic](basic.md), [measured facts](../frontends/qb/abi.md) | the same procedure convention with every call near, one code space: [basic](basic.md) |
| BC objects (`llrm-omf`) | the Microsoft runtime's stack contracts: [basic](basic.md), [bc](../frontends/bc.md) | none |

## Documents

| Doc | Holds |
|---|---|
| [x86-m16](x86-m16.md) | `cdecl16`, `pascal16`, `regparm3`, `watcall16`, `ia16` and `interrupt16`: registers, stack, near and far calls, results, aggregates, variadic calls |
| [x86-m32](x86-m32.md) | `watcall32`, `cdecl32` and `sysv32`: the same, flat |
| [c](c.md) | How a C function gets a convention; `-mabi=` |
| [nib](nib.md) | Nib's `@export` and `@extern` marks and what each makes of a function |
| [basic](basic.md) | BASIC procedures, the runtime's contracts, `B$ENRA` and `B$EXSA` |
| [selection](selection.md) | How a convention is chosen for a function and a call: `-mabi=`, marks, private functions, `calleepop`, `deadargs`, `argpromotion`, register facts |
| [conformance](conformance.md) | The tests and tools that pin the conventions against BCC, BC, Open Watcom and gcc |

The measured facts of Microsoft's BASIC procedure, array and symbol ABI are in [frontends/qb/abi.md](../frontends/qb/abi.md) and are not repeated here. Aggregates passed by value are in [aggregate-arguments](../reference/semantics/aggregate-arguments.md).

## Stated and not stated

`calling.toml` has no key for stack alignment beyond the slot size (2 bytes on x86-m16, 4 on x86-m32) or for a red zone, and a key no code reads is refused when the file is parsed. No convention has either.
