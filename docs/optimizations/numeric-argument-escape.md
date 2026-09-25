# Integer PRINT arguments are values, not escaped addresses

`module.escaped` previously treated the fixup in `push word [r]` as evidence
that r's address escaped. For integer PRINT that is false: prnval.asm's PRINTX
path consumes the integer bytes themselves. Its cleanup is two bytes for I2
and four for I4, as recorded in the established runtime contracts.

The recognizer excludes only complete, adjacent push groups whose byte count
matches a known PEI2/PSI2/PEI4/PSI4 call. Unknown calls, string/pointer calls,
wrong-width groups and address-taking outside those groups remain conservative.
It does not remove the runtime's general memory-writing effects.

For ARITH's first AND, the previously emitted fragment was:

```asm
mov eax,[b]
mov ebx,[a]
and eax,ebx
push eax
```

The computation now becomes:

```asm
mov eax,02040608h
push eax
```

PDS ARITH drops from 794 to 690 static ranking units, and 1487 to 1360 object
bytes. NOTS drops from 484 to 450 units and 1126 to 1082 bytes. These are not
elapsed-time measurements; ARITH still needs an independent target.

This exposed a JUMPS backend defect: a constrained ON GOTO input was renamed
in `uses` but not in the call's explicit source. The allocator then refused
the stale source value. Constraint splitting now updates both, with a fail-first
regression. No fallback is accepted as success.

ARITH, NOTS, JUMPS and FPDEEP pass all 96 output cases across PDS/G2, QB/O and
VBDOS/G3. The focused tests pass (77). Runtime checks cover the changed PDS
program set and their other two primary compiler variants, not the full suite.
