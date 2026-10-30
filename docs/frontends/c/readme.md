# C frontend

Audience: users and contributors of `llrm-c`. How C source is compiled, which
dialect and ABI it follows, and its options.

`llrm-c` compiles C through Open Watcom's own front end and hands MIR to the
shared optimizer and backend.

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
flowchart TB
  classDef src fill:#2563eb,stroke:#1e40af,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef opt fill:#ea580c,stroke:#9a3412,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef out fill:#16a34a,stroke:#166534,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef data fill:#7c3aed,stroke:#5b21b6,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  linkStyle default stroke:#64748b,stroke-width:2.2px
  src["<b>program.c</b>"]:::src
  wccq["<b>wccq: Open Watcom's C front end (cc/)</b><br/><small>linked against toolchain/owshim/cgshim.c<br/>instead of its code generator,<br/>with llrm's patches<br/>(toolchain/owshim/patches)</small>"]:::opt
  stream["<b>stream: one line per cg call</b><br/><small>types, symbols, trees, data, debug</small>"]:::mid
  llrm["<b>llrm-c</b><br/><small>hir.rs reads the stream<br/>translate.rs makes HIR<br/>ow_facts.rs turns the facts it<br/>states into MIR facts</small>"]:::out
  mir["<b>HIR -#gt; MIR</b>"]:::mid
  opt["<b>shared optimizer, backend</b>"]:::mid
  obj["<b>OMF object</b><br/><small>(or ELF, Mach-O, COFF)</small>"]:::data
  src --> wccq
  wccq --> stream
  stream --> llrm
  llrm --> mir
  mir --> opt
  opt --> obj
```

*From C source to an object file*

The front end does the parsing, type checking, constant folding and struct
layout; `cgshim.c` records what it asks of its code generator, and nothing
generates code there. What the stream carries and what `llrm-c` does with it
is in [the front end contract](ow-contract.md). `llrm-c` also accepts a
recorded stream (`.cgs`) as its input.

[`llrm-cc`](clang-route.md) is the same pipeline from clang's front end instead of Open Watcom's; [its calling conventions](calling-conventions.md).

## Targets and ABI

`-m16` compiles for real-mode DOS and `-m32` for flat 32-bit DOS; the target
description names which Open Watcom tree builds the front end (`i86` or `386`),
what `int` is, and the switches `wccq` is given (`runtime/c/x86-m16/c.toml`,
`runtime/c/x86-m32/c.toml`).

On real mode the default is Borland C 3.1's medium model: far code, near
data, cdecl, signed `char`, byte-packed structs, 16-bit
enums, inline x87 and no stack probes. The patched front end also lays bit
fields out as Borland C does. `long double` is x87's 80-bit type at every target (llrm-c adds `-fld` where the layout has it; the
size in memory is the description's, `Layout::long_double_bytes`, the front end is built with it and `__SIZEOF_LONG_DOUBLE__` is it: 12 at m32, with 4-byte alignment, and an argument takes whole 12-byte slots). `-mabi=ABI` selects another ABI of the target:

| Target | ABIs |
|---|---|
| `x86-m16` | `cdecl` (Borland, the default), `watcom`, `ia16`, `regparm3` |
| `x86-m32` | `watcom`, `sysv` |

The C runtime and its start-up are in `runtime/c`, on top of the OS layer in
`runtime/shared/dos`.

## Usage

```
llrm-c SOURCE [-o OUT] [-I DIR]... [options]
```

`-o OUT.obj` (or `.o`) writes an object; any other output name, or `-S`,
writes assembly. The object format is `-fobject-format=omf|elf|macho|coff`.

| Option | Does |
|---|---|
| `-I DIR`, `-IDIR`, `--include DIR` | Add an include directory |
| `-DNAME[=VALUE]`, `-UNAME` | Define or undefine a macro (`-D NAME` too) |
| `-fsigned-char`, `-funsigned-char` | Whether a plain char is signed; each ABI family has its default (Borland's and gcc's signed, Open Watcom's unsigned) |
| `-std=c99` | The language standard, the front end's `-zastd=` |
| `-oa` | Open Watcom's relaxed alias checking: an object whose address is never taken is unaliased |
| `-on` | Open Watcom's relaxed floating point |
| `--dump DIR` | Write every stage to `DIR` |
| `--os-layer FIELD` | Print one field of the target's OS layer and exit |
| `-g` | CodeView line numbers, symbols and types |
| `-fsanitize=stack` | Check the stack on entry, comparing with Open Watcom's `_STACKLOW` and calling `__STKOVERFLOW` |
| `-ftrapv`, `-fwrapv` | Signed overflow traps, or wraps |
| `-fexcess-precision=standard`, `-ffloat-store` | A conversion to a float type rounds to that type (C11 6.3.1.4, 6.3.1.5); by default x87 keeps the wider value in its register, as gcc's fast mode does |

The shared options of `llrm-qb`, `llrm-c` and `llrm-nib` apply: `-O0` to
`-O3`, `-Omax`, `-Os`, `-Oz`, `-Og`, `-f[no-]PASS`, `-march`, `-mtune`,
`-m[no-]stack-is-data`, `-m[no-]far-bss`, `-mcode-segment=NAME` (every module's code in the one segment, as Open Watcom's `-nt=`), `-m[no-]bss-begin` (an empty `BSSB` segment
ahead of each object's `_BSS`, where a start-up zeroes from it, as the QB runtime's does), `--machine`, `-fstack-usage` and
`-Wstack-usage`. `llrm-c --help` prints the full list.

A program that includes `llrm_os.h` finds it without `-I`: the header is the
target's OS layer interface. The target's physical addresses are defined as
`PHYSICAL_<NAME>`.

## Inline code with registers

Open Watcom's `#pragma aux name = "bytes" __parm [..] __value [..] __modify [..]` is inline code that takes its
parameters in the registers it names and answers in the registers it names. `llrm-c` turns it into an `asm` block:
the parameters are its inputs, the answer its outputs, and what it modifies its clobbers. A register it modifies is
saved by a function that keeps that register for its caller. Where a register pair holds a far pointer, the segment is
the register Open Watcom's compiler picks (a segment register always; otherwise the offset is the first of ax, bx, si, di, dx, cx). The registers, their bits and ranks are the target's description (`[frontend.registers]` of its `c.toml`), not the
front end's. What it cannot
express (a register convention of a function's own, a parameter on the stack) is refused, not guessed at.

Beyond plain registers: a parameter may be in `es`, a byte register takes the low byte of a byte or a word, a struct
of 1, 2 or 4 bytes is answered as the integer of its size, a 64-bit value in four registers has the target's top word
first (`quad` in `[frontend.registers]`), and `__parm __caller []` code finds its arguments on the stack, pushed in
order and popped by the caller.

## Inline code that lowers SP

`#pragma aux __doalloca = "sub sp,ax" __value [__sp] __modify __exact [__sp]` (Open Watcom's `alloca`) says it changes
SP. A function that contains such a call keeps a frame register, pushes every register it keeps at its entry (no
shrink-wrapping) and returns by setting SP from the frame register before popping them; the register allocator and the
stack addressing see SP as changed by the call. Inline code that does not declare SP is assumed to leave it.

## The start-up and init segments

With `-fentry-requests` (a program linked against Open Watcom's library; llrm's own runtime has no `_cstart_`) a module that defines `main` asks its linker for the start-up (`_cstart_`) and, where `main`
takes arguments, for `__argc`, which builds `argv`: the references Open Watcom's own compiler leaves there
(`entry_requests` in the target's `c.toml`). Segments a linker lays out between two markers (`XIB:XI:XIE`, the init and
final routines the start-up walks) are declared with their markers in every object (`bracketed_segments`): without
them a linker laid the ranges out empty, `argc` was 0 and no initializer ran.

## Numbers as pointers

A number used as a pointer (`NULL`, Open Watcom's `SIG_DFL` = `(handler)2`) is the pointer with those bits, whatever
pointer it is used as: as a far pointer its segment is 0, not the data segment a near pointer would be given.

## Code pointers in data

A function's address in static data is as wide as a code pointer of the model: 4 bytes where code is far (medium,
large), 2 where it is near. At two bytes in a far-code model a table of handlers (Open Watcom's `signal` table) was
half its size.

## `__attribute__`

gcc's `__attribute__((...))` is read where gcc allows it on a declaration: before the type and after a declarator
(`int f(void) __attribute__((noinline));`, which holds for the definition that follows it). The target's header spells it as
Open Watcom's `__declspec( ... )`, and the patched front end reads the list, commas, arguments and `__name__` spellings
included. `noinline`, `always_inline` and `noreturn` state facts. `unused`, `used`, `cold`, `hot`, `format`, `nonnull`,
`pure`, `const`, `malloc`, `deprecated`, `visibility` and a few others are read and dropped: leaving them out changes no
meaning. Any other (`packed`, `aligned`, `section`, `weak`, `alias`) is an error, not ignored.

## Block locals' lifetimes

From -O1 up, a local declared in a nested block is started where its block
begins (`llvm.lifetime.start`) and ended where the block's end is reached, as
clang's `EmitAutoVarAlloca` and `CallLifetimeEnd` do
([CGDecl.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/clang/lib/CodeGen/CGDecl.cpp)).
The blocks are Open Watcom's `DBBegBlock`/`DBEndBlock`, each followed by its
`CGAutoDecl`s. Inlined copies and frame-slot sharing (`lifetimes.rs`) read the
markers. Function-level locals get none.

A block left by `break`, `continue`, `goto` or `return` is not ended on that
path: its locals stay live to the next start, which costs sharing, not
correctness. A block a jump enters from outside is not marked at all (clang's
`Bypasses`).

## Build

The front end is built by `build.rs` from an Open Watcom tree: `$OWROOT` when
set, otherwise a tree cached per commit (`toolchain/owshim/ow-commit`) and
bootstrapped once. `--no-default-features --features wccq` builds only the front
end. Without the toolchain feature `llrm-c` still reads `.cgs` streams.

[GNU C builtins](builtins.md): how a `__builtin_*` call compiles.

## Tests

Unit tests are in `crates/frontends/llrm-c/src/`. Run them with `cargo test -p llrm-c --features toolchain`; without it they do not compile. Programs in `tests/run/c`
are compiled, run in DOSBox-X and compared with their `.out` by
`tools/dosbatch/run_tests.py c`; see [testing](../../working/testing.md).
