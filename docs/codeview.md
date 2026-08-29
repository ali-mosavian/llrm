# CodeView debug info (`/Zi`)

`/Zi` doesn't add new OMF structure. Every BC object already declares two
empty debug segments (class `DEBSYM`/`DEBTYP`); `/Zi` is what makes BC put
bytes in them, plus `LINNUM` records the compiler otherwise omits.

    qbopt/cvinfo.py       reads $$SYMBOLS directly from the .OBJ, no LINK/CVPACK
    python -m qbopt.cvinfo FILE.OBJ

Verified identical across all three compilers (VBDOS, PDS 7.1, QB 4.5) by
compiling probe programs with `/Zi`, linking with `/CODEVIEW`, running CVPACK,
and diffing the raw pre-link bytes against CVPACK's fully-documented CV4
output. Ground truth: [TIS's "Microsoft Symbol and Type
Information"](http://openwatcom.org/ftp/devel/docs/CodeView.pdf) plus Open
Watcom's own compiler backend (`bld/watcom/h/cv4*.h`), which still emits and
reads this exact format.

## $$SYMBOLS: the raw, pre-link record format

Not CV4 -- CVPACK's job is partly to translate this into the format the PDF
documents. Every record here is a byte shorter per field throughout, and
lexical-scope linkage (`pParent`/`pEnd`/`pNext`) is absent because CVPACK is
what fills it in.

    [len:u8][kind:u8][data, len-1 bytes]

| kind | meaning | data |
|---|---|---|
| `0x00` | module scope open | shape varies by compiler; QB 4.5's carries no filename at all, so `cvinfo.py` reads the module name from `THEADR` instead |
| `0x01` | SUB/FUNCTION start | `offset:u16, ?:u16, proc_length:u16, debug_start:u16, debug_end:u16, ?:u16, flags:u8, name` |
| `0x02` | end of scope | (no data) |
| `0x04` | BP-relative param/local | `bp_offset:i16, type_index:u16, name` |
| `0x05` | module-level DIM | `offset:u16, segment:u16, type_index:u16, name` |
| `0x0b` | line label (`GOTO` target) | `offset:u16, ?:u8, name` |

`offset`/`segment` on a `0x05` record are legitimately `0` pre-link -- BASIC's
module data isn't at a static address the way a C static is, so BC leaves
that for the linker. Not a parser gap.

Two `0x01` fields are unidentified (marked `?` above): tried and failed to
correlate them with anything in the CVPACK'd output, including a procedure's
`proctype`. A FUNCTION's return type is *not* recoverable from its own `0x01`
record.

## $$TYPES: the raw, pre-link type table

Not CV4 either, and not a smaller version of it the way `$$SYMBOLS` is --
confirmed independently rather than assumed, by compiling a dozen probe
programs (arrays of a primitive and of a `TYPE`, 1-D and 2-D, structures of
one/three/differently-named fields, BYREF `LONG`/`INTEGER`/`STRING`
parameters) on all three compilers and differencing the raw bytes as each
probe changed by one thing at a time. Every record shares one framing:

    [kind:u8][length:u16][data, length bytes]

`kind` is `0x01` in every record measured; nothing here exercises another
value, so `cvinfo.py` reports one as `Unresolved` rather than guessing what
it would mean. Records are walked in file order with no gaps and no trailing
pad, indices starting at `0x0200` and counting up one per record -- the
first record measured is always index `0x0200` and reads the same 1-byte
`data` (`0x80`) regardless of source, so it looks like fixed per-module
boilerplate rather than anything source-dependent.

`data`'s own first byte (or two) is BC's private tag -- confirmed to be
unrelated to CVPACK's CV4 leaf numbers, not merely un-cross-checked against
them:

| tag | meaning | data |
|---|---|---|
| `0x8c` | array | `0x83, element:u16` -- the element's type_index, nothing else |
| `0x79 0x86` | structure | `size_bits:u32, 0x85, count:u16, 0x83, field_types:u16, 0x83, field_names:u16, 0x82, namelen:u8, name, trailer:u8` |
| `0x7f` | a flat list | either `(0x83, type_index:u16)*` (a structure's field types, or an argument list) or `(0x82, namelen:u8, name, 0x85, offset:u16)*` (a structure's field names+offsets), told apart by which tag the list's own first entry uses |
| `0x76` | BYREF wrapper | `0x83, target:u16` -- target is always another record with tag `0x7a` in every case measured |
| `0x7a 0x74` | pointer | `0x83, target:u16` |
| `0x75 0x80` | a procedure's own signature | `0x83, rvtype:u16, 0x73, nparms:u8, 0x83, arglist:u16` -- likely what a `0x01` `$$SYMBOLS` record's still-unidentified second field points at, but that link is not made here; nothing in `cvinfo.py` reads a `0x01` record's own type index today |

**An array's record carries only its element type, never bounds.** A 1-D
`DIM x(N) AS LONG` and a 2-D `DIM x(N, M) AS LONG` produce the byte-identical
11-byte `$$TYPES` segment (`suite/arrays.bas` and a throwaway 2-D probe) --
so BASIC's own array bounds live in the runtime's array descriptor, not in
debug type info, and `Array` here has nowhere to put a bound because BC never
writes one. Confirmed for both a primitive element and a `TYPE` element
(`suite/udt.bas`'s `pts`, an array of `Coord`) -- the same `0x8c` shape either
way, just pointing at a different type_index.

**A structure lists its own field types and its field names+offsets as two
separate, positionally-parallel records**, referenced by index from the
structure's own record and zipped back together by `cvinfo.py`'s `Struct`.
Confirmed against one field, three fields, and two structures with 3- and
5-character names in the same module (to rule out the trailing byte after the
name being alignment padding tied to name-length parity -- it is not; it is
present, and always `0x69`, regardless of whether the record's own length is
even or odd). What that trailing byte means is not resolved; `cvinfo.py`
reads the name using only the declared `namelen` and otherwise ignores it.

**A BYREF parameter is two hops on VBDOS and PDS**: the `$$SYMBOLS` record's
type_index names a `0x76` record, which names a `0x7a` (pointer) record,
which finally names the parameter's real type (`suite/procs.bas`'s
`Twice&`/`Report`, `LONG` and `STRING` respectively). `cvinfo.py` walks both
hops and reports e.g. `BYREF LONG`; the two tags are not exposed as separate
concepts since BASIC has no syntax for a bare pointer to tell them apart with.

**QuickBASIC 4.5 does not build this chain at all.** The same BYREF `LONG`
parameter gets type_index `0xa2` directly off the `$$SYMBOLS` record --
never touching `$$TYPES` -- and BYREF `INTEGER`/`STRING` get `0xa1`/`0xb7`.
Each is exactly its PRIMITIVES byte plus `0x20` (`0x81`+`0x20`, `0x82`+`0x20`,
`0x97`+`0x20`); measured for those three, not extended to `SINGLE`/`DOUBLE`/
`STRING`(far), which were not compiled through this probe.

## Types

`type_index` on a `0x04`/`0x05` record is either:

- **A primitive** -- one of six compiler-independent codes, decoded by the
  same diff-against-CVPACK method:

  | code | type |
  |---|---|
  | `0x81` | INTEGER |
  | `0x82` | LONG |
  | `0x88` | SINGLE |
  | `0x89` | DOUBLE |
  | `0x97` | STRING (near) |
  | `0x9c` | STRING (far) |

  Which STRING code a compiler picks tracks its near/far string memory model,
  not anything in the source.

- **Custom** -- a module-local index into `$$TYPES` (an array, a `TYPE`
  record, a BYREF parameter's pointer wrapper), decoded in the `$$TYPES`
  section below. QuickBASIC 4.5 additionally has its own small family of
  BYREF-parameter codes that never touch `$$TYPES` at all -- also below.

**A FUNCTION's return type** comes from BASIC's own type-suffix sigil on the
function's name (`%`/`&`/`!`/`#`/`$`), read directly off the name already in
the `0x01` record -- not reconstructed from `$$TYPES`. Reliable when the
sigil is there; PDS 7.1 and QB 4.5 keep it in the debug name, VBDOS drops it
unconditionally -- `Twice&` (`suite/procs.bas`, explicitly `LONG`, not just
the `DEFINT` default) still shows up as plain `Twice` under VBDOS, so this
isn't about redundancy with a default, VBDOS just never puts a sigil on a
procedure's debug name.

**QB 4.5 under `ON ERROR GOTO`/`RESUME` emits a label per resumable
statement.** `suite/divmod.bas` has exactly one source label (`handler:`);
VBDOS and PDS 7.1 report exactly two label records (`_0` and `handler`), but
QB 4.5 reports 33 -- one compiler-generated `_0` at nearly every statement
boundary, presumably so `RESUME NEXT` has something to jump to. Confirmed
against the raw bytes, not a parser artifact: each is a distinct, real `0x0b`
record with a genuine (if duplicated) offset.

**CONST has no debug record at all**, on any of the three compilers --
checked empirically (a probe with `CONST LIMIT = 10` produces zero symbol
records for it). It's a compile-time substitution with no storage, so there
is nothing in the object to extract. Not a limitation of `cvinfo.py`.

## Contrast: the packed CV4 form

Linking with `/CODEVIEW` (LINK invokes CVPACK itself when VBDOS's LINK 5.31
sees it; PDS 7.1's LINK 5.10 needs `CVPACK.EXE /P` run separately -- and
stays on the older `NB02` signature even packed, unlike VBDOS's `NB08`) turns
this into the documented format: `NBxx` trailer, a subsection directory
(`sstModule`, `sstAlignSym`, `sstSrcModule`, `sstGlobalTypes`, ...), real
`S_LPROC16`/`S_BPREL16`/`S_LDATA16` records with full lexical-scope linkage,
and a `sstGlobalTypes` table using the real numbered LF_ leaves. QB 4.5
doesn't ship CVPACK, so this path only exists for VBDOS and PDS.
