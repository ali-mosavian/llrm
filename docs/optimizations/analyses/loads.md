> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/loads.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/loads.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/loads.rs sha256:e859de67fd7c237e5e5e914a6f96fe3e494b7edd32f824bed960ca6f66cff585 -->

# loads

The value a load can reuse from an access before it in its block.

A translation of the available-value half of LLVM's
[`Loads.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/Loads.cpp)
at d1106deb71cc: `FindAvailableLoadedValue` (both overloads, :552 and
:798), `findAvailablePtrLoadStore` (:705), `getAvailableLoadStore` (:617)
and `AreEquivalentAddressValues` (:285).

## How it works
`find_available_loaded_value` scans back from a cursor in one block, at most
`max_insts_to_scan` instructions (`DEF_MAX_INSTS_TO_SCAN`, 6, as LLVM's
`available-load-scan-limit`; 0 is no limit). The first load of the same
address, store to it or constant `memset` that covers it gives the value.
A store to a different alloca or global is skipped; any other store, or
anything else that may write memory, ends the scan, unless the alias
analysis says it does not modify the location. Without one, a store with
the same base, constant offsets and no overlap is skipped. The cursor is
left at the access that gave the value, or after the one that ended the
scan, so a caller can scan on.

```text
store i16 %v, ptr %p
store i16 0, ptr %q      ; %q and %p are different allocas: skipped
%x = load i16, ptr %p    ; %x is %v
```

MIRROR: every check and its order, the limit, the cursor.

DEVIATE 1: no atomics, volatile aside: MIR has none, so an access is never
"at least atomic", and a volatile load or store is not CSE'd (a volatile
load of the pointer still gives its value, as LLVM's).

DEVIATE 2: no debug instructions to skip, and no element-wise atomics.

DEVIATE 3: a `memset` forwarded to a load wider than 128 bits gives
nothing (a constant integer here has at most 128 bits).

DEVIATE 4: the alias analysis is a `ModRefOracle`, which `BatchAAResults`
is, so a caller holding an analysis of its own passes it without naming
`BatchAAResults`.

## Used by
InstCombine's `visitLoadInst`, JumpThreading's load PRE, Promote.

## Example
```mir
%a = load i16, ptr %p
%b = load i16, ptr %p    ; %a, with is_load_cse
```
