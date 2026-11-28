> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/internalize.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/internalize.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/internalize.rs sha256:6b8b067c4c3e7506d8dbfe65f6c84dd2932d0015d55b3672e745a3310b4d10db -->

# internalize

Make every definition the program does not export local to it.

## How it works
The translation of LLVM's `InternalizePass`
([Internalize.cpp#L132](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/IPO/Internalize.cpp#L132)):
a definition with external linkage that is not preserved becomes internal
with default visibility. Preserved are declarations, available-externally
bodies, globals already local, the `llvm.` metadata arrays, the entry
points and the globals the program keeps (`exports`), and the names given
to the pass. [`crate::globaldce`](globaldce.md) then deletes what no live code names,
which is what a linker's `--gc-sections` does for a whole program.

## Runs
As a program pass, before GlobalDCE, where a driver compiles a whole program
(the 6502 driver adds it with `main` preserved).

## Example
With `main` preserved, `@scale` and `@table` become internal and the next
GlobalDCE deletes them if nothing reaches them:

```mir
@table = global [2 x i16] [i16 1, i16 2]
define i16 @scale(i16 %x) { ... }
define i16 @main() { ... }
```
