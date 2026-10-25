# C++ exceptions on m16 (table-driven)

Status: design, nothing built. Clang's IR for `try`/`catch`/RAII is what `llrm-llc` must accept; today it takes only the
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
| Pad entry | `ax` = exception pointer, `dx` = selector, all else clobbered (`__LANDING` as QB) |
| `resume` | `call __unwind_resume(ax, dx)` |
| Personality | `__gxx_personality_v0` in the C++ runtime, two-phase search then cleanup |

## Phases

1. Backend: several pads per function, pad values from `ax`/`dx`, `resume`, `invoke` labels. QB's single-pad path unchanged.
2. Tables: unwind rows and LSDA into OMF data with fixups; a test reading them back.
3. Runtime in C: `__cxa_allocate_exception`, `__cxa_throw`, `_Unwind_RaiseException`, `_Unwind_Resume`, personality,
   `__cxa_begin_catch`/`__cxa_end_catch`/`__cxa_rethrow`, `std::terminate`, class_type_info matching.
4. DOSBox tests: catch by value/ref/base, rethrow, nested, destructors during unwind, `dynamic_cast`.

## Open

- A C function without an unwind row ends the walk: `terminate`, as Itanium does. C code compiled by llrm gets rows too.
- Frame pointer omission is off for any function that has a row.
