> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/atomicexpand.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/atomicexpand.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/atomicexpand.rs sha256:99ea4c34a2911b9745aea2bc92554e0ae8fb3411e8edf887676495b844510718 -->

# atomicexpand

A read-modify-write the target has no instruction for, as a compare-exchange
loop: LLVM's `AtomicExpandPass` (`expandAtomicRMWToCmpXchg`).

## How it works
The block is split at the call. The first block loads the value (a plain
load), the loop block computes the new value from the loaded one, a
compare-exchange of loaded for new gives the old value and whether it
matched, and a mismatch retries with that old value. The exit's result is
the last old value. The failure ordering is the strongest the success one
allows (`getStrongestFailureOrdering`).

## Example
`%o = atomicrmw add ptr %p, i32 %v seq_cst`

```text
  %init = load i32, ptr %p ; br atomicrmw.start
atomicrmw.start: %loaded = phi [%init, pre], [%new_loaded, atomicrmw.start]
  %new = add %loaded, %v
  %pair = cmpxchg ptr %p, %loaded, %new seq_cst seq_cst
  %new_loaded = extractvalue %pair, 0 ; %ok = extractvalue %pair, 1
  br %ok, atomicrmw.end, atomicrmw.start
```

## Runs
In the m68k driver for a 68020 and later (`shouldExpandAtomicRMWInIR` is
`CmpXChg` there), on the atomics `llrm_mir::atomic::hidden` made calls of.

## See also
LLVM's `AtomicExpandImpl::expandAtomicRMWToCmpXchg`
([AtomicExpandPass.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/AtomicExpandPass.cpp)).
