> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/calleepop.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/calleepop.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/calleepop.rs sha256:b3c7af684259aa00310d873c5e613d08c99913667ced0f8b1e595b9b6f0796e9 -->

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
