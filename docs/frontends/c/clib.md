# Building Open Watcom's C library

Audience: contributors. What it takes to build Open Watcom's clib (16-bit, medium model, `msdos.086/mm`) with `llrm-c`, how the build is checked, and where it stands. Tools: [`tools/clib`](../../../tools/clib).

The build is 386-class real mode: `x86-m16` generates 386 code only ([targets](../../machine/targets.md)), so the objects are not the 8086 library's and sizes are not comparable one to one with `msdos.086`. The pinned source tree is the library's; the reference is the pinned tree's own `bwcc` and `bwasm` with the makefiles' flags and headers generated from the same tree by its `wsplice`.

## Pipeline

| Step | Tool | Does |
|---|---|---|
| objects | `inventory.py` | The makefiles' own object list (`objects.mif` tags, `deps.mif` sources, per-object `_opts`), `flags.mif`'s switches, the alias stubs from `aliasgen`. C by `llrm-c -m16 -mabi=watcom -mcode-segment=_TEXT -fsigned-char`, assembly by JWasm; or by the reference tools (`--reference`) |
| headers | `headers.py` | DOS headers from `bld/hdr`: the installed ones are a release older than the sources |
| library | `library.py` | `wlib` archive; objects the makefiles build `-zld` stay out as stand-alone files; `manifest.json` says which tool made each object |
| compare | `compare.py`, `report.py` | Publics, code and data bytes, compile time, against the reference |
| run | `qa.py` | `bld/clibtest`'s 18 programs built both ways, run one per directory in DOSBox-X ([dosbatch](../../../tools/dosbatch/dosbatch.py)), output and exit code diffed |
| find | `swap.py` | Relinks a program once per library module swapped for the reference's: the module a difference lies in |

## Where it stands

| | |
|---|---|
| C sources | 978: `llrm-c` compiles 959, the reference 978 |
| assembly | 246 (75 files, 171 alias stubs): JWasm assembles all; code and data are byte-identical to `bwasm`'s in all 246 |
| publics | identical to the reference's in every object both build |
| code, `-O2` | 142,529 B against 118,370 (1.20x); `-Os` 132,076 B (1.12x); per object `-Os`: 238 smaller, 140 equal, 556 larger |
| data | 14,011 B (`-Os`) against 13,130 |
| compile time | 11.0 s for 959 objects against 1.4 s for 978; worst `streamio/scnf_s` 0.35 s against 0.011 s |
| `clibtest` | 13 of 18 print the same and exit the same; `direct`, `search`, `string` pass where the reference's 2020 library fails the 2026 tests; `bios` links in neither; `heap` is refused (below) |

The library the programs link is **mixed**: 957 objects by `llrm-c`, 246 by JWasm, and 19 C objects from the reference for what `llrm-c` still refuses. `-Os` is `-os` on the makefiles' objects that ask for it; the makefiles' general flags (`-oabelr -om`) ask for no size.

## What `llrm-c` refuses

An inline `#pragma aux` block or a register list it cannot honour is refused, not guessed at.

| Cause | Objects |
|---|---|
| `alloca` (inline code that changes SP) | 8 |
| a register convention of its own (`_DoINTR`, `byteswap`, `__raise`, `default_handler`, `__do_exit_with_msg`) | 7 |
| inline code taking a parameter on the stack | 2 |
| a `long long` in `dx:cx:bx:ax` | 2 |

## Gaps the run found

Each was a silent wrong program, found by running, and has a fail-first test in `llrm-c`.

| Gap | Showed as |
|---|---|
| `#pragma aux` inline code dropped its register parameters | `cgets` ran DOS function 0Ah on whatever DX held |
| a register inline code changes was not saved | `_fstrrev` corrupted its caller's SI |
| a register pair's segment taken from the bit order | `_fmemmove` copied from the wrong place; the front end picks it by register ([hwreg](../../../crates/frontends/llrm-c/src/hwreg.rs)) |
| a number cast to a far pointer got DS | `p == NULL` false, `signal` answered the wrong handler |
| a static in a `__segname` segment was dead | the init tables were empty |
| `XI`/`XIE` markers not declared with `XI` | the start-up's range was empty, `argv` never built |
| code pointer constants 2 bytes in far-code data | the signal table was half size |
| a plain char signed in the Open Watcom family | `'\x81'` compared unequal to an unsigned char |
| a start-up not asked for (`_cstart_`, `__argc`) | `main` ran without it; `argc` was 0 |
| near calls between modules with segments of their own | `-mcode-segment=_TEXT` (Open Watcom's `-nt=`) |

Smaller: the front end predefined the host's OS (`-bt=dos` now), the entry is `main_` in registers, a pragma's name pattern (`"*"`, `"_*"`) was replaced by `name_`, the address of an extern function in code declared nothing.

## The forks

JWasm and JWlink are the pinned forks of the toolchain; the clib needed three changes there, with tests in the forks.

- JWasm: `PUBLIC "C",name` and `EXTERNDEF "C",name` (Wasm's mangler, `name_` / `_name`); a quoted or backticked actual in `"&p"` or `` `p` `` loses its quotes, as in Wasm.
- JWlink: a library name placed by `wlib` from the bucket a full dictionary block ended at is found (clibm.lib's `__doserror_` was undefined).

## Inline divides

A 64-bit divide by a variable is the helper's code laid down at every site, whatever the level: no cost model, and no pairing of `/` and `%` on the same operands, so `lldiv` carries two copies (385 B against 92 B for a call).
