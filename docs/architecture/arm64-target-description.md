# arm64 `llrm_target::Target`: the 39 methods without a default (2026-11-08, v0 (unrun))

`llrm-target/src/lib.rs:476`. Read from the signatures and docs, not yet implemented. Owner llrm-arm64-lir. Arm64 data lives in the arm64 crate; nothing arm64-named in `llrm-target`; no edit to `llrm-target` until coordinator and oversight have seen the x86-shaped list (C below).

Answer source: **P** the pin (AArch64 .td / InstrInfo / FrameLowering), **E** existing arm64 code, **N** new arm64 data file, **D** own decision (DEVIATE row).

## A. arm64 answers it, no `llrm-target` change (27)
| method | answer | src |
|---|---|---|
| `name`, `machine`, `layout`, `cpus`, `default_cpu` | "arm64"; Darwin/Linux `Platform`; its datalayout; the `tti.rs` cpus | E |
| `registers`, `registers_text`, `register_named`, `stack_pointer`, `frame_register`, `register_capacity` | `REGISTER_INFO`, `registers.regs`; sp, x29; allocatable gprs | E |
| `debug_target`, `objects` | dwarf column of `registers.regs`; Mach-O/ELF writers behind `DebugObjectTarget` | E (adapter) |
| `stack_slot_bytes`, `results`, `callee_saved`, `convention_results`, `convention_callee_saved`, `conventions`, `calling` | 8; x0/x1, v0..; x19-x28 and d8-d15 (low 64); AAPCS64 and Darwin from a new `calling.toml` | E `cc.rs`, N |
| `register_op_bytes`, `shift_op_bytes`, `multiply_immediate_bytes` | 4 each: fixed-width A64 | P |
| `forms_text`, `dialect` | new `arm64.instr` (names as in `lir_body`, kinds, fixed regs, flags) and `dialect.toml` (N Z C V; `{cc}` eq..nv). The dialect is data (`instr.rs:10`), so no `llrm-target` edit | N |
| `address_forms`, `float_width` | base+imm, base+index<<s, pre/post; a vector file holds a float at its own size | P |

## B. answerable but x86-worded; arm64 gives the true answer (4)
| method | arm64 answer | note |
|---|---|---|
| `first_argument_offset(far)` | 16 (x29/x30 record), `far` ignored | `far` is x86-16's |
| `return_address_bytes(far)` | 0: the return address is in x30 | assumes a stacked address |
| `operand_bytes` | 4 | x86 "size without a prefix" |
| `object` (`bitness` 16 or 32) | needs a value for 64 | field is x86's |

## C. x86-shaped (8): arm64 can answer all within today's signatures
Read: `cost_model` is `fn(&CpuPrices) -> Rc<dyn llrm_mir::target::Machine>` (`lib.rs:358`), so arm64 answers it with its own `tti.rs` machine and ignores the x86-derived prices. `march`/`cpu_table` return `Option`, `marches` a `Vec`, the routines a `Result`, and `OperationCosts` has `Default` (`llrm-mir/src/target.rs:947`).
| method | arm64 answer today (no edit) | neutral default, a convenience for later targets |
|---|---|---|
| `march`, `marches` | `None`, empty | trait default |
| `cpu_table` | `None` | trait default |
| `operation_costs` | `OperationCosts::default()` | trait default |
| `cost_model` | the `tti.rs` machine (A in kind: real data) | none: every target has its own |
| `huge_fill_routine`, `huge_copy_routine`, `divide_routine` | `Err("none")` | trait default |

So no `llrm-target` edit blocks arm64. The defaults only save m68k, wasm, z80 and 6502 writing seven stubs; additive, x86 unchanged, deferred until the objection window closes.

## Effort for a second target (per method)
- **Trivial, under 20 lines (a constant or a lookup), 22:** `name`, `machine`, `cpus`, `default_cpu`, `layout`, `registers`, `registers_text`, `register_named`, `stack_pointer`, `frame_register`, `register_capacity`, `stack_slot_bytes`, `first_argument_offset`, `return_address_bytes`, `operand_bytes`, `float_width`, `register_op_bytes`, `shift_op_bytes`, `multiply_immediate_bytes`, `results`, `callee_saved`, `conventions`.
- **Stub, one line each (zero with a trait default), 7:** `march`, `marches`, `cpu_table`, `operation_costs`, `huge_fill_routine`, `huge_copy_routine`, `divide_routine`.
- **Data file, 6:** `calling`, `convention_results`, `convention_callee_saved` (`calling.toml`), `dialect` (`dialect.toml`), `address_forms`, `debug_target`.
- **Adapter over existing writers, 2:** `objects`, `object`.
- **Large, 2:** `forms_text` (the instruction file; arm64 about 250 names), `cost_model` (a `Machine` impl; arm64 has `tti.rs`, 1183 lines, a target without a TTI pays this).
- Shared cost of a second target beyond these: its `RegisterClasses` pins live in the same forms file, its `Frame` facts in `FrameRegisters`.

## Deviations expected
- `cpu_table`/`operation_costs` empty: arm64 prices from `tti.rs` (the pin's cost model), not `timings.times`. DEVIATE, exit when the cost model is data.
- Everything else follows the pin 1:1; a hook that cannot gets its row when written.
