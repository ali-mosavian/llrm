# Inspecting runtime contracts

`tools/contracts.py` maps a function's reachable dependencies in 16-bit OMF
objects and libraries, then propagates contracts from callees to callers.
Supply additional libraries with repeated `--lib` arguments:

```sh
uv run python tools/contracts.py 'B$FreeHandleBlock' --lib /path/to/VBDCL10E.LIB --dump /tmp/contracts.json
uv run python tools/contracts.py 'MYPROC' --lib /path/to/module.obj --lib /path/to/runtime.lib --json
uv run python tools/contracts.py --all --lib /path/to/VBDCL10E.LIB --dump /tmp/library-contracts.json
```

The JSON contains each function's reachable disassembly, dependency edges,
register effects, stack cleanup, and unresolved evidence. Public symbols and
local call targets are both followed. Relocations, not the placeholder call
bytes, identify targets. Duplicate symbol definitions are ambiguous, not chosen
by file order. Recursive components are identified separately from their callers.

`--all` starts from every public entry in a `CODE`-class segment and analyzes
shared dependencies once. Additional/custom classes can be selected by repeating
`--code-class`. Other public entries are listed as excluded, not decoded as code.
OMF does not distinguish a function from a public code label or table: these are
entry candidates, not a claim that every symbol is a function. Unreferenced
private routines cannot be discovered without additional entry-point information.
Input file hashes identify the precise library version behind the report.

Local byte constants and equality tests can exclude impossible branch edges.
The JSON records each exclusion in `excluded_edges`. At joins, only agreeing
facts survive; calls discard constants, and relocated fields are never treated
as literal values. This is not yet context-sensitive propagation into callees.
For example, VBDOS B$ETS2 sets DL=2 before testing it against zero, so its
B$RDTRIG call is excluded. Its SI=0 argument to B$EVNT_SET is not yet used to
specialize that callee, and its error path remains part of the contract.

`preserved` is an entry-value proof across all modeled returning paths.
`restored` is the subset written during execution and proven to regain its
entry value. `clobbers` means **may clobber**, not necessarily changed on every
execution. `reads` conservatively includes saves and pass-through uses; it is
not a minimal argument list. `cleanup` is bytes removed beyond the return address.

The current scope is 8/16/32-bit general registers (SP reported through cleanup),
data segment registers, and aggregate flags. Byte lanes model overlapping aliases:
writing AL clobbers AL, AX and EAX but preserves AH and `eax[31:16]`. Saving AX
does not save EAX's upper half; saving EAX does. Callee effects propagate with
this same precision. x87 state is not proved. Code is assumed immutable; this is not a termination, exception,
or memory-safety proof. USE32 segments and wide OMF records are unsupported.

Register moves and word/dword stack saves/restores carry byte-lane entry tokens.
An arbitrary memory write may alias a saved stack value, so it invalidates those
tokens. This intentionally overstates clobbers until stronger alias evidence is
available. Indirect transfers, unavailable callees, recursion, unsupported stack
changes, and missing code prevent preservation guarantees. Nothing here installs
a contract into the optimizer automatically.

Single-symbol traversal defaults to 256 functions; `--all` has no function-count
limit by default. Both use 2,000 instructions per function;
`--functions` and `--instructions` raise these limits. Unvisited dependencies are
listed explicitly in JSON and callers remain unknown. Abstract execution also
has bounded state/stack tracking; exhausted budgets never count as proof.

Measured on VBDOS, `B$FreeHandleBlock` reaches the local routine at `0xda`,
which calls `B$FreeHandle`. The tool finds all three and reports four bytes of
root argument cleanup. Larger allocation routines reach indirect calls and
recursive dependencies: inspect those unknowns rather than turning a partial
report into an ABI guarantee.

The initial full VBDOS run visited 2,254 public code entries and 2,717 distinct
routines, with no unvisited roots or dependency targets. 724 had modeled
contracts and 1,993 remained incomplete; graph coverage is not proof coverage.
