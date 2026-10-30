# VBDOS forms: `$FORM` and `TYPE CONTROL` / `FORM`

Plan only; nothing is implemented. Five programs of the 573-file sweep (15 with copies) refuse on VBDOS's form
objects, all in the VBDOS toolchain's `INC` directory.

| Program | Refuses on | What it uses |
|---|---|---|
| CUSTEVNT | unknown TYPE CONTROL | `Ctrl AS Control` parameters; no `Ctrl.` access (a callback library) |
| SPIN | unknown TYPE CONTROL | the same, with 35 `Ctrl.` property accesses |
| SETUP | unknown TYPE FORM | `'$FORM` x4, `x AS FORM`, 66 `frm*.` / `Screen.` property uses |
| TORDRAW | `'$FORM` | `'$FORM frmTorus`, 17 `frmTorus.` / `screen.` uses |
| CMNDLG | `'$FORM` | a form from `CMNDLGF.FRM` |

## What VBDOS BC does (measured, VBDOS 1.0 BC)
- TORDRAW compiles with BC; LINK then stops on `L2029: 'FRMTORUS' unresolved external`. `'$FORM` names an external symbol
  whose object the IDE builds from the `.FRM` (the project's `.MAK` lists it). BC alone cannot make a runnable program,
  so there is no differential run against a reference here, only a compile and an object comparison.
- `FORM` and `CONTROL` are compiler-known types; properties and methods become calls into the forms runtime with
  property and method ids (the `INVOKEEVENT` / `INVOKEMETHOD` aliases in CUSTEVNT).

## Recommendation
1. Do the cheap half: `FORM` and `CONTROL` as built-in types (opaque, passed by reference), so CUSTEVNT compiles (2 programs
   with SPIN once its `Ctrl.` property form is measured with BC). One PR, object comparison against BC.
2. Defer the forms runtime (`'$FORM`, `x.Property`, `x.Method`): it needs the whole forms library and the `.FRM` objects
   from the IDE. Three programs of 573; no runnable reference. Not worth building before the 32-bit parity work.
