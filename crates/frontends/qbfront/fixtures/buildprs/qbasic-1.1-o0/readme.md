# QBasic 1.1 `buildprs` Default Golden Artifacts

This directory contains byte-for-byte outputs from the original QBasic 1.1
parser table generator in its default mode, with no `-O` option.

## Source Inputs

- Grammar: `qb5/ir/qbasbnf.prs` in the QuickBASIC 4.5 source tree
- Generator: `tl/bin/buildprs.exe` in the same tree

The artifacts in this directory were generated under DOSBox-X through the
DOSBox debug MCP on 2026-06-13. Equivalent DOS commands:

```bat
cd \45\qb5\qbas
set TL=\45\TL\BIN
\45\TL\BIN\buildprs.exe -v < \45\qb5\ir\qbasbnf.prs
```

These files are the oracle for the Rust graph backend's `OptLevel::O0` mode.
