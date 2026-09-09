# Bounds across unit steps

FPDEEP's optimized MIR retained the loop-counter interval 1..3, but lost it
at `decrement i`. Range analysis handled binary subtraction but not the
equivalent unary operation. The following copies and shifts therefore had
no usable bounds.

Before:

```
i: 1..3
j = decrement i: unknown
offset = j << 2: unknown
p(i) byte extent: unknown
```

After:

```
i: 1..3
j = decrement i: 0..2
offset = j << 2: 0..8
p(i) byte extent: BC_DATA+6 through BC_DATA+17
```

QB uses an offset of 4..12 with a displacement of 2 instead; the resulting
byte extent is identical. The regression checks the actual covered memory,
not one compiler's expression shape. Increment and decrement refuse signed
wrap, just like the existing binary arithmetic analysis.

The PDS/VBDOS extent checks and valid unit-step checks failed before the
fix. 56 range/floating-bound tests pass afterward. A before/after emission
comparison across the ordinary PDS fixtures found no changed objects.
The proven extent is a prerequisite for finite array-value analysis, not
proof that the values are finite or permission to remove floating effects.

Initial stage dumps for this investigation:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-fpdeep-array-y4td5z34`.
