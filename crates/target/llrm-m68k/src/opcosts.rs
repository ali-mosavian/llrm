//! What an IR operation costs in code bytes on m68k: the length of the
//! instructions it becomes, from the encoder (the one source of a form's
//! length; `tests/opcosts.rs` assembles each form with binutils and compares).
//!
//! `Machine::size_costs` was `OperationCosts::default()`, one per operation,
//! so every byte count a size decision read from it was an instruction count.
//! The 68000 has no 32-bit multiply or divide: those are a library call (two
//! pushes, `jsr`, the cleanup); the 68020 has `muls.l` and `divs.l`.
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
    ]
}

/// The operations in code bytes at `cpu`.
pub fn size_costs(cpu: Cpu) -> Result<OperationCosts, String> {
    let mut costs = OperationCosts::default();
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
            other => return Err(format!("no field for {other}")),
        }
    }
    Ok(costs)
}
