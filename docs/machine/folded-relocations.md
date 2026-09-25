# Numeric folding drops address relocation ownership

The ARITH stage dump exposed a real object-file defect after stronger constant
propagation. A load became `mov esi,12345678h`, but retained its `symbol` marker.
Emission attached the old load's OFFSET16 fixup to the new immediate. LINK adds
the address to the field, so the numeric value was no longer stable after link.

Before:

```asm
mov esi,12345678h ; OFFSET16 BC_DATA+6 patches the immediate
```

After:

```asm
mov esi,12345678h ; no relocation
```

`_folded_op` now clears the marker when replacing an operation with a numeric
constant. Legitimate relocated stores and symbolic operands are unchanged.
The emitted-object regression checks the immediate field against actual OMF
fixups; all three compiler variants failed before the fix and pass afterwards.

ARITH is the only changed PDS/G2 fixture. Its object shrinks 1360 to 1355 bytes
because one fixup disappears; the machine instruction stream is unchanged.
All thirty ARITH output cases pass across QB, PDS and VBDOS. Those output checks
also passed before the fix, which is why the byte/fixup regression is necessary.
