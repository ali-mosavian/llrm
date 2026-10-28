> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/cgpretdup.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/cgpretdup.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/cgpretdup.rs sha256:2f5e27733d04a06e1e7e021508bd4ac7a5e7ec32da833517871a128f40c309c4 -->

# cgpretdup

CodeGenPrepare's `dupRetToEnableTailCallOpts`: a return block that only
returns what a tail call produced is copied into the predecessors that end
in that call
([CodeGenPrepare.cpp#L3014](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L3014)).

## How it works
A block holding a phi, an optional `bitcast` and `extractvalue` of it, and
the `ret` (nothing else), or a `ret` alone, is a candidate. A predecessor
counts when it ends in `call; br ret-block` and the call is a `tail` call
whose return attributes the caller's permit (`attributesPermitTailCall`),
and either produces the phi's arm, or (a `ret void`, or a returned first
argument of `memcpy`, `memmove`, `memset`, `strcpy`, `strncpy`, `strcat`,
`strncat`) leaves its result unused. Each such predecessor gets its own
`ret` in place of the branch (`FoldReturnIntoUncondBranch`); the block goes
when nothing reaches it.

```text
 b1: tail call void @llvm.memcpy(..)      b1: tail call void @llvm.memcpy(..)
     br label %ret                            ret void
 b0: br i1 %c, label %ret, ..             b0: br i1 %c, label %ret, ..
 ret: ret void                            ret: ret void
```

Why it matters here: two returns give shrink wrapping two exits; one shared
return lets the pin's `ShrinkWrap` find no restore point where ours found
one (`x_mergesort` `msort`, -Os m32 +9 B).

DEVIATE RD1: `mayBeEmittedAsTailCall` is `CI->isTailCall()` (x86's), read
from the call's marker; no bitcast of a pointer is looked through (MIR's
pointers are opaque), and there are no fake uses or pseudo probes.

## Example
Before:

```mir
b1:
  tail call void @llvm.memcpy.p0.p0.i16(ptr %d, ptr %s, i16 8, i1 false)
  br label %ret

b0:
  br i1 %c, label %ret, label %b1

ret:
  ret void
```

After:

```mir
b1:
  tail call void @llvm.memcpy.p0.p0.i16(ptr %d, ptr %s, i16 8, i1 false)
  ret void

b0:
  br i1 %c, label %ret, label %b1

ret:
  ret void
```

## Runs
In `cgpaddr`'s run, after the empty blocks go.
