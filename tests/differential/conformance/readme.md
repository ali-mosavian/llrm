# Microsoft BASIC conformance cases

For contributors: the QB 4.5, PDS 7.1 and VBDOS 1.0 cases `llrm-qb` is held to, and the format a case takes.

The suites are additive:

```text
QB 4.5 (base) <- PDS 7.1 additions <- VBDOS 1.0 additions
```

A construct is not copied into each directory. Each `suite.toml` records a case's source, level, feature tags, the original compiler's switches, the expected console output, linker libraries, DOS arguments and environment, and its `parser`, `lowering` and `runtime` obligations. `coverage.toml` maps the installed Microsoft help topics to cases or to a stated reason they have none.

## Running

    python3 tools/dosbatch/run_conformance.py [qb45|pds71|vbdos]... [--list-skipped]
    cargo test --release --test differential

Each case compiles with `llrm-qb` (`--dialect` and `--runtime` of the suite, flags from the case's BC switches), links with the dialect's own runtime library and runs in DOSBox; stdout must equal the case's `expected`. A case runs only if its `runtime` is `required` and it needs one source, no artifact, companion, compiler probe, expected diagnostic or environment table; `--list-skipped` names the rest and why. `known.toml` maps a failing case to its issue: a known case that passes fails the run.

## Case format

Every deterministic executable writes its verdict with bare `PRINT`, and the runner captures `EXE > RESULT.TXT`. `OPEN "CON"` is forbidden: the named console device bypasses COMMAND.COM redirection.

All DOS-side sources, companions and expected-output files have 8.3 basenames and CRLF-only text.

`runtime` is `required` (an obligation the runner checks), `pending` (the expectation or integration still needs completing) or `reference-only` (hardware, UI, external-object or service-dependent behavior no headless run can judge).

Microsoft's compiler and runtime are the authority for what a case prints; FreeBASIC, QB64 and host BASIC runtimes are not.
