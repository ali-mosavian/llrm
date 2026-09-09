# Floating-to-integer results are MIR values

B$FIST and B$FIS2 are now recognized in the frontend as strict floating
storage conversions producing ordinary 32-bit and 16-bit integer values.
The old machine return halves are expressed as extracts; argument raising
can rejoin their stack arguments. No register knowledge enters an opt pass.

The installed QB45, PDS and VBDOS libraries were disassembled independently.
All three have the same B$FIST body at offset 25h:

```asm
push bp
mov bp,sp
sub sp,4
wait
fistp dword [bp-4]
nop
wait
pop ax
pop dx
mov sp,bp
pop bp
retf
```

B$FIS2 at 14h uses two bytes and omits POP DX. Recognition requires the
established exact helper contract, no local override, no observed flag
result, and the object's FIDRQQ protocol. Changed contracts are refused.
The backend materializes an owned temporary, emits the conversion with
pre/post waits, and reads the integer only afterward. Normal peephole rules
may remove an explicit wait already covered by a waiting FP instruction.
Rounding and floating exceptions remain strict; no conversion is folded here.

Before, the caller hid the conversion behind `call far B$FIST` and passed
DX:AX to PRINT. After, the generated code includes:

```asm
fld   dword [q]
fistp dword [bp-4]
wait
mov   eax,[bp-4]
; integer value now feeds the ordinary dataflow and argument lowering
```

The remaining return-half extracts still cause avoidable moves in some
builds. This is not a claim that integer-result allocation is finished.
PDS FPDEEP object size grows 1506→1541 bytes; FPEMU grows 1957→2013.

## Cost the helper body too

The scoreboard previously charged B$FIST/B$FIS2 only the default 20 units,
making all conversion work inside the helper invisible. Independent sums
of the decoded instructions are 86 and 80 units in every installed library.
B$FIST's per-instruction costs are 6,2,2,5,34,2,5,6,6,2,6,10; RETF reads
both words of its far return address. With call overhead, the entries are
106 and 100, not hardware timings. The target listing is unchanged.

Using the corrected instrument on both sides:

| FPDEEP build | Before | After |
| --- | ---: | ---: |
| PDS | 14481 | 13489 |
| QB | 14699 | 12555 |
| VBDOS | 14411 | 13419 |

Verification: three production MIR regressions and two cost regressions
failed first. 173 focused tests pass, including contract/flag guards and
both result widths' wait/temporary placement. Only FPDEEP and FPEMU changed
in the ordinary PDS scan. All 69 output cases pass across three compilers,
with LIR required rather than fallback.

FPDEEP runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-float-integer-2x0plurx`.
FPEMU checks and FPDEEP stage dumps:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-conversion-check-3lt7tszp`.
