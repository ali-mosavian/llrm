# x_life and x_isqrt against main (oversight bench, m32, 2026-10-21)

Binaries: oversight's base (main 854bb7efc, md5 56f120bb59fa) and new (adad0265b1, md5 379ab337524c); the pin's `clang` at d1106deb71cc, `-m32 -march=i486`.

## X-LIFE-MECH: x_life -O1 +48% and -Os +44% clocks vs main

Matches the pin's mechanism. clang's `UnrollLoops` defaults to `OptLevel > 1` ([CompilerInvocation.cpp#L1936](https://github.com/llvm/llvm-project/blob/d1106deb71cc/clang/lib/Frontend/CompilerInvocation.cpp#L1936)); `PTO.LoopUnrolling` takes it (BackendUtil.cpp#L868) and the pipeline runs `LoopFullUnrollPass(Level, /* OnlyWhenForced= */ !PTO.LoopUnrolling)` at O1 as at O2 ([PassBuilderPipelines.cpp#L576](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Passes/PassBuilderPipelines.cpp#L576)). So at clang -O1 no loop is unrolled unless it asks, and the pin makes no cost decision at all; `UP.Threshold` (150 below -O3) is never read. Ours: `fix(pipeline)` c17cf3ad74 (2026-10-20) sets `loop_unrolling` off at -O1, `only_when_forced` is then true and the unroller returns before `LLRM_DEBUG=unroll` prints (new logs no decision at -O1, 9 at -O2, 10 at -Os; `-funroll-loops` turns it on and unrolls the 3x3 fully). Main (gcc-style: -O1 unrolls what does not grow) logged `11 ops -> 15 unrolled, budget 150, admitted`.

At -O2 both unroll: clang -O2 has 10 `movzbl`; ours admits the loop. At -Os clang's `OptLevel` is 2, so unrolling is on and the optsize threshold (0) keeps the loop, as ours (a single `movzbl` loop in both).

The stale part was `docs/reference/levels.md` (-O1 cells of `mir unroll` and the `unroll-loops` row) and the `Options::basic` comment: fixed.

## X-ISQRT: x_isqrt -O1/-O2 +29% clocks vs main: closed, matches the pin

GVN (`-fno-gcse` removes it) merges the `r >> 1` both arms compute, SimplifyCFG's two-entry-phi fold (`decide@675`) then speculates the arms into two `select i1 c, x, 0`, and a target without `cmov` lowers each to a branch diamond. The pin's clang -O1 and -O2 emit the same diamond (`movl $0,%ecx; movl $0,%edx; jb`). Instructions per iteration of the loop, by path: clang 10 / 13 (avg 11.5), ours 11 / 13 (12.0), main 9 / 7 (8.0). The remaining half instruction is the `test` after `shr` (flags not reused, w-2's row, 2026-11-02).

## X-LIFE-ZEXT: zero-extending byte load

Ours `xor ecx,ecx; mov cl,[m]` (2 instructions) where clang -march=i486 uses `movzbl` (1); main has the same. Owner w-2, 2026-11-02: first check whether it is a recorded DEVIATE (i486/P5 tuning) and price both on the 486 and P5.
