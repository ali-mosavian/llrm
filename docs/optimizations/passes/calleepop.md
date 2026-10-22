> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/calleepop.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/calleepop.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/calleepop.rs sha256:3663592e3a22a72e5b62998f4f5df220cfdf7aa5439785b63b932c1655ef4f39 -->

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
(`private`, as the x86 m32 description does with `watcall32`), every
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
| what `fastcc` is: two integer arguments in ECX, EDX, the rest on the stack, the caller pops | `watcall32` at m32, the callee-pop twin at m16 | DEVIATE CC1 |

DEVIATE CC1 (owner funcattrs-port, review 2026-11-05). Reason: on the 486
and P5 an argument register costs no more than a stack slot, and `watcall32`
has four of them. Cost, ours against the pin's `fastcc` (`LLRM_PIN_FASTCC=1`
builds the latter), m32, 486 model, f36fd3559b: the 71 rows' clocks (code
bytes) at -O1 -0.9% (+0.1%), -O2 -0.2% (+0.6%), -O3 -0.2% (+0.4%), -Os +0.2%
(-0.1%); recursion moves it most, `rectwo` -9.6% at -O1, `x_hanoi2` +32% at
-Os. QCport, 62 modules, bytes ours/pin: -O1 299137/299678, -O2
317103/317209, -O3 342916/343179, -Os 238865/239002 (+0.06%). Exit: adopt
the pin's `fastcc` when its rows are no worse; they are mixed, so not yet.

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
