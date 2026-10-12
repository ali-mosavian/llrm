# Microsoft BASIC conformance cases

For contributors: the QB 4.5, PDS 7.1 and VBDOS 1.0 cases `llrm-qb` is held to, and the format a case takes.

The suites are additive:

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
flowchart LR
  classDef src fill:#2563eb,stroke:#1e40af,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef opt fill:#ea580c,stroke:#9a3412,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef out fill:#16a34a,stroke:#166534,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  linkStyle default stroke:#64748b,stroke-width:2.2px
  qb["<b>QB 4.5 (base)</b>"]:::src
  pds["<b>PDS 7.1 additions</b>"]:::opt
  vb["<b>VBDOS 1.0 additions</b>"]:::out
  qb --> pds
  pds --> vb
```

*The suites are additive*

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
