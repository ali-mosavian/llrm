> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/calleepop.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/calleepop.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/calleepop.rs sha256:afe26aa27b752798bd311493ccb0c7b36a5d6465db7dfd21329b7cc794efc7c1 -->

# calleepop

Callee-pops calling convention (LLVM's GlobalOpt giving an internal function
a `fastcc`): an internal function all of whose callers are seen pops its own
stack arguments.

## How it works
The convention is assigned here and nowhere else: isel takes the cleanup of
both sides from it, so a call and the function it names agree. A function is
a candidate when `direct_only` accepts it (internal, never named but as a
callee), it is not variadic, and every call of it is a plain call in the
function's own convention.

```text
 f:  ...  ret             (cdecl)         f:  ...  ret 4     (callee pops)
 c1: call f; add sp, 4                ==> c1: call f
 c2: call f; add sp, 4                    c2: call f
```

Two cases, by what the target's description says.

By default a function takes the target's callee-pops twin of its convention
(`Machine::callee_pop`), if it has one and takes stack arguments. It is
priced by the target for the level being built, in clocks or, at -Os, in
bytes: the cleanup each call no longer makes, summed over the call sites
(`costs.cleanup(words)`, plus `pop`), must pay for what each `ret N` costs
(`return_pops`).

Where the description gives a private function a convention of its own
(`private`, as the x86 m32 description does with `fastcc32`), every
internal, non-variadic function whose convention may be replaced takes that
one, with the calls of it, under any ABI the program has. No price is asked:
registers cost no more than the stack at either end.

A function some call of which is not a plain call in its convention keeps
its convention.

## Translation
| LLVM | Rust | Kind |
|---|---|---|
| [`OptimizeFunctions`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/IPO/GlobalOpt.cpp#L2019): `fastcc` for a function with a changeable convention | `CalleePop::run` | MATCH |
| [`hasChangeableCCImpl`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/IPO/GlobalOpt.cpp#L1700) | `direct_only` and the plain-call test | MATCH |
| [`ChangeCalleesToFastCall`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/IPO/GlobalOpt.cpp#L1673) | the call-site loop in `run` | MATCH |
| [`useFastCCForInternalCall`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L8196): always true here | none | MATCH |
| what `fastcc` is: two integer arguments in ECX, EDX, the rest on the stack, the caller pops | `fastcc32` at m32 (the pin's); the callee-pop twin at m16 | MATCH |

CC1 is closed: the private m32 convention is the pin's `fastcc`
(`fastcc32`). `LLRM_PRIVATE_WATCALL=1` gives `watcall32` back, to compare;
it is no switch for production. Priced on the 71 m32 rows (486 model) and
QCport (62 modules, `qcport.py`), dda83cd1c5, the same binary on both arms;
new against the old `watcall32` default:

| level | clocks | code bytes | QCport bytes |
|---|---|---|---|
| -O1 | -0.9% | +0.05% | +0.18% |
| -O2 | -0.2% | +0.6% | +0.03% |
| -O3 | -0.2% | +0.4% | +0.08% |
| -Os | +0.2% | -0.1% | +0.06% |

The exit ("adopt the pin's `fastcc` if no worse") is met on clocks at
-O1..-O3, by `rectwo` (-9.6%) and `fib` (-5.0%); d_faces compile Ir -0.28%.
Rows above the old default are numbered, owner funcattrs-port, exit "tune
after the allocator work" (the cost is frame traffic around calls), review
2026-11-13:
- CC2, -Os clocks +0.2%: `x_hanoi2` +32.2%, `queens` +4.9%, `bintree` +2.0%.
- CC3, -O1..-O3 rows: `x_hanoi2` +9.4% at -O1 and +3.8% at -O2 and -O3,
  `queens` +8.2% at -O2.
- CC4, code bytes +0.05%, +0.6%, +0.4% at -O1..-O3 (`quicksort` and
  `bintree` +64 and +48 B at -O2), QCport +0.03% to +0.18%.

## Example
Before:

```mir
define internal i16 @work(i16 %a, i16 %b, i16 %c) {
b0:
  %x = add i16 %a, %b
  %y = add i16 %x, %c
  ret i16 %y
}

define i16 @f(i16 %x) {
b0:
  %p = call i16 @work(i16 %x, i16 2, i16 3)
  %q = call i16 @work(i16 3, i16 %x, i16 1)
  %s = add i16 %p, %q
  ret i16 %s
}
```

After:

```mir
define internal fastcc i16 @work(i16 %a, i16 %b, i16 %c) {
; ...
}

define i16 @f(i16 %x) {
b0:
  %p = call fastcc i16 @work(i16 %x, i16 2, i16 3)
  %q = call fastcc i16 @work(i16 3, i16 %x, i16 1)
  %s = add i16 %p, %q
  ret i16 %s
}
```

The function and both calls take the `fastcc` convention, so isel emits `ret
N` once and no cleanup after either call.

From unit test
`an_internal_function_called_directly_twice_pops_its_own_arguments` in
`calleepop_tests.rs`.

## Runs
A module pass, once, after the interprocedural step, `globaldce` and
`available-externally`, so the callers that remain are the ones that stay.
Skipped at -O0.
