# Explicit integer overflow checks

`--basic-semantics` now controls BC's explicit `INTO` checks, independently of
`--bounds-checks`. Recognition and removal happen at the raise boundary; no
MIR optimization pass recognizes a machine overflow instruction.

```asm
; Before / BASIC compatibility       ; Native
add ax,1                             add ax,1
into                                 ; no explicit BASIC overflow trap
```

Trace/break calls remain. Native arithmetic still follows the widths of its
MIR values; removing an overflow observer is not permission to invent a wider
result. This change is specifically about explicit INTO sites, not a claim
that every numeric runtime operation is already replaced.

`tests/fixtures/regressions/ovfpol.bas` evaluates 32767+1 with an error handler that
prints ERR and the error number. Its three `ovfpol-*.obj` fixtures are real
QB/PDS/VBDOS output with each named primary configuration plus `/D /X`.
BC and BASIC-compatible output print `ERR 6` and `DONE`. Native output prints
`-32768` and `DONE`. All six optimized runs were verified; compiler logs report
zero severe errors and linker logs no errors.

Runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-overflow-policy-final-savmsx9f`.
HARR pass dumps after this change:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-native-overflow-4mwg1ulw/stages`.

HARR's debug builds still contain tracing/break barriers. Removing those
barriers just to hoist descriptor loads would discard behavior; this change
does not do that. Huge-array address lowering and checked loop preguards
remain open work.
