# Buildprs Microfixtures

Tiny `.prs` grammars with captured DOS `prsstate.asm` goldens used to isolate
graph-backend lowering before full `qbasbnf.prs` parity work.

## Layout

```
micro/<name>/
  grammar.prs
  peropcod.txt
  o0/prsstate.asm   # DOS buildprs default mode
  o1/prsstate.asm   # DOS buildprs -O1
```

## Capture

Maintainers can refresh goldens with DOSBox-X:

```bash
uv run python tools/capture_buildprs_micro.py --level both
```

Equivalent DOS commands (from `qb5/qbas` with `TL=\tl\bin`):

```bat
\tl\bin\buildprs.exe -v < d:\grammar.prs
\tl\bin\buildprs.exe -O1 -v < d:\grammar.prs
```

## Tests

```bash
cargo test -p buildprs graph_backend_o0_matches_plain_microfixtures
cargo test -p buildprs graph_backend_o1_matches_plain_microfixtures
```

Failures print decoded entry-level diffs via `buildprs_micro` helpers.
