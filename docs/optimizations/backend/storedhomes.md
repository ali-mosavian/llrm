> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/storedhomes.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/storedhomes.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/storedhomes.rs sha256:377ba4c2019fb9cb8fafd7798b06238b44e077e6e442c33d38802ce9e1f11e84 -->

# storedhomes

The cells the optimizer proved hold a phi's value, as the body now stands.

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

## Used by
[`crate::backend::spiller`](spiller.md) and [`crate::backend::floatassign`](floatassign.md) (`held`);
[`crate::backend::floatfold`](floatfold.md) and [`crate::backend::pressuresink`](pressuresink.md)
(`may_write`); [`crate::backend::postrasink`](postrasink.md) (`apart`).
