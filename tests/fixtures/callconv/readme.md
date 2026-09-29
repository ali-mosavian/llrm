# Calling-convention conformance

llrm against the compilers it links with: BCC 3.1 for C, BC (VBDOS, PDS 7.1,
QB 4.5) for BASIC.

## C (`c/`)

One program per convention: `cf` cdecl far, `cn` cdecl near, `pf` pascal
far, `pn` pascal near. Each has a callee and a caller module for scalars
(`*callee.c`, `*caller.c`) and one pair for structs by value (`*aggee.c`,
`*agger.c`), all generated from the X-macro lists in `cases.h`. A callee
stores what arrived in globals; a caller passes `harness.c`'s inputs and keeps
what came back.

- `bcc/` is BCC's side, rebuilt by `tools/callconv/reference.sh`: `-S`
  listings of the plain modules, and the objects the execution tests link
  (callers built `-DARMED -r-`; `*x.OBJ` is a near callee built into the
  caller's segment).
- `reference.txt` is every fact the listings show, one per line, with the
  listing line it comes from. `testing::boundary` derives it; the test
  `test_the_reference_table_is_what_bcc_listed` keeps it current
  (`LLRM_BLESS=1` rewrites it).
- `known.txt` lists where llrm still differs.

`cargo test --release -p llrm-c --features toolchain callconv` runs the
unit level. `tools/callconv/c.sh` runs the programs: llrm and BCC callers
and callees in all four pairings, a module llrm refuses replaced by BCC's.

## BASIC (`bas/`)

`CE`/`CES` are callees, `CR`/`CRS` callers, `HN` the harness; `CUE`
(CURRENCY) and `CVE` (BYVAL and SEG in a definition) are PDS 7.1's and
VBDOS's only, and QB 4.5 includes the empty `qb45/` copies of their `.BI`
files. `PROBE.ASM` holds the register probe, a CDECL and a pascal asm
callee, and the slots the modules keep values in (llrm-qb refuses COMMON).

- `bc/<dialect>/` is BC's `/A` listings, rebuilt by
  `tools/callconv/basref.sh`; `reference.txt` and `known.txt` as for C.
- `tools/callconv/bas.sh <dialect> <dir>` builds and runs the four
  directions under dosrun (see its header).

`cargo test --release -p llrm-qb callconv` runs the unit level.

## Running them

`cargo test --release --test callconv -- --ignored` runs both matrices
under dosrun (`DOSRUN`, else ~/scratch/pr-dosbox's, else the built one),
about ten seconds. `runs.txt` beside each lists the failures still
expected.
