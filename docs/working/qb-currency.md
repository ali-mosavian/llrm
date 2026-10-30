# CURRENCY: design

PDS 7.1 and VBDOS have a fifth numeric type, `@` / `AS CURRENCY` / `DEFCUR`: a 64-bit integer scaled by 10000 (range
±922,337,203,685,477.5807). The front end parses it; `semantic.rs` refuses it ("CURRENCY is not attached yet"). The sweep of 573
sample programs counts it at 33 first refusals (about 11 files, all PDS/VBDOS source). QB 4.5 has no CURRENCY.

## What BC does (PDS 7.1, measured from its OBJ)
Runtime routines BC calls for a program using every operator and conversion:
Mixed-type conversions and I/O call `B$` routines: `B$I2CY B$I4CY` (INTEGER, LONG to CURRENCY), `B$CYI2 B$CYI4` (back), `B$FPCY
B$CYFP` (to and from the FP stack), `B$CPCY` (compare), `B$STCY` (STR$), `B$FVAL`/`B$FMKC B$FCVC` (VAL, MKC$, CVC),
`B$INTC B$FIXC B$SGCY` (INT, FIX, SGN), `B$PCCY B$PECY B$PEOS` (PRINT, WRITE), `B$INPP B$RDCY` (INPUT, READ), `B$USNG`.

**Arithmetic is not on the FP stack** (a first reading of this note said so; BC's OBJ shows otherwise). Currency expressions are
a stack machine in the runtime, `__aFcy*`, in registers: `mov bx, &a / call __aFcyld` pushes a (constants live in data),
`__aFcyadd`, `__aFcysub`, `__aFcymul`, `__aFcychs`, `__aFcyabs` take the top two (one), `mov bx, &c / call __aFcyst` pops into c.
`B$I2CY` (AX) and `B$I4CY` (DX:AX) push a converted integer, `B$CYI2` (AX) and `B$CYI4` (DX:AX) pop to one, `B$FPCY` pops the FP
stack onto it and `B$CYFP` the other way, `B$CPCY` compares the top two and returns the flags (`jnl` follows). Division of two
CURRENCY values did not use `__aFcydiv` (none appears): it went through the FP stack. So + - * are exact integer-scaled
operations (the multiply keeps 128 bits before the divide by 10000), `/` is DOUBLE.

## Design
* One storage type, 8 bytes, scale 10000, alignment as a DOUBLE; arrays, UDT fields, COMMON, SHARED, parameters (BYVAL by value
  8 bytes, by reference an address), function results (`FUNCTION f@`), `DEFCUR`.
* Expression rule (BC's): an operation with a DOUBLE or SINGLE operand is DOUBLE (the CURRENCY operand enters through
  `B$CYFP`); with only INTEGER/LONG/CURRENCY operands, + - * stay CURRENCY on the `__aFcy*` stack, `/` is DOUBLE and
  converts back with `B$FPCY` when stored. Comparisons call `B$CPCY`. Constants fold at compile time to the scaled integer.
* Under the Microsoft runtime the calls must be BC's, register calling convention included: the audited contracts in
  `abi/qb.rs` already carry register inputs and results (BX in, ES:BX out for array elements), so each `__aFcy*`/`B$..CY..`
  routine is a row with its registers. llrm's runtime gets C versions with the same convention and a static stack of 8-byte
  values; the stack is hidden state between calls, so the calls stay in BC's order and are never reordered or merged.
* The routines are llrm's own C in `runtime/qb` (like B$FILS); their stack contracts go in the audited table in
  `abi/qb.rs`, one row each, from the BC calling sequence (arguments pushed, callee-cleaned bytes) measured per routine.
  Under the Microsoft runtime (`' dialect: pds71`) they are Microsoft's, so the call shapes must equal BC's.

## Phases (one PR each; the first is the one that unblocks files)
1. Type, `@` and `AS CURRENCY`, `DEFCUR`, conversions both ways, `+ - * /`, comparison, constants, assignment, arrays and UDT
   fields, parameters and results. Test: `tests/run/qb/currency_*_pds71.bas` with expected output from PDS BC.
2. PRINT, WRITE, STR$, VAL, INPUT, READ, LSET/RSET, PRINT USING.
3. INT, FIX, SGN, ABS, MKC$, CVC, PUT/GET of records holding CURRENCY, `\`, `MOD`, `^`.

## Size and risk
Phase 1 needs the register-convention rows and a currency-stack runtime on top of the type work. It touches the type table, `common_type`, conversions, `declare_as`, the size of UDTs and arrays, parameter passing and
the contracts: a new numeric type in a front built on five fixed ids, so the largest of the sweep's items (about 3 days).
Risk: rounding of `B$FPCY` (round to nearest even on the scaled value? to be measured against PDS), and how the FP stack
operands travel in llrm's FP model (the `stored_float_argument` boundary used for SINGLE arguments).

## Open
* Per-routine call shape: measure each B$ routine's pushed bytes in BC's OBJ before writing its contract row.
* Whether VBDOS differs from PDS in any routine (the sweep's files are PDS and VBDOS).
