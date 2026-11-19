# M68K-CS: valid programs the m68k selector refused (2026-11-09)

Owner: m68k. Exit: 2026-11-11, all three closed by the PRs below.

TL;DR: `x_strlen` (all levels), `x_popcount` (O1/O2/Os) and `scroll` (-O0/-Oz) died with `CannotSelect`. Three causes, three fail-first tests.

| Row | Programs | Cause | Fix | Test |
| --- | --- | --- | --- | --- |
| M68K-CS-SRL | x_popcount O1/O2/Os | the shared combine hook made post-legalization shifts with an `i8` amount; m68k selects shifts by `i32` (pin: pointer-width `getScalarShiftAmountTy`) | `CombineHooks::shift_amount_type` = `i32` | `tests/bswap.rs` (popcount's `bswap` form, run on qemu) |
| M68K-CS-SUBREG | x_popcount (bswap32) | pin's `bswap` is `SUBREG_TO_REG(ROL16di(EXTRACT_SUBREG ..))`; the emitter took only a cleared low word | `rotated_low_word`: `rol.w #8; swap; rol.w #8` on one register | `tests/bswap.rs` |
| M68K-CS-IDX16 | x_strlen all levels, scroll -O0/-Oz | `select_arii` refused a constant index so `ARID` could take it, but `ARID` has one base register and `buf + k + 500` has two: no pattern for the store | refuse the constant only when `select_arid` matches | `tests/far_index_store.rs` (run on qemu) |

Each test fails on the tree before its fix: `CannotSelect(Shl)` / `SUBREG_TO_REG of anything but a cleared low word` / `CannotSelect(Store)`.
