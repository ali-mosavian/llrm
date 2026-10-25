# CURRENCY: design

PDS 7.1 and VBDOS have a fifth numeric type, `@` / `AS CURRENCY` / `DEFCUR`: a 64-bit integer scaled by 10000 (range
±922,337,203,685,477.5807). The front end parses it; `semantic.rs` refuses it ("CURRENCY is not attached yet"). The sweep of 573
sample programs counts it at 33 first refusals (about 11 files, all PDS/VBDOS source). QB 4.5 has no CURRENCY.

## What BC does (PDS 7.1, measured from its OBJ)
Runtime routines BC calls for a program using every operator and conversion:
`B$I2CY B$I4CY` (INTEGER, LONG to CURRENCY), `B$CYI4` (to LONG), `B$FPCY B$CYFP` (to and from the FP stack), `B$CPCY` (compare),
`B$STCY` (STR$), `B$FVAL`/`B$FMKC B$FCVC` (VAL, MKC$, CVC), `B$INTC B$FIXC B$SGCY` (INT, FIX, SGN), `B$PCCY B$PECY B$PEOS`
(PRINT, WRITE), `B$INPP B$RDCY` (INPUT, READ), `B$USNG`. `+ - * /` call none of their own: BC loads both operands with
`B$CYFP`, does the operation on the FP stack, and stores with `B$FPCY` (which rounds to the scaled integer). So CURRENCY
arithmetic is DOUBLE arithmetic with a rounding store; `\`, `MOD`, `^` convert to LONG or DOUBLE first.

## Design
* One storage type, 8 bytes, scale 10000, alignment as a DOUBLE; arrays, UDT fields, COMMON, SHARED, parameters (BYVAL by value
  8 bytes, by reference an address), function results (`FUNCTION f@`), `DEFCUR`.
* Expression rule: a CURRENCY operand is loaded as DOUBLE (`B$CYFP`); an operation with a DOUBLE or SINGLE operand is
  DOUBLE; with only INTEGER/LONG/CURRENCY operands the result type is CURRENCY (store via `B$FPCY`). Comparisons call `B$CPCY`.
  Constants fold at compile time to the scaled integer.
* The routines are llrm's own C in `runtime/qb` (like B$FILS); their stack contracts go in the audited table in
  `abi/qb.rs`, one row each, from the BC calling sequence (arguments pushed, callee-cleaned bytes) measured per routine.
  Under the Microsoft runtime (`' dialect: pds71`) they are Microsoft's, so the call shapes must equal BC's.

## Phases (one PR each; the first is the one that unblocks files)
1. Type, `@` and `AS CURRENCY`, `DEFCUR`, conversions both ways, `+ - * /`, comparison, constants, assignment, arrays and UDT
   fields, parameters and results. Test: `tests/run/qb/currency_*_pds71.bas` with expected output from PDS BC.
2. PRINT, WRITE, STR$, VAL, INPUT, READ, LSET/RSET, PRINT USING.
3. INT, FIX, SGN, ABS, MKC$, CVC, PUT/GET of records holding CURRENCY, `\`, `MOD`, `^`.

## Size and risk
Phase 1 touches the type table, `common_type`, conversions, `declare_as`, the size of UDTs and arrays, parameter passing and
the contracts: a new numeric type in a front built on five fixed ids, so the largest of the sweep's items (about 2 days).
Risk: rounding of `B$FPCY` (round to nearest even on the scaled value? to be measured against PDS), and how the FP stack
operands travel in llrm's FP model (the `stored_float_argument` boundary used for SINGLE arguments).

## Open
* Per-routine call shape: measure each B$ routine's pushed bytes in BC's OBJ before writing its contract row.
* Whether VBDOS differs from PDS in any routine (the sweep's files are PDS and VBDOS).
