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
  record, a BYREF parameter's pointer wrapper). `$$TYPES` has no per-entry
  length the way `$$SYMBOLS` does, so walking it needs the full CV4 leaf
  grammar. That grammar *is* now decoded -- LF_POINTER, LF_ARGLIST,
  LF_PROCEDURE, LF_FIELDLIST, LF_MEMBER, LF_STRUCTURE all confirmed against a
  CVPACK'd build (e.g. `TYPE Coord / x AS LONG / y AS LONG / END TYPE` comes
  back as `LF_STRUCTURE{count=2, size=8, name="Coord"}` over an
  `LF_FIELDLIST` of two `LF_MEMBER`s at offsets 0 and 4) -- but that decode
  hasn't been carried back onto the raw pre-link bytes. `cvinfo.py` reports
  these as `custom (type 0xNNNN, unresolved)`.

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
