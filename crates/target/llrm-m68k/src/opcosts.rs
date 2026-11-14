//! What an IR operation costs in code bytes on m68k: the length of the
//! instructions it becomes, from the encoder (the one source of a form's
//! length; `tests/opcosts.rs` assembles each form with binutils and compares).
//!
//! `Machine::size_costs` was `OperationCosts::default()`, one per operation,
//! so every byte count a size decision read from it was an instruction count.
//! The 68000 has no 32-bit multiply or divide: those are a library call (two
//! pushes, `jsr`, the cleanup); the 68020 has `muls.l` and `divs.l`.
//!
//! Floating point is the soft-float library: an operation pushes its operands'
//! words from their stack slots, calls, takes the arguments off and stores the
//! result words, priced once here as that expansion.
//!
//! Clocks are not here yet: `costs()` still prices an operation at one.
//!
//! # See also
//! MC68000UM chapter 8 (effective-address extension words); `sizes.rs`, the
//! `CodeSize` channel, to be derived from these forms.

use llrm_mir::target::OperationCosts;

use crate::dagisel::hooks::Cpu;
use crate::mc::{Inst, Operand};
use crate::reg;

fn inst(
    opcode: &'static str,
    operands: Vec<Operand>,
) -> Inst {
    Inst { opcode, operands }
}

fn r(register: llrm_lir::registers::RegId) -> Operand {
    Operand::Reg(register)
}

/// The bytes of `form`: the encoder's.
fn bytes(form: &[Inst]) -> Result<i64, String> {
    form.iter().try_fold(0, |sum, one| Ok(sum + crate::encode::encode(one)?.bytes.len() as i64))
}

/// A 32-bit multiply or divide: an instruction from the 68020, else the
/// library call.
fn wide(
    cpu: Cpu,
    opcode: &'static str,
) -> Vec<Inst> {
    if cpu.at_least(Cpu::M68020) {
        return vec![inst(opcode, vec![r(reg::D0), r(reg::D0), r(reg::D1)])];
    }
    vec![
        inst("MOV32ed", vec![r(reg::SP), r(reg::D1)]),
        inst("MOV32ed", vec![r(reg::SP), r(reg::D0)]),
        inst("CALLb", vec![Operand::Symbol("__mulsi3".to_owned(), 0)]),
        crate::expand::sp_adjust(8, true),
    ]
}

/// A soft-float operation: `words` argument words pushed from stack slots, the
/// call, the arguments taken off, `stores` result words stored.
fn libcall(
    words: i64,
    stores: usize,
) -> Vec<Inst> {
    let mut form: Vec<Inst> = (0..words).map(|_| inst("MOV32ep", vec![r(reg::SP), Operand::Imm(4), r(reg::A0)])).collect();
    form.push(inst("CALLb", vec![Operand::Symbol("__adddf3".to_owned(), 0)]));
    form.push(crate::expand::sp_adjust(4 * words, true));
    form.extend((0..stores).map(|_| inst("MOV32pd", vec![Operand::Imm(4), r(reg::A0), r(reg::D0)])));
    form
}

/// The forms of each operation class, named as `OperationCosts` names them.
pub fn forms(cpu: Cpu) -> Vec<(&'static str, Vec<Inst>)> {
    let imm = Operand::Imm;
    vec![
        ("add", vec![inst("ADD32dd", vec![r(reg::D0), r(reg::D0), r(reg::D1)])]),
        ("shift", vec![inst("SHL32di", vec![r(reg::D0), r(reg::D0), imm(2)])]),
        ("multiply", wide(cpu, "SMULd32d32")),
        ("divide", wide(cpu, "SDIVd32d32")),
        ("address", vec![inst("LEA32p", vec![r(reg::A1), imm(4), r(reg::A0)])]),
        ("load", vec![inst("MOV32dp", vec![r(reg::D0), imm(4), r(reg::A0)])]),
        ("store", vec![inst("MOV32pd", vec![imm(4), r(reg::A0), r(reg::D0)])]),
        ("store_immediate", vec![inst("MOV32pi", vec![imm(4), r(reg::A0), imm(3)])]),
        ("memory_update", vec![inst("ADD32pd", vec![imm(4), r(reg::A0), r(reg::D0)])]),
        ("move", vec![inst("MOV32dd", vec![r(reg::D0), r(reg::D1)])]),
        ("call", vec![inst("CALLb", vec![Operand::Symbol("f".to_owned(), 0)])]),
        ("return_", vec![inst("RTS", vec![])]),
        ("adjust", vec![crate::expand::sp_adjust(8, true)]),
        ("extend", vec![inst("EXT32", vec![r(reg::D0), r(reg::D0)])]),
        ("set", vec![inst("SETd8eq", vec![r(reg::BD0)])]),
        // A short branch (`bra.b`, `bcc.b`): 2 B; the assembler widens a far one to 4, which the function's
        // `function_scale` carries. A branch to the next instruction is not a `.b`; the test lays a `nop` between.
        ("branch", vec![inst("BRA8", vec![imm(2)])]),
        ("argument", vec![crate::callpush::push_of(&r(reg::D0))]),
        ("pop", vec![inst("MOV32do", vec![r(reg::D0), r(reg::SP)])]),
        ("float_add", libcall(4, 2)),
        ("float_multiply", libcall(4, 2)),
        ("float_divide", libcall(4, 2)),
        ("float_compare", libcall(4, 0)),
        ("float_to_int", libcall(2, 1)),
        ("float_load", vec![inst("MOV32dp", vec![r(reg::D0), imm(4), r(reg::A0)]), inst("MOV32dp", vec![r(reg::D1), imm(4), r(reg::A0)])]),
        ("float_store", vec![inst("MOV32pd", vec![imm(4), r(reg::A0), r(reg::D0)]), inst("MOV32pd", vec![imm(4), r(reg::A0), r(reg::D1)])]),
        // Every function: one callee-saved register saved and restored.
        ("function", vec![crate::callpush::push_of(&r(reg::D2)), inst("MOV32do", vec![r(reg::D2), r(reg::SP)])]),
    ]
}

/// The fields `size_costs` leaves at `OperationCosts::default()`, each with why: no such operation on m68k, or a
/// tuning number that is not the size of an instruction. A field equal to its default that is not here is one nobody
/// priced (`tests/opcosts.rs` fails on it).
pub const AT_DEFAULT: &[(&str, &str)] = &[
    ("argument", "2: a register push, which is what the default says; also a form"),
    ("prefix", "m68k has no operand-size prefix"),
    ("width_scaled", "false: the 32-bit word is native, one price per operation (native_bits 0)"),
    ("constant_offsets_fold", "false: a constant GEP is priced as `address`; the displacement folds into the access, so this overprices (unmeasured, open)"),
    ("branch_in_compare", "false: `branch` is priced each; the compare's price does not include the jump (unmeasured, open)"),
    ("direct_load", "0: an absolute load is priced as any load (unmeasured, open)"),
    ("direct_store", "0: an absolute store is priced as any store (unmeasured, open)"),
    ("ext_tsp_for_size", "false: the pin ships -apply-ext-tsp-for-size off"),
    ("native_bits", "0: a price is for the operation as the type has it, as on every target but the 6502"),
    ("address_dword", "m68k states no address-size prefix; the price is `address` alone"),
    ("address_third", "same"),
    ("return_pops", "the caller pops its arguments"),
    ("carry", "no segmented or huge pointers"),
    ("carry_step", "no segmented or huge pointers"),
    ("multiply_high", "constant divides use the multiply-long sequence priced above"),
    ("divide_margin", "no isIntDivCheap margin stated"),
    ("float_load_folded", "soft float folds no load into an operation (folds_loads is false: unread)"),
    ("folds_loads", "same"),
    ("float_release", "no floating register stack"),
    ("fill", "no string instruction: a memset is stores or a library call (loop idioms, own row)"),
    ("fill_cell", "same"),
    ("fill_segment", "no segment registers"),
    ("memset_stores", "a tuning limit, not an instruction size"),
    ("huge_fill", "no huge pointers"),
    ("huge_copy", "no huge pointers"),
    ("copy", "no string instruction (loop idioms, own row)"),
    ("copy_cell", "same"),
    ("copy_variable", "same"),
    ("direction", "no direction flag"),
    ("unroll_budget", "a tuning limit"),
    ("pieces_set", "a tuning limit"),
    ("pieces_clear", "a tuning limit"),
    ("pieces_set_size", "a tuning limit"),
    ("pieces_clear_size", "a tuning limit"),
    ("memset_store", "a tuning limit"),
    ("jump_table", "a tuning limit (switch lowering, own row)"),
    ("cache_line", "set by the CPU table"),
    ("line_fill_clocks", "set by the CPU table"),
];

/// What the lowering adds that no IR operation names (spills, branch relaxation, copies the allocator leaves), as a
/// scale and a per-function constant: `(operations + function) * scale`. Fit (`tools/m68k_size_fit.py --corpus --dump`,
/// 2026-11-09, -O2 and -Os) on the functions of `tests/run/c` WITHOUT the 22 fixtures of `tests/data_size`, graded on
/// the 124 functions of those fixtures (disjoint): one median scale of 1.50 priced large functions 30% over. The fit
/// surface is flat (1.20 + 3 B is within 0.005 of the best pair, median |error| 0.123 on 1826 functions); on the
/// fixtures the median |error| is 0.25 (68000) and 0.24 (68020) against 0.41 and 0.32 at 1.50, aggregate 1.03 against
/// 1.24.
const FUNCTION_SCALE_PERCENT: i64 = 120;

/// Bytes added to every function's estimate beyond the saved register (see `FUNCTION_SCALE_PERCENT`): the frame and
/// returns the lowering emits that no operation names.
pub const FUNCTION_RESIDUE_BYTES: i64 = 3;

/// The bytes of an `i64` product and of the product of two `i32`s into an `i64`, the whole sequence: the object of
/// `long long f(long long a, long long b) { return a * b; }` and of `(long long)a * b` at -Os (2026-11-09: 152 and 178
/// B at the 68000, 44 and 14 B at the 68020) less the saved registers, `rts` and loads around the product
/// (`tests/size_calibration.rs` holds them to the objects). The 68000 has no 32-bit multiply either: each is a run of
/// `__mulsi3` calls on 16-bit halves; the 68020 has `muls.l`/`mulu.l`.
fn products(cpu: Cpu) -> (i64, i64) {
    if cpu.at_least(Cpu::M68020) { (34, 8) } else { (136, 160) }
}

/// The operations in code bytes at `cpu`.
pub fn size_costs(cpu: Cpu) -> Result<OperationCosts, String> {
    let (wide, widening) = products(cpu);
    let mut costs = OperationCosts {
        function_scale: FUNCTION_SCALE_PERCENT,
        free_intrinsics: true,
        expanded_intrinsics: true,
        multiply_wide: wide,
        multiply_widening: widening,
        ..OperationCosts::default()
    };
    for (class, form) in forms(cpu) {
        let size = bytes(&form)?;
        match class {
            "add" => costs.add = size,
            "shift" => costs.shift = size,
            "multiply" => (costs.multiply, costs.multiply_long) = (size, size),
            "divide" => (costs.divide, costs.divide_long) = (size, size),
            "address" => costs.address = size,
            "load" => costs.load = size,
            "store" => costs.store = size,
            "store_immediate" => costs.store_immediate = size,
            "memory_update" => costs.memory_update = size,
            "move" => costs.r#move = size,
            "call" => costs.call = size,
            "return_" => costs.return_ = size,
            "adjust" => costs.adjust = size,
            "extend" => costs.extend = size,
            "set" => costs.set = size,
            "branch" => costs.branch = size,
            "argument" => costs.argument = size,
            "pop" => costs.pop = size,
            "float_add" => costs.float_add = size,
            "float_multiply" => costs.float_multiply = size,
            "float_divide" => costs.float_divide = size,
            "float_compare" => costs.float_compare = size,
            "float_to_int" => costs.float_to_int = size,
            "float_load" => costs.float_load = size,
            "float_store" => costs.float_store = size,
            "function" => costs.function = size + FUNCTION_RESIDUE_BYTES,
            other => return Err(format!("no field for {other}")),
        }
    }
    Ok(costs)
}
