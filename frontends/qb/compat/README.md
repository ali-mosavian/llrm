# Microsoft BASIC compatibility ladder

The compatibility corpus is additive:

```text
QB 4.5 (base) <- PDS 7.1 additions <- VBDOS 1.0 additions
```

Running the VBDOS manifest therefore includes every PDS case and every QB 4.5
case. A construct is not copied into each directory. Each `suite.toml` records
the source, complexity level, feature tags, original-compiler switches,
expected console output, linker libraries, DOS arguments/environment where
needed, and separate parser/lowering/runtime obligations.

Every deterministic executable writes its sole verdict with bare `PRINT`.
The DOS runner invokes `EXE > RESULT.TXT` and requires the captured file to
equal the manifest value exactly. `OPEN "CON"` is forbidden: measurement
proved that the named console device bypasses COMMAND.COM redirection. The
first inherited case separately forces color text mode, writes a marker, and
saves the complete `B800:0000` text page. Its 4,007-byte BSAVE file (header
plus 4,000-byte page) must match the measured golden byte for byte; this is a
screen-memory artifact check, not evidence that verdict redirection worked.
`B000h` is the monochrome page and is not correct after this color-mode setup.
Mode-13 captures use `A000h`.

All DOS-side sources, companions, expected-output files, and artifacts have
8.3 basenames. Every DOS text fixture is CRLF-only. The validator rejects a
lone LF or CR, a long DOS basename, a missing feature-specific runtime
verification claim, or an artifact golden that does not exist.

Validate the complete inherited set without invoking DOSBox:

```text
uv run tools/qbcompat.py qb45
uv run tools/qbcompat.py pds71
uv run tools/qbcompat.py vbdos --list
uv run tools/qbcompat.py vbdos --gaps
uv run tools/qbcompat.py qb45 --prepare-artifacts DOS_RUN_DIRECTORY
# compile with the qb frontend, link its OBJ to BCOM45.LIB, and run in DOS
uv run tools/qbcompat.py qb45 --artifacts DOS_RUN_DIRECTORY
```

`required` is an obligation, not a claim that the current compiler passes.
`pending` means the expectation or integration fixture still needs completion;
`reference-only` covers hardware, UI, external-object, or service-dependent
cases that cannot honestly be counted as a headless runtime pass. `--gaps`
prints every pending/reference-only compiler stage and every help row still
lacking an executable test. A green compatibility claim requires successful
fresh compilation by the qb frontend, linking that frontend's emitted OBJ
against the matching Microsoft runtime, captured DOS behavior, and byte-exact
artifact equality where declared. Microsoft BC acceptance and raw objects are
oracle/provenance evidence established once; they never make a qb-frontend
test pass.

No finite suite proves “100% compatibility.” Passing this ladder provides
strong evidence for every represented feature, but the claim must remain tied
to its feature matrix, compiler switches, runtimes, and hardware profile.

## Current corpus and frontend audit

The help-grounded structural gate contains 74 QB cases, 16 additive PDS cases,
and 23 additive VBDOS cases: 113 cases when the complete inheritance chain is
selected. ISAM and OS/2 are explicitly outside the requested scope; their
coverage rows and cases are removed, their headings are mechanically excluded
from generated gaps, and their prose remains only in the pinned FULL extracts.
The coverage inventories pin the installed Microsoft help bytes and map 989 QB
topics, 19 PDS delta contexts, and 54 VBDOS delta contexts to exact
source evidence or an explicit environment-bound/out-of-scope rationale.
The deterministic 8.3/CRLF full topic-body extractions are checked in beside
each inventory (`QB45*.TXT`, `B7*.TXT`/`BCHELP.TXT`, and the VBDOS help
extracts), so syntax, semantics, examples, and notes can be reviewed without
relying on a transient `build/` file. They are not heading-only indexes.
`tools/qbcompat.py` validates inheritance, evidence lines and TOML selectors,
HLP and full-extraction SHA-256s, extraction line counts, DOS files, runtime
verification claims, emitted-source hashes, exact runtime recipes, and measured
binary artifacts. An artifact check is
refused unless `--prepare-artifacts` opened a fresh run window first; that
preparation removes the exact declared outputs so an earlier DOS run cannot be
mistaken for the current frontend-linked run.

This is not complete coverage. The generated whole-chain report currently
contains 1,865 explicit gaps: 40 cases with at least one pending/reference-only
compiler stage, 433 classified non-ISAM/non-OS/2 help rows still lacking an
executable qb-frontend test, and 1,392 FULL-extraction headings not yet given a
granular classification (409 PDS and 983 VBDOS). The last category closes the
old measurement hole in which sparse delta tables looked complete merely
because no row existed for an omitted heading. Run `vbdos --gaps` to regenerate
the list; adding rows without an independent observable does not reduce it.

The first real stage dump is
`build/qbstages/qbcompat-qb45-00lexical-round`. It reaches HIR and optimized
MIR, then stops before physical MIR/LIR because the QB 4.5 runtime profile has
no complete `B$FREF` stack-cleanup contract. The dump also exposes an
independent constant-typing defect: the unsuffixed decimal literal `100000`
was first treated as a 16-bit INTEGER and folded to `4294936231` after sign
extension, whereas Microsoft BASIC selects a representable numeric constant
type. These are failing compatibility observations, not adjusted tests.

A focused VBDOS source probe also exposes an `END`-inside-`IF` ambiguity: the
current parser consumes the program-termination statement as the start of
`END IF`. Original PDS and VBDOS compiler acceptance sweeps are recorded in
their profile documentation. Consequently, suite validation and original-BC
acceptance do not imply that the new frontend already passes the runtime
ladder; no dialect is yet claimed fully compatible.
