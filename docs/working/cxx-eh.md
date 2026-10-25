# C++ exceptions on m16 (table-driven)

Status: phase 1 built (`backend/cxxeh.rs`, several pads in both selectors); phases 2-4 not.

Clang's IR for `try`/`catch`/RAII is what `llrm-llc` must accept; today it takes only the
QB landing pad (`ehprepare.rs`, `isel/unwind.rs`: one pad, value unread, no `resume`).

## Shape

LLVM's Itanium model: `invoke` is a call between two labels; a landing pad is entered by the unwinder with the exception
pointer and selector in registers; `resume` calls `_Unwind_Resume`. The tables name what to do per call site.

No DWARF CFI. llrm frames are `push bp / mov bp,sp`, so the unwinder walks the `bp` chain; the only per-function fact it
needs is which callee-saved registers the frame pushed.

| Piece | m16 form |
|---|---|
| Frame walk | `[bp]` caller bp, `[bp+2]` return address; every function in a C++ program keeps its frame pointer |
| Unwind row (every function) | start, end, pushed `si`/`di` at `bp`-relative offsets; sorted table in its own segment |
| LSDA (functions with pads) | Itanium layout (call-site table, action records, type table) with 16-bit offsets |
| Pad entry | unwinder stores exception pointer and selector in `__eh_exception` / `__eh_selector`, jumps to the pad with `sp`/`bp` set; all registers clobbered (`__LANDING` as QB) |
| `resume` | `call __unwind_resume(exception, selector)`, which does not return |
| Personality | `__gxx_personality_v0` in the C++ runtime, two-phase search then cleanup |

## Phases

1. Backend (done): several pads per function; a pad reads the exception pointer and selector the unwinder left in
   `__eh_exception` / `__eh_selector` (not registers); `resume` calls `__unwind_resume`; `llvm.eh.typeid.for` is the type's
   place among the function's catch clauses, from 1. `invoke` labels come with phase 2. QB's single-pad path unchanged.
2. Tables: unwind rows and LSDA into OMF data with fixups; a test reading them back.
3. Runtime in C: `__cxa_allocate_exception`, `__cxa_throw`, `_Unwind_RaiseException`, `_Unwind_Resume`, personality,
   `__cxa_begin_catch`/`__cxa_end_catch`/`__cxa_rethrow`, `std::terminate`, class_type_info matching.
4. DOSBox tests: catch by value/ref/base, rethrow, nested, destructors during unwind, `dynamic_cast`.

## Open

- A C function without an unwind row ends the walk: `terminate`, as Itanium does. C code compiled by llrm gets rows too.
- Frame pointer omission is off for any function that has a row.
