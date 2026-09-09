# Constants across runtime calls

Constant propagation now consults raised write effects for established calls,
instead of discarding all memory facts solely because the call can write.
Unknown calls, barriers and writing calls without effects still clear facts.

The existing `MemRef.beyond` escape set names addresses, not object extents.
It cannot prove that a byte fragment above an escaped address is unreachable.
The first implementation retained bytes 1–3 of an escaped long after a possible
write; the fail-first regression in `tests/test_constant_call_memory.py` catches
each byte. Constant propagation therefore ignores nonempty escape exclusions
until the raise supplies trustworthy extents. Escape sets with no addresses in
the protected program-data segment remain usable, even when string-segment
addresses escape.

Before: even a proven no-escape call discarded a complete constant.
After: that constant survives; an escaped long loses every byte fact.
The first guarded implementation changed no PDS/G2 objects. Filtering escapes
by their segment then removed NEGNOT's input reloads and reduced FLAGS as well.
NOTS still exposes program-data addresses and remains conservative.

NEGNOT's PDS object shrank from 991 to 925 bytes. Static cost fell from 510 to
370 against an unchanged target of 290 (1.28x). QB costs 380 (1.31x), VBDOS 370
(1.28x). These are ranking units, not elapsed time. All four runtime cases pass
on all three compilers.

For `-(not a)`, the earlier emitted sequence reloads a, splits its halves,
complements both, then negates the pair. The new sequence is:

```asm
mov ax,0A987h
neg ax
mov bx,0EDCBh
adc bx,0
neg bx
```

The reload and complement disappeared. The remaining split negation was a
raising gap: a fully folded result should simply pass 12345679h to PRINT.

## Whole unary recognition

The raise now recognizes adjacent paired NOTs and NEG/ADC/NEG over extracted
halves of one value, even when the result goes to PRINT rather than a store.
It emits one whole-value unary operation followed by extracts for existing
word consumers. Observed flags and intermediate carry values prevent recognition.
No carry interpretation or register knowledge was added to optimization passes.

The expression above now emits:

```asm
mov ax,1234h
mov bx,5679h
push ax
push bx
; call B$PEI4
```

NEGNOT's PDS object is now 907 bytes, down from 925; static cost is 354 (1.22x).
QB costs 364 (1.26x), VBDOS 354 (1.22x). NOTS costs 582/378 (1.54x), still
outside the goal. NEGNOT, NOTS and ARITH pass all 57 output cases across the
three compilers. No elapsed-time claim is made.

Follow-up: supply object-range reachability in the raise, not machine knowledge
in constant propagation. Do not infer object ends from individual operand
addresses: a reference can name a field or the high half of the same object.

Focused verification also found three stale SPILL assertions requiring an
immediate 7. Stage dumps show strength reduction has already replaced the inner
loop's ten additions of 22 with one addition of 220. The regression now requires
that collapsed sum and no reload of h3, rather than an obsolete instruction.
