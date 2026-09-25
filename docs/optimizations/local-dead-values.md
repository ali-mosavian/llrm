# Dead values before an opaque reader

FPDEEP's newly explicit integer conversions still materialized the old
AX/DX halves, although PRINT already used the whole value. The later opaque
DOUBLE copy disabled dead-code removal for the entire body.

In a body containing incomplete readers, removal is now limited to versioned
SSA results overwritten in the same block before an opaque reader or exit.
Every other definition remains a conservative liveness root, including the
inputs needed to compute its replacement. Phi inputs stay protected. Global
phi pruning remains disabled in such bodies. Unversioned values are not
treated as versions of the same variable merely because both default to zero.

This analysis uses variable identity, not machine registers. It does not
declare the opaque copy's use list complete or enable unrestricted DCE.

The PDS conversion now ends with:

```asm
fistp dword [bp-4]
wait
mov eax,[bp-4]
push eax
call far B$PEI4
```

Before, unused EXTRACTs between the MOV and argument generated PUSH/POP
sequences reconstructing the original return halves. Those computations
and several other locally overwritten values now disappear.

PDS object sizes: FPDEEP **1541→1497**, FPEMU **2013→1914**.
FPDEEP static costs: PDS **13489→12287**, VBDOS **13419→12267**;
QB remains **12555**. These are not elapsed-time measurements.

The production checks failed before the change on PDS/VBDOS; QB was already
passing. 47 focused ownership/floating tests pass. All 69 output cases pass
across PDS, QB and VBDOS, requiring LIR emission. Only FPDEEP/FPEMU changed
in the ordinary PDS emission scan. The broader transform test file has nine
pre-existing failures and two xfails, reproduced with the unchanged HEAD
dead-code function and unchanged by this patch; no tests were weakened.

Stages and runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-local-dead-me70cclw`.
