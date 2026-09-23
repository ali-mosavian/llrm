# VBDOS additive compatibility suite

This is the VBDOS 1.0 delta only. It inherits all QB 4.5 and PDS 7.1 cases
through `../pds71/suite.toml`; do not duplicate an inherited feature here.
Each executable case writes its sole verdict with bare `PRINT`, and the runner
captures it with `EXE > RESULT.TXT`. `OPEN "CON"` is forbidden because the
named console device bypasses COMMAND.COM redirection. A successful run ends
in exactly `PASS <case>`; an
executable failure prints `FAIL <case> <check>` and stops.

## Levels and inventory

| Level | Case | VBDOS delta or integration shape | Current requirement |
|---|---|---|---|
| 1 | `underscore_identifiers` | VBDOS permits `_` in identifiers; QB 4.5 and PDS 7.1 reject it. | parser, lowering, runtime |
| 2 | `g3_byval_long` | `/G3` uses the VBDOS 386 long-argument ABI; a `BYVAL LONG` call exercises its source-level boundary. | parser, lowering, runtime |
| 2 | `g2_byval_long` | VBDOS also accepts `/G2`; this keeps its two-word ABI separate from `/G3`. | parser, lowering, runtime |
| 4 | `segmented_memory` | `DEF SEG`, `VARSEG`, `VARPTR`, and `POKE` write through a selected segment. The variable name also contains `_`. | parser, lowering, runtime |
| 4 | `local_error` | Procedure-local error trap with exact ERR/ERL, numbered RESUME target, and caller-side recovery witness. It uses `/X`. | parser, lowering, runtime |
| 4 | `string_far_address` | `SSEG`, `SADD`, and `SSEGADD`, independently witnessed through physical PEEK bytes and a null-string boundary. | parser, lowering, runtime |
| 4 | `interrupt_abi` | The supplied `VBDOS.BI` `RegType` layout and `INTERRUPT` far routine. It asks DOS interrupt 21h/function 30h for a nonzero major version, but intentionally does not print the version. | parser, lowering, runtime |
| 4 | `vga_framebuffer_bsave` | Defined mode-13 VGA segment `A000h`, direct pixels, and an unmeasured BSAVE source surface. | parser/lowering; runtime pending |
| 4 | `interruptx_abi_surface`, `absolute_abi_surface`, `legacy_interrupt_abi_surface` | Remaining routines declared by the installed `VBDOS.BI`: segmented-register, absolute-address, and legacy-array interrupt forms. | parser/lowering; reference runtime |
| 4 | `cdecl_alias_external` | An external `CDECL ALIAS` declaration with a `BYVAL LONG` argument. | parser/lowering; reference runtime |
| 4 | `form_event_surface`, `timer_event_surface`, `custom_control_surface` | Form event/property/method, Timer control event, and custom-control callback surfaces. | parser; form lowering pending; reference runtime |
| 5 | `qrender_shape` | A small executable synthesis of qb-qrender's underscore names, `$STATIC`/`$DYNAMIC`, fixed UDT strings, line continuation, `BYREF` UDT, and `BYVAL` scalar patterns. | parser, lowering, runtime |

The installed-help completion adds deliberately separate cases rather than
folding distinct form and object contracts into a single umbrella.

| Level | Case | VBDOS delta | Current requirement |
|---|---|---|---|
| 1 | `option_explicit_negative` | `OPTION EXPLICIT` rejects an undeclared assignment. | parser negative acceptance |
| 1 | `continuation_terminal_probe` | Original BC accepts a terminal bare underscore; this preserves the observed parser behavior even though `README.TXT` describes a different editor rule. | parser reference |
| 2 | `date_serial_surface` | VBDOS unsuffixed date/time serial APIs and date-part functions. | parser, lowering, runtime |
| 4 | `dialog_surface` | `MSGBOX` statement/function and `INPUTBOX$`. | parser; interactive reference runtime |
| 4 | `form_lifecycle_surface` | Load, resize, paint, mouse, and unload event procedure shapes. | parser; generated-form reference runtime |
| 4 | `object_runtime_surface` | `CONTROL`, `IF TYPEOF`, `LOAD`, `UNLOAD`, `DOEVENTS`, and object methods. | parser; generated-project reference runtime |
| 4 | `financial-functions` | All 13 `FINANCE.LIB` functions, including double `BYVAL`, status outputs, and array arguments. | parser, lowering, matching-library runtime |

`coverage.toml` is the machine-readable topic-to-case inventory. It pins all
three requested HLP files by SHA-256 and names exact heading ranges for the
four databases concatenated in `VBDOS.HLP`. The qck, advisor, and error ranges
are classification inputs; an unmatched heading is emitted as an explicit
gap. Worked examples, product-support navigation, and host-help navigation are
hashed oracle/reference prose rather than duplicate language features. ISAM
and OS/2 cases and coverage rows are deleted; matching FULL-extraction headings
are excluded mechanically and do not count as tests or gaps.
`evidence.md` records the extraction command, installed sample/project forms,
and bounded original-BC acceptance probes.

`REFERENCE-ONLY` sidecars are deliberately **not** captured program output:
they state that the case cannot be run by the ordinary standalone harness yet.
The runner must report that state, never treat it as a passing `PASS` result.

## Runtime and toolchain assumptions

- Required executable cases must compile through the qb frontend, emit OMF,
  and link that object against the matching VBDOS runtime. VBDOS `BC.EXE`
  supplies one-time oracle/provenance evidence only. The default ABI-bearing configuration is
  `/O /FPi /R /G3 /E`; this is the locally documented VBDOS shipping shape.
- `interrupt_abi` additionally requires the bundled `VBDOS.LIB` routine and a
  DOS-compatible guest exposing interrupt 21h/function 30h with a nonzero
  major version. It is not a host syscall test and it must not substitute a
  FreeBASIC ABI.
- `cdecl_alias_external` needs a separately built 16-bit real-mode
  `CDEXT.OBJ` exporting `compat_external` under VBDOS CDECL and
  returning its `LONG` argument plus one. No FreeBASIC object, calling
  convention, or runtime is an acceptable substitute.
- `financial-functions` links `VBDCL10E.LIB` plus `FINANCE.LIB`. It requires
  status zero and a separately measured MKD$ byte result for every entry;
  IRR, MIRR, and NPV additionally cross the DOUBLE-array ABI.
- `form_event_surface` requires a generated VBDOS `FORMEVT.FRM` defining
  `frmCompat` and `cmdRun`; that generated form format is not guessed or
  reconstructed here. It remains reference-only until project/form loading
  is implemented.

## Evidence and explicit unknowns

Evidence is local and intentionally narrow: `docs/frontend/qb/dialects.md`
records the underscore distinction and `/G3`; `docs/frontend/qb/parser-provenance.md`
records VBDOS as the full-program bring-up profile; `frontends/qb/src/intrinsics.rs`
identifies currently catalogued pointer intrinsics; and the installed VBDOS
tree supplies `INC/VBDOS.BI`, `INC/CUSTCALL.BAS`, and form examples.
qb-qrender supplies the source shapes in `src/{ent,model,r_bsp,
d_surf,sys}.bas`.

Original-compiler acceptance is now recorded for underscore identifiers,
the `OPTION EXPLICIT` negative case, terminal underscore parsing, every
financial declaration/call shape, and the date/time surface. The original
compiler consumes CRLF physical lines; a
runner must convert checked-out source before calling BC, or its diagnostics
describe one accidentally concatenated source line rather than the test.

Unknowns are kept visible rather than encoded as a green test:

- Form binding, custom-control registration, and `CDECL ALIAS` linking remain
  dependent on original project/object artifacts; source acceptance is not a
  substitute for those ABI/runtime contracts.
- The `/G3` source case establishes behavior and requests `/G3`; an object
  inspection check still needs to assert the expected single-dword argument
  push, not infer it from output.
- No graphics framebuffer golden is included, so `VBGFX.BIN` is not a declared
  artifact and the case is runtime-pending. Add a byte-level golden only after
  measuring the original toolchain. The source already pins mode 13, segment `A000h`,
  offsets 0--15, and the 16-byte save length; it restores both `DEF SEG` and
  screen mode. Monochrome `B000h` is deliberately not tested without a
  monochrome device profile.
