# CodeGenPrepare `optimizeSwitchType` (w9, 2026-10-25)

The pin widens `switch i8/i16` to a `zext` and `switch i32` before isel (`llc -stop-after=codegenprepare`, m16/m32/m64: i32). llrm had no such step; `_pl_select_weapon` -Os compared `cmp ax, 9` and extended the index for the jump table, the pin `movzwl; decl; cmpl $9`.

Added `cgpswitch.rs` (run from `cgpaddr`) and `Lowering::preferred_switch_condition_bits` (x86: at least 32).

DEVIATE SW2: the 16-bit target keeps the narrow switch. Unguarded, m16 grew: queens -O3 +15 (old) / +3 (dag), x_life -O1..-Os +6..+11. Exit: when m16 prices operand-size prefixes like the pin.

## Evidence (base `330a08b714cd`, new `56b9f3c2172d`)

- objsweep, vsgcc 568 cells, `--isel=old` and `--isel=dag`: 0 changed, 0 failed.
- QCport (stubbed tree, 65 modules) emitted bytes at `-Os`:
  - m32 old: 260594 -> 260457 (-137); 64 of 65 compile under old. Largest: combat -48, mdl_ai -31, weapons -31, triggers -30, cmd +114, item +11.
  - m32 dag: 267793 -> 267738 (-55). weapons -27, mdl_ai -30, cmd +29, plats +17, sys +16, keybind +9.
  - m16 old and dag: unchanged (guard).
