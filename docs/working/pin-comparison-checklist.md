# Pin comparison checklist

TL;DR: the pin's `llc` allocates its own pre-RA code from our IR, not ours. Before a count of the pin's instructions, spills or frequencies is quoted against ours, the five lines below are stated in the report.

A comparison that skipped them was withdrawn once: `_place` (queens, m32 -O2) was "pin 56 instructions, ours 72", but the pin takes the three arguments from the stack (`MOV32rm %fixed-stack.n`), while our `calleepop` gave the static function `watcall32` and passed them in EAX/EDX/EBX. The two allocators did not get the same problem.

## The five lines

1. **Input stage.** Which stage dump of ours is the pin's input (`NN-name.ll`), and that the stage is the same for both arms of an A/B. Strip attributes the pin does not parse (`threeway`); say so.
2. **Target header.** The `target:` line `pin.py` prints (triple, CPU, features, datalayout, `optsize`). A number without it is not quoted. `--optsize` for a build for size.
3. **`optsize`.** `-Os` is `--optsize`; `-O2` is not. The pin's `llc` always runs at `-O2` codegen.
4. **Convention.** The calling convention and the argument registers of the function on each side. Compare only a function whose convention is the same: non-static (or an address-taken, `direct_only` false) so `calleepop` leaves it alone, or one `calleepop` gives the pin's own (`fastcc`: ECX/EDX). Say which, and the arguments. m32: a `calleepop`'d function is `watcall32` (EAX, EDX, EBX, ECX); the pin has `sysv32` (all on the stack). m16: `regparm3` or `regparm3pop`, the pin's `i386-unknown-unknown-code16` has its own.
5. **Opcode and pre-RA differences.** Name what is not the same before RA: isel's choices (our `switch table` dispatch, `lea` folding, address modes), phi lowering (parallel-copy groups on ours, plain copies on the pin), TwoAddress copies, the coalescer's order. If the pre-RA instruction counts differ, a final-count comparison measures isel as much as the allocator.

## What a pin comparison can and cannot say

- Block frequencies: the pin's `-print-bfi` / `-print-machine-bfi` against ours are comparable when the CFG is the same (the final IR of ours into the pin's `opt -passes='print<block-freq>'`).
- Instruction counts, spills, reloads: comparable only when lines 4 and 5 hold.
- Decisions: not comparable function by function. The pin's intervals are not ours. The same-input instrument is a printer of our pre-RA LIR as LLVM MIR (parked: an opcode mapping, days not hours). Until then, check our decisions against RAGreedy.cpp rule by rule (`LLRM_GREEDY_DECISIONS`).

## Method caveat: re-parsed text

`pin.py` runs the pin on re-parsed text, so use-list-order ties differ from the pin's own pipeline (IvUsers visits users in descending program order since IU1). See [ivusers-order.md](ivusers-order.md) (#3750) and its follow-up rows IU2-IU6.

## The internal-function convention

`calleepop` (the translation of GlobalOpt's `ChangeCalleesToFastCall`, see [selection](../abi/selection.md#private-functions-and-calleepop)) gives an internal, non-variadic m32 function the target's `private` convention `watcall32`, and on m16 the callee-pop twin where it pays. It is not the pin's: x86's `fastcc` passes two integer arguments in ECX and EDX. Proposed as a numbered DEVIATE (number to be given by oversight):

- Reason: registers cost no more than the stack at either end of a private call (selection.md); the m32 target's own convention is `watcall32`.
- Cost: every comparison of a static function against the pin differs in its arguments; it moves `_place`-like recursive functions' pressure; it can make ours smaller or larger than the pin with no allocation decision involved.
- Exit: none while the target prefers it; a pin comparison names the function's convention (line 4).
