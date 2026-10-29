# Conformance tests and tools

For contributors changing a convention. What pins each convention, against which compiler's output, and how to run it. Each reference is a compiler's own output, committed; llrm is checked against it and never against itself.

| Convention | Reference | Pinned by |
|---|---|---|
| `cdecl16`, `pascal16` (far and near) | BCC 3.1 `-S` listings and objects | the matrix below |
| BASIC procedures and runtime calls | BC `/A` listings and objects of VBDOS, PDS 7.1 and QB 4.5 | the matrix below |
| `watcall16`, `watcall32` | `bwcc`, `wcc386 -3r` output | `compile.rs`, `tests/register_convention.rs` |
| `regparm3`, `ia16`, `sysv32` | gcc's output (`-mregparm=3`, gcc-ia16, `gcc -m32`) | `compile.rs`, `tests/run/c/regparm3_matrix_*`, `tests/register_convention.rs` |

## The matrix against BCC and BC

Each program is a callee module and a caller module built twice, by the reference compiler and by llrm, and linked in all four pairings. A harness the reference compiler built checks every value that arrives and comes back, and prints only failures.

```
                       callee: reference      callee: llrm
 caller: reference     BB  (control)          BL
 caller: llrm          LB                     LL
```

| Part | Where | Run |
|---|---|---|
| C: `cf` cdecl far, `cn` cdecl near, `pf` pascal far, `pn` pascal near; scalars, and structs by value | `tests/fixtures/callconv/c`, `tools/callconv/c.sh` | `cargo test --release --test callconv -- --ignored` |
| BASIC: callees `CE`, `CES`, callers `CR`, `CRS`, `CUE` and `CVE` (PDS 7.1 and VBDOS) | `tests/fixtures/callconv/bas`, `tools/callconv/bas.sh DIALECT WORK` | the same |

`runs.txt` beside each matrix lists the failures still expected, `<program> <case>` or `<program> crashed`; a failure not listed, and a listed one that passes, both fail the test. The run needs the DOS toolchains and `dosrun`.

`tools/callconv/reference.sh` rebuilds BCC's side (`bcc.sh` is its compile step) and `tools/callconv/basref.sh` BC's listings; the commands are in [reference/tools](../reference/tools.md) and the fixtures in `tests/fixtures/callconv/readme.md`.

## The unit level

The matrix's listings are read as facts, one per line with the listing line it comes from: where each argument lies, what returns and where, what is kept, what the callee pops.

| Test | Pins |
|---|---|
| `cargo test --release -p llrm-qb callconv` | BASIC's lowering of each call boundary equals BC's listings |

## Per-convention tests

| Test | Pins |
|---|---|
| `tests/register_convention.rs` | `watcall32`: arguments in EAX, EDX, EBX, ECX, the rest on the stack popped by the callee; `cdecl32`; what a callee saves; private functions under `-mabi=sysv`; a full-width `va_start` |
| `crates/codegen/llrm-codegen/src/backend/isel_tests.rs` | the frame and register save sequence of a far function |
| `tests/ipra.rs` | callee facts: a caller keeps a value in a register a private callee does not write; an exported or recursive one tells nothing |
| `tests/nearcalls.rs` | near entry for a private function in C, Nib and BASIC |
| `tests/dead_arguments.rs` | a parameter nothing reads leaves with its argument |
| `tests/indirect_call.rs` | a call through a pointer passes its arguments in the convention's registers |
| `tests/frame_register_free.rs` | a function that needs no frame register holds a value in it |
| `tools/callconv/matrix.py SEED N OUTDIR` | random signatures: scalars of every width, floats, near and far pointers and structs, each called directly and through a pointer, compared with a host compiler; `tests/run/c/regparm3_matrix_*` are seeds |
