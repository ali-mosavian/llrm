# QBasic 1.1 buildprs golden artifacts

Audience: maintainers of `crates/buildprs`. Byte-for-byte outputs of the
original QBasic 1.1 parser table generator, the oracle for the Rust
generator's tables.

| Directory | Mode | Oracle for |
|---|---|---|
| `qbasic-1.1/` | `-O1` | the graph backend at `OptLevel::O1` |
| `qbasic-1.1-o0/` | default, no `-O` | the graph backend at `OptLevel::O0` |

## Source inputs

- Grammar: `qb5/ir/qbasbnf.prs` in the QuickBASIC 4.5 source tree
- Generator: `tl/bin/buildprs.exe` in the same tree
- Build rule (`-O1`): `qb/ir/makefile` in the same tree

The artifacts were generated from the original binary under DOSBox-X.
Equivalent DOS commands:

```bat
cd \45\qb5\qbas
set TL=\45\TL\BIN
\45\TL\BIN\buildprs.exe -O1 -v < \45\qb5\ir\qbasbnf.prs
\45\TL\BIN\buildprs.exe -v < \45\qb5\ir\qbasbnf.prs
```

The first writes `qbasic-1.1/`, the second `qbasic-1.1-o0/`.

## Files

Copied without editing: `prstab.inc`, `prstab.h`, `prsirw.inc`, `prsorw.inc`,
`prsstate.asm`, `prsrwt.asm` and `contexts`.

The host generator must match their parser-table bytes and key equates;
`crates/buildprs/src/buildprs_generator.rs` and `buildprs_integrate.rs` include
them (for example `graph_backend_o1_matches_qbasic_11_prsstate_fixture`).
