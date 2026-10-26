> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/callregs.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/callregs.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/callregs.rs sha256:a8d94d8f2bbee6cd6d7f2d26f7458341a19ed4ba1f6c987e1fd9ccd84a8b4dbd -->

# callregs

Which registers a runtime call destroys or keeps, read from its contract, so
the allocator knows what may stay live across it.

## How it works
A runtime routine's contract names the registers it disturbs. Each
allocatable register is matched by its 16- and 32-bit spellings:
`call_clobbers` is the registers the contract disturbs plus the selectors
it cannot be trusted with, and `call_keeps` is the general registers it
does not disturb.

Selectors are the exception. A contract describes the 8086 and names no
FS or GS, so where it disturbs every register, or was written for the
386 (`contract.i386`), the selectors it does not name are clobbered
too (`unnamed_selectors_clobbered`). For an i386 contract
`call_clobbered_high` is the registers the call keeps only the 16-bit half
of: every general register it does not clobber whole.
`call_clobbered_high_keeping` removes those the callee's convention saves
in full.

## Example
No honest example for the contract tables: they are data in the runtime
description, and no unit test prints a set. The result is in the dump. In a
nib `main`, the first runtime call is `N$BGRW` (the dump's call at 0x0001,
the first `call far ptr` of the listing) and a bounds-failure call is
`N$EBND` (the dump's call in block 0x000f, label `L1_15` of the listing):

```lir
call   requires v1@AX,v2@DX,v3@CX; delivers v4@EAX; clobbers EAX,ECX,EDX,EBX,ES,FS,GS
call   clobbers EAX,ECX,EDX,EBX,ES,FS,GS
```

`FS` and `GS` are in the set though no 8086 contract names them.

From `ISEL_DUMP=1 llrm-nib -O2 -fno-inline-functions
-fno-inline-functions-called-once -fno-ipa-cp-clone
-march=i386` of `examples/pass-docs/calleefacts.nib`, stage
`FarIndirectCalls`.

## Used by
`isel.rs`, `isel/wide.rs`, `isel/unwind.rs` (the clobber sets of a call
they select) and `abi/qb.rs` (how many registers a call keeps).
