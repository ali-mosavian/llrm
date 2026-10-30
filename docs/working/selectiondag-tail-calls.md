# Tail calls in the DAG path (sibling calls)

The pin turns a call in tail position into `jmp` (`TCRETURNdi` becomes `TAILJMPd`) at -O2; fib, hanoi and
recchop show it in the shadow. Neither selector does it today (`call g; ret`). Owner: sdag-a. Written
2026-10-24; steps dated below. The old selector is the default path and does not get it: this ships with the
DAG when the DAG becomes the default (`selectiondag-default-readiness.md`); until then only `--isel=dag` and
the shadow carry it.

## Steps, each a fail-first test by execution

| # | Step | Pin function | Date |
|---|---|---|---|
| 1 | Call in tail position: the call's value is the block's return, nothing between | [isInTailCallPosition](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/Analysis.cpp#L539), [SelectionDAGBuilder::canTailCall](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGBuilder.cpp#L9235) | 10-27 |
| 2 | Eligibility: same convention, no byval or sret mismatch, callee pops no more than the caller's argument area | [isEligibleForSiblingCallOpt](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLoweringCall.cpp#L2976), [LowerCall's isTailCall](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLoweringCall.cpp#L2160) | 10-30 |
| 3 | `X86ISD::TC_RETURN` node and its operands (callee, popped bytes, argument registers, no `callseq`) | [LowerCall TC_RETURN](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLoweringCall.cpp#L2712) | 11-02 |
| 4 | Matcher: `TCRETURNdi` / `TCRETURNri` (sdag-c's tables already have the patterns; check m16 has none) | [X86InstrCompiler.td](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86InstrCompiler.td#L1393) | 11-04 |
| 5 | Emitter + frame: the epilogue goes before the jump | [emitEpilogue / isTailCallOpcode](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FrameLowering.cpp#L2572), [X86ExpandPseudo](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ExpandPseudo.cpp#L281) | 11-06 (first PR) |
| 6 | m16 (far callee: `jmp far`, regparm3 pops) after m32 is green | same | 11-13 |

## Decisions

- m32 first; m16 after. At m16 a far call tail-jumps with the far return address already on the stack, so a callee
  that pops arguments differs from the caller's: step 2's check carries it.
- The LIR needs a return-like instruction that the frame pass gives an epilogue (`Operation::Return` with a
  symbol), not a plain `Jump`: step 5 settles it by reading `framelowering.rs`.
- Each step's test is an execution test (fib, hanoi, recchop) under `--isel=dag` against the old selector's output.

## Default-on at the flip (w9, 2026-10-25)

Landed: m32 direct (04b7ba0f0), indirect `TCRETURNri` (#4480), m16 `jmp far` (#4483, direct callees only). On by default under `--isel=dag` since 2026-10-26; `LLRM_TAIL_CALLS=0` turns them off (one release, then the env read goes), and the old selector never makes them.

What the default needs, in order:

1. **The switch (done 2026-10-26).** `dag_function.rs` `with_tail_calls(env || TAIL_CALLS)` becomes true under `--isel=dag`; keep `LLRM_TAIL_CALLS=0` as the off switch for one release, then delete the env read. One line plus a test that the default builds a `TC_RETURN` (`a_sibling_call_through_a_pointer_jumps_through_a_register` and `an_m16_tail_call_jumps_to_the_callee` set the thread switch today).
2. **Corpus proof, both targets.** `run_tests.py c alias huge nib qb` under dag with the flag on, m16 and m32, must pass the same set as with it off. Run 26 (before #4483 landed, flag on, 734 programs): 698 pass, 36 red, none caused by the flag. Re-run on the tree with #4508 (the only open red class) and post the pass set diff; expected 734 / 734 both ways.
3. **DOSBox, the programs that exercise it.** `tests/run/c/tail_calls.c` (m16 and m32; direct, swapped and extra arguments, a framed caller, mutual recursion, a counting loop, a call through a pointer). It passes with the flag on and off. Add before the flip: a callee that pops arguments (`-mabi=regparm3` at m16), a `byval` or `sret` argument (must stay a call), a variadic call (stays a call unless every parameter is in registers), a float result, and a far-pointer callee at m16 (stays a call).
4. **Rows.** vsgcc old/dag clocks on the recursion kernels (`x_hanoi2`, `bintree`, `x_list`, `recchop`, `recsum`): the flag should lower clocks and not raise code bytes; record the numbers in the flip table.
5. **Side readers of a tail `jmp`.** `-fstack-usage` and `-Wstack-usage` read the call graph: a tail edge adds no frame, so the sum along it is an over-estimate, not an error (check one program). `-g` marks and `Callee.tail` printing are covered by the DOSBox runs above.

The pin's rule when a call is not eligible: `tail` is a hint. `isInTailCallPosition` (the call's value is the `ret`, nothing between that reads memory or traps) and `isEligibleForSiblingCallOpt` (same convention and returns, no `byval`/`sret` mismatch, callee-saved registers kept, stack arguments already in the caller's incoming slots, callee pops what the caller does, a free caller-saved register for an indirect callee) must both hold; if either fails the call is an ordinary `call` + `ret`, no diagnostic. Only `musttail` is a requirement: the pin stops with "failed to perform tail call elimination on a call site marked musttail" when it cannot. Ours calls normally for a `musttail` that is not eligible (silently); no producer of `musttail` exists in the pipeline, so it is a row, not a blocker: make it a refusal when one appears.
