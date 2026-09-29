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
unit level.
