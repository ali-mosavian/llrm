# VBDOS evidence and probe record

This suite uses the installed VBDOS 1.00 toolchain only.  No FreeBASIC
object, ABI, or runtime is evidence for any case here.

## Help extraction

`/Users/alim/work/personal/qb-qrender/tools/hlpextract.py` decoded the three
installed LN help files. Their SHA-256 values are in `coverage.toml`.
`VBDOS.HLP` is a concatenation of four LN databases, found by the four `LN\x02\0`
headers at byte offsets 0, 421532, 700367, and 821723:

| Embedded database | Topics | Coverage treatment |
| --- | ---: | --- |
| qck | 622 | Classification input; unmatched non-ISAM/non-OS/2 headings are generated gaps. |
| advr | 329 | Classification input because narrative help may define executable semantics. |
| ener | 389 | Classification input because runtime errors are observable contracts. |
| ex | 243 | Worked examples duplicate qck/advisor contracts; retained as oracle prose, not separate features. |

`VBDPSS.HLP` has 197 support headings and `ADVISOR.HLP` has 41 decoded host-help headings.
They are pinned and explicitly excluded in `coverage.toml`; neither is treated
as a language specification.

The predecessor check used the installed PDS 7.1 `HELP/BAS7QCK.HLP`
(`43503f8c91c7eb436de759b1f4ef78951ff06cc43d212feb1eb39a22e11552e3`).
It already documents `DateSerial#`, `DateValue#`, `TimeSerial#`, CDECL,
ALIAS, BYVAL, and INTERRUPT[X]. VBDOS entries are retained only
where spelling/type changed or VBDOS adds a project, object, toolkit, or ABI
contract; the inherited suite remains authoritative for PDS behavior.

## Installed source and binary evidence

| Evidence | SHA-256 | What it pins |
| --- | --- | --- |
| `INC/VBDOS.BI` | `5edd3dd0cc46a4baa3cdffa3e77ca3fb58ba3bfc59f084742675e9c3afd4b4d6` | `RegType`, `RegTypeX`, ABSOLUTE, INTERRUPT[X], legacy INT86 calls. |
| `INC/FINANCE.BI` | `0010ef5c52bac5480dc02a52a5f8079e6c25ac50f84a0664d18f6de29c00beaa` | All 13 financial toolkit declarations and their `BYVAL DOUBLE`/array shapes. |
| `INC/CLOCK.MAK` | `d47e7fce92582bad0e084a47eec22e3ebd635786ccfc7c7a2d5d7812cf9aa244` | Timer-form project shape. |
| `INC/CLOCK.FRM` | `97f75993a7558bfb0dbcb7aa8908731a5537ea6542e36b5ee61a30e32b16d3b4` | Original binary VBDOS form, not a guessed serialization. |
| `INC/CUSTCALL.BAS` | `0e47ef7551384b2c1c87db5b2cdd7032cf1ef3d926d636a0fd78bd39e9d0b756` | `SetProperty`, `GetProperty`, `InvokeEvent`, and `InvokeMethod` callback argument order. |

`BIN/PACKING.TXT` identifies `FINANCE.LIB`, `FINANCE.QLB`, `NOFORMS.OBJ`, and
the custom-control kit. Any future external CDECL probe
  must supply an 8.3 `CDEXT.OBJ`. `BIN/README.TXT` is the source of
the space-or-tab-before-continuation rule and of the underscore identifier
distinction.

## Bounded original-compiler probes

On 2026-09-19, VBDOS `BIN/BC.EXE` was run in DOSBox-X against CRLF-normalized
copies with `/O /FPi /R /G3 /E`. The first run against LF-only copies was
discarded: BC treats it as one physical line, so its results were an instrument
failure rather than compiler evidence. This is why any eventual runner must
write DOS CRLF source before invoking BC.

| Source | Result | Observation |
| --- | --- | --- |
| `underscore_identifiers` | accepted | Confirms underscore identifiers and `OPTION EXPLICIT` under VBDOS BC. |
| `option_explicit_negative` | rejected | One severe error: `undeclaredValue` is not declared. |
| `continuation_terminal_probe` | accepted | BC accepts a bare terminal underscore at end of file. This contradicts the README wording, so the parser-only probe preserves the compiler fact rather than claiming rejection. |
| `financial-functions` | accepted and run | Linked with `VBDCL10E.LIB+FINANCE.LIB`; all 13 calls returned status zero and their measured MKD$ byte anchors before exact redirected `PASS financial`. |
| `date_serial_surface` | accepted | The first version used names colliding with VBDOS's case-insensitive `DATEVALUE`/`TIMEVALUE` keywords; renamed to `serialDate`/`serialTime` and then compiled/linked cleanly. |

The renamed `date_serial_surface` subsequently compiled with zero severe
errors and linked with `VBDCL10E.LIB`. Its first DOSBox batch exposed a broken
instrument: `OPEN "CON"` bypassed `> DR5.TXT` and left a zero-byte result.
That verdict convention was removed corpus-wide. Required cases now use bare
`PRINT`, and the runner must accept only the exact redirected result from the
current frontend-linked executable.

A second bounded sweep on 2026-09-19 compiled all 22 retained manifest cases
with their declared BC switches (and no substitute host object or library).
The 17 positive standalone sources reported zero severe errors, including both
`/G2` and `/G3` calls, the repaired `ON LOCAL ERROR` case under `/X`, direct
VGA `POKE`/`PEEK` plus `BSAVE`. The sole negative case,
`option_explicit_negative`, reported exactly one severe error for its
undeclared assignment. The four form/object reference sources still
report unresolved generated form names when compiled standalone; that is the
expected missing `.FRM`/project precondition, not runtime success.  The sweep
used the checked-in CRLF, DOS-8.3 source names; a prior invocation lacking BC's
trailing semicolon was discarded because it waited for the linker prompt.

## Deliberate gaps

- Form and object cases require a genuine `.FRM` plus project metadata. The
  binary form files above are references, not fake text forms.  The suite has
  source-level cases but does not claim standalone execution.
- Dialog cases block for keyboard input; they remain source/runtime-reference
  coverage until the DOS runner can drive a dialog deterministically.
- Financial execution requires VBDOS's supplied `FINANCE.LIB` and compatible
  floating-point runtime. Its actual declarations are covered; no host math
  library substitutes for it.
- `/G3` output still needs an OBJ-level assertion for the single-dword argument
  push. The source probes do not infer ABI layout from the final answer.
