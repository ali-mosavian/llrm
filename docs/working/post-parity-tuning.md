# Post-parity tuning

Owner: w-6. Where llrm may differ from the pin on purpose after parity, and how.

## Inlining at -Os on 16-bit code

The pin inlines a callee priced under `OptSizeThreshold` (50) whatever it does to the bytes: `idioms.bas` -Os is +1062 B for it (4747 to 5809), the
`called_once` kernel's `twice` (cost 45) is inlined. llrm's D11 and D12 (a code-size bound and stack-slot trials) held that back and are deleted.

The tuning path is a per-instruction inline cost for 16-bit code through the TTI cost hook, as LLVM does per target, not a veto in the pass.
