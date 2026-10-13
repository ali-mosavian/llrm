# Buildprs microfixtures

Audience: maintainers of `crates/buildprs`. Tiny `.prs` grammars with captured
DOS `prsstate.asm` goldens, to isolate one construct of the graph backend at a
time.

```
micro/<name>/
  grammar.prs
  peropcod.txt
  o0/prsstate.asm   # DOS buildprs, default mode
  o1/prsstate.asm   # DOS buildprs -O1
  o2/prsstate.asm   # DOS buildprs -O2 (absent where DOS buildprs hangs)
```

Goldens are captured by running the original generator under DOSBox-X from
`qb5/qbas` with `TL=\tl\bin`:

```bat
\tl\bin\buildprs.exe -v < d:\grammar.prs
\tl\bin\buildprs.exe -O1 -v < d:\grammar.prs
\tl\bin\buildprs.exe -O2 -v < d:\grammar.prs
```

The sources of the plain fixtures are embedded in
`crates/buildprs/src/buildprs_micro.rs` (`ensure_plain_micro_fixture_sources`
writes them).

## Tests

```bash
cargo test --manifest-path crates/frontends/qbfront/crates/buildprs/Cargo.toml micro
```

The tests are in `buildprs_micro.rs`: every fixture has its checked-in goldens,
and the graph backend is semantically equivalent to them at each level.
Failures print decoded entry-level diffs.
