# Arithmetic timing audit — incomplete

The CPU selector introduced in 9887b1c is not yet supported by a sufficiently
precise timing model. The pending reciprocal-division change remains
uncommitted. Correct runtime answers do not validate speed predictions.

## Verified findings

Intel's 80386 Programmer's Reference Manual, IMUL entry:

https://www.ardent-tool.com/CPU/docs/Intel/386/manuals/prref386/IMUL.htm

Register-operand word IMUL spans 9–22 clocks; dword spans 9–38. The
sign-extended immediate-byte forms span 9–14. For a positive known
multiplier, the documented early-out formula is
`max(ceil(log2(multiplier)), 3) + 6`. Thus the immediate factor 20 costs
11 core clocks, and 85 costs 13, not the selector's flat 22. This formula
alone does not establish whole-sequence execution cost or prefix effects.
The text rendering loses which register operand is underlined; inspect
the original page before applying it to register-register multiplication.

Intel's IDIV entry distinguishes word (27 clocks) and dword (43):

https://www.ardent-tool.com/CPU/docs/Intel/386/manuals/prref386/IDIV.htm

Intel Architecture Optimization Reference Manual 245127-001, Table 1-1,
printed page 1-8 (PDF page 32), describes Pentium II/III integer multiply
as four-cycle latency and one-per-cycle throughput. These are distinct
quantities; neither justifies assigning every operand form a scalar cost
of four. Printed page 2-16 explains that prefixes increase instruction
length and may constrain decoding. Do not extrapolate this document's
Pentium II/III details to every P6 model without checking.

https://download.intel.com/design/PentiumII/manuals/24512701.pdf

## Required corrections before relying on selection

- Separate instruction form, operand width and known multiplier value.
- Separate latency from reciprocal throughput and resource occupancy.
- Check the actual 16-bit-mode encodings, including operand/address prefixes.
- Compare the selected LEA sequence, not only its earlier shift/add expansion.
- Account for required copies and allocation effects without double-counting.
- Verify 486 and Pentium instruction tables, and exact P6 forms, from primary
  manuals. Existing recollected representative numbers are not that evidence.

The current HOTLPX P6 multiply choice and reciprocal-division cost claims
remain provisional.

## First correction

The selector now uses the documented 386 early-out formula for positive
imm8 multipliers (0–127), whose signed value is identical for word and
dword forms. Other forms still use the old estimate pending their audit.
The fail-first regression is factor 85: previously selected abstract chain
`shl 2; add source; shl 2; add source; shl 2; add source`, now direct IMUL.
The chain's current estimate is 17 clocks including a seed move; the
immediate IMUL's documented core cost is 13, not 22.
This is a correction of the multiply input to selection, not a claim that
the whole chain model or final machine schedule has been validated.
