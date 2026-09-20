# PDS 7.1 compatibility additions

This is the additive PDS 7.1 layer. It inherits the QuickBASIC 4.5 suite at
`../qb45/suite.toml`; inherited source is not copied here. Each automated
program emits exactly one `PASS <case>` or `FAIL <case> <check>` with bare
`PRINT`. The runner captures it with `EXE > RESULT.TXT`; `OPEN "CON"` is
forbidden because the named console device bypasses COMMAND.COM redirection.

The PDS runtime is its own target. This suite does not borrow an ABI, string
descriptor, event entry, or data layout from FreeBASIC or from the QB 4.5
runtime. `parser`, `lowering`, and `runtime` marked `required` mean required
for compatibility even before the current frontend supports that form.

| Level | Case | PDS facility and observable result |
|---:|---|---|
| 1 | `pds-huge-array` | `/Ah` indexed access through a >64 KiB dynamic two-dimensional INTEGER array. |
| 1 | `pds-row-major-array` | `/R` changes physical multidimensional order, witnessed by `VARPTR`/`PEEK`. |
| 2 | `pds-far-string` | `/Fs` preserves both ends of two 30,000-byte dynamic strings. |
| 2 | `pds-currency` | CURRENCY/CCUR arithmetic plus independent MKC$/CVC scaled-byte witnesses. |
| 2 | `pds-runtime-check` | `/D /E /X` produces exact ERR/ERL and PASS only after RESUME. |
| 2 | `pds-local-error` | `ON LOCAL ERROR` checks exact ERR/ERL and caller-side post-RESUME return. |
| 3 | `pds-quick-call` | `/Ot` uses three noncommutative arguments and distinguishes BYREF from two BYVAL formals. |
| 3 | `pds-common-modules` | separately compiled named `COMMON SHARED` and cross-module SUB call. |
| 4 | `pds-timer-event` | `/V /W` TIMER GOSUB dispatch, event return, and PDS polling path. |
| 4 | `pds-com-event` | reference-only COM event and `/C` buffer case. |
| 4 | `pds-key-pen-play-events` | reference-only KEY, PEN, and PLAY event registrations. |
| 5 | `pds-interrupt` | reference-only Interrupt and InterruptX ABI forms. |
| 5 | `pds-uevent` | reference-only UEVENT registration. |
| 4 | `pds-mbf` | `/MBF` exact SINGLE/DOUBLE Microsoft Binary Format bytes plus CVS/CVD round trips. |
| 4 | `pds-fpa` | `/FPa` linked to `BCL71ANR.LIB`, with exact COS/SIN/LOG DOUBLE bytes. |
| 1 | `pds-switch-g3-rejected` | negative compiler probe: `/G3` emits the known ignored-option diagnostic. |

The `/Ah` test uses the known PDS-safe 201 by 201 INTEGER shape rather than
the one-dimensional `REDIM h(40000)` probe. PDS 7.1 reports math overflow for
that latter shape even with `/Ah`, so it cannot establish huge-array access.

## Build and runtime assumptions

The profile is PDS 7.1 BC/LINK in 386+ DOS real mode. `/G2` is selected because
PDS supports it; `/G3` is deliberately absent because PDS rejects it. Every
case records its semantic switches in `suite.toml`; a default compile is not
evidence for `/Ah`, `/Fs`, `/R`, `/D`, `/Ot`, or event polling.

`pds-common-modules` compiles `modules/PDCOMM.BAS` and
`modules/PDCOMW.BAS` separately and links both objects. The manifest
names the entry source; its sibling is part of the same case.

All PDS text fixtures (source, expected output, manifests, inventory, and
documentation) are CRLF-only, and every DOS-visible source/companion/artifact
basename is 8.3. The original LF-only probe gave PDS severe errors.
`pds-timer-event` has a finite 20,000,000-iteration cap as a failure witness,
not a hang; the known PDS `/V /W` baseline reaches one handler hit and value 99.

The inherited QB 4.5 B800/BSAVE case remains a separate display-memory
artifact check. It does not gate or validate redirected runtime verdicts;
those are accepted only from the exact `RESULT.TXT` produced by the current
frontend-linked executable.

The serial/input/audio cases are `reference-only` because they require a
serial peer, keyboard event injection, light pen, or speaker timing. They
still exercise parser/lowering requirements and provide an explicit setup
anchor rather than silently treating a non-event run as success.

## Evidence and remaining scope

[coverage.toml](coverage.toml) is the machine-readable inventory. It pins the
five installed PDS QuickHelp files by SHA-256 and their decoded header topic
counts. Every PDS delta links its QuickHelp context to a granular case and a
source/switch evidence line. `required-reference` cases compile and lower the
PDS spelling but do not claim a console pass without their named DOS service,
hardware, or external library. The inventory also identifies the familiar
features accounted for by inherited QB 4.5 cases rather than copied. It retains
the full extraction bytes for provenance, but removes ISAM and OS/2 coverage
rows and cases entirely. The generated heading inventory excludes those titles
mechanically, so they cannot inflate either coverage or gap totals.

PDS BC's built-in usage text identifies `/Ah` huge dynamic arrays, `/Fs` far
strings, `/V`/`/W` event checks, `/C` COM buffering, `/D` runtime checks, `/Ot`
quick calls, and `/R` row-major arrays. It reports that `ON EVENT` needs `/V`
or `/W`. The independent PDS TIMER witness establishes `HITS=1`, `VALUE=99`,
and normal completion for the same finite polling shape.

The `/MBF` case is now an ordinary required runtime obligation rather than a
reference fixture. PDS 7.1 BC plus `BCL71ENR.LIB` measured `1.5` as
`00 00 40 81` for MKS$ and, for the DATA-decoded DOUBLE, as
`F8 FF FF FF FF FF 3F 81` for MKD$; the source checks those bytes before the
inverse conversions. These Microsoft measurements establish the oracle only.

The `/FPa` case is likewise executable now, but only with the manifest's
`BCL71ANR.LIB`; linking the same object against `BCL71ENR.LIB` was measured to
terminate with `Error during run-time initialization`. The three COS/SIN/LOG
results are checked as exact MKD$ bytes so an ordinary/emulator runtime cannot
satisfy the alternate-math obligation accidentally.

These Microsoft compiler/runtime measurements establish oracle behavior only.
A suite PASS requires the qb frontend's emitted object, linked to the PDS
runtime; original BC acceptance cannot satisfy that gate.

Unresolved PDS scope remains intentionally visible: QBX IDE/QuickLib behavior,
browse and CodeView metadata switches, EMS policies, UI/chart/font toolbox
libraries, and device-dependent event delivery. ISAM and OS/2 are explicitly
excluded and have no manifest cases or coverage rows. The remaining facilities need compiler
diagnostic or controlled hardware probes before they can become ordinary
required console cases; they must not be filled in with a modern BASIC ABI
assumption.
