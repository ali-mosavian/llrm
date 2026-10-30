> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/storedhomes.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/storedhomes.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/storedhomes.rs sha256:7278ea912f2a6c71eb9e2d66d28c9c7b812e0e06076c0a5796b8e0954c58d7f5 -->

# storedhomes

Finds the memory cells that provably still hold a phi's value, so a spilled
value read from one needs no store and no slot of its own.

## How it works
MIR proves, for a phi, that a memory cell already holds its value
(`!llrm.home`, carried into the body as `LirBody.homes`). A spilled value
read from such a cell needs no store and no slot of its own. This module
re-checks the proof against the machine code, since later phases may have
changed it:

```text
 held(body, wanted):
   homes the body names, filtered by the caller's `wanted`
     drop a value with a definition that is not a plain copy
       (a spilled value's definitions are removed by spilling)
     walk each block backward with the values live there
       drop a value when an instruction may write its cell
       and is not itself a store of that value to the cell
```

`may_write` is the spiller's own test for the frame, plus any store to other
memory that is not certainly elsewhere. `apart` says two cells are certainly
different bytes: both are fixed addresses that `overlap` separates. A
pointer, or two elements of an array, are never apart; what a pointer
reaches is the optimizer's proof, not checked here.

## Example
No honest example: `held`, `may_write` and `apart` answer queries and print
no LIR of their own. Read the spiller's `ISEL_DUMP=1` output (see
[`crate::backend::lirtext`](lirtext.md)) to see a spilled phi read from its home cell.

## Used by
`crate::backend::spiller` and [`crate::backend::floatassign`](floatassign.md) (`held`);
[`crate::backend::floatfold`](floatfold.md) (`may_write`); [`crate::backend::postrasink`](postrasink.md)
(`apart`).
