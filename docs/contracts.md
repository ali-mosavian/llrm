# Inspecting runtime contracts

`tools/contracts.py` maps a function's reachable dependencies in 16-bit OMF
objects and libraries, then propagates contracts from callees to callers.
Supply additional libraries with repeated `--lib` arguments:

```sh
uv run python tools/contracts.py 'B$FreeHandleBlock' --lib /path/to/VBDCL10E.LIB --dump /tmp/contracts.json
uv run python tools/contracts.py 'MYPROC' --lib /path/to/module.obj --lib /path/to/runtime.lib --json
```

The JSON contains each function's reachable disassembly, dependency edges,
register effects, stack cleanup, and unresolved evidence. Public symbols and
local call targets are both followed. Relocations, not the placeholder call
bytes, identify targets. Duplicate symbol definitions are ambiguous, not chosen
by file order. Recursive components are identified separately from their callers.

`preserved` is an entry-value proof across all modeled returning paths.
`restored` is the subset written during execution and proven to regain its
entry value. `clobbers` means **may clobber**, not necessarily changed on every
execution. `reads` conservatively includes saves and pass-through uses; it is
not a minimal argument list. `cleanup` is bytes removed beyond the return address.

The current scope is 16-bit general registers (SP reported through cleanup),
data segment registers, and aggregate flags. Upper register halves and x87 state
are not proved. Code is assumed immutable; this is not a termination, exception,
or memory-safety proof. USE32 segments and wide OMF records are unsupported.

Whole-register moves and word stack saves/restores carry entry-value tokens.
An arbitrary memory write may alias a saved stack value, so it invalidates those
tokens. This intentionally overstates clobbers until stronger alias evidence is
available. Indirect transfers, unavailable callees, recursion, unsupported stack
changes, and missing code prevent preservation guarantees. Nothing here installs
a contract into the optimizer automatically.

Traversal defaults to 256 functions and 2,000 instructions per function;
`--functions` and `--instructions` raise these limits. Unvisited dependencies are
listed explicitly in JSON and callers remain unknown. Abstract execution also
has bounded state/stack tracking; exhausted budgets never count as proof.

Measured on VBDOS, `B$FreeHandleBlock` reaches the local routine at `0xda`,
which calls `B$FreeHandle`. The tool finds all three and reports four bytes of
root argument cleanup. Larger allocation routines reach indirect calls and
recursive dependencies: inspect those unknowns rather than turning a partial
report into an ABI guarantee.
