# QBasic 1.1 `buildprs` Golden Artifacts

This directory is reserved for byte-for-byte outputs from the original
QBasic 1.1 parser table generator.

## Source Inputs

- Grammar: `qb5/ir/qbasbnf.prs` in the QuickBASIC 4.5 source tree
- Generator: `tl/bin/buildprs.exe` in the same tree
- Build rule: `qb/ir/makefile` in the same tree

The checked-in artifacts in this directory were generated from the original
binary under DOSBox-X on 2026-06-12. Equivalent DOS commands:

```bat
cd \45\qb5\qbas
set TL=\45\TL\BIN
\45\TL\BIN\buildprs.exe -O1 -v < \45\qb5\ir\qbasbnf.prs
```

## Expected Outputs

The following generated files are copied here without editing:

- `prstab.inc`
- `prstab.h`
- `prsirw.inc`
- `prsorw.inc`
- `prsstate.asm`
- `prsrwt.asm`
- `contexts`

These files are the oracle for the Rust `buildprs` reimplementation. The host
generator must match their parser-table bytes and key equates before
`src/parser_tables.rs` is wired to generated Rust data.
