# Calling a Nib library from QuickBASIC 4.5

`sortlib.nib` exports QB45 procedures: SUBs taking an array and a string by
reference, and DOUBLE, INTEGER and string FUNCTIONs, one of a 2-D array.
`scores.bas` calls them through `SORTLIB.BI`, which
`nibfront --declare bi sortlib.nib` wrote, and `Average#` calls back
into its `Mean#`.

Build the library on the host:

```text
target/release/llrm-nib docs/examples/basic/sortlib.nib -o SORTLIB.OBJ -O2
```

Then, in DOS, with `SCORES.BAS`, `SORTLIB.BI` and `SORTLIB.OBJ` in one
directory and QuickBASIC 4.5's `BC.EXE`, `LINK.EXE` and `BCOM45.LIB` on the
path:

```text
BC SCORES.BAS /O;
LINK SCORES.OBJ SORTLIB.OBJ,SCORES.EXE,,BCOM45.LIB;
SCORES
```

It prints:

```text
 97  88  73  60  42  15
 62.5
 36  66
ADA LOVELACE AL
```

Under DOSBox, whose x87 keeps 64 bits, QuickBASIC prints any `62.5#` as
`62.49999999999999`; the value itself is exact.

The library uses 386 instructions, so it needs a 386 or later.
