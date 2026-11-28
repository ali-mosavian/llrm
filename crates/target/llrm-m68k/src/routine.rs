//! Clocks of the runtime routine a library multiply calls, which a function's
//! own instructions do not show.
//!
//! `__mulsi3` is `runtime/compiler-rt/config/m68k/mulsi3.s`: at the 68000 and
//! 68010 one `mulu.w` when both operands are under 64 K and three otherwise,
//! at the 68020 and later one `mulu.l`. Its clocks do not depend on the order
//! of the operands. (It was a shift-add loop whose clocks ran with the bit
//! length of its second operand: quicksort 68000 -O2 read 1.89 M or 2.61 M
//! clocks for the same sums by the order of the call.)
//!
//! Measured 2026-11-11 with `cycles` (CYCLES_FUNCS=main,__mulsi3): 100 calls of
//! `__mulsi3(K, K)` in a loop, K = 0, 1, 65535 (narrow), 65537, 2^32 - 1
//! (wide), less the same loop around a bare `rts`, plus that `rts` (16 clocks
//! at the 68000 and 68010, 6 at the 68020). The tables price a `mulu.w` at its
//! published maximum (38 + 2 x 16 clocks), so the table below takes 32 off
//! each `mulu.w` for the floor a trial counts:
//!
//! | CPU   | narrow (max) | narrow (floor) | wide (max) | wide (floor) |
//! |-------|--------------|----------------|------------|--------------|
//! | 68000 | 138          | 106            | 340        | 244          |
//! | 68010 | 106          | 74             | 248        | 152          |
//! | 68020 | 61           | 61             | 61         | 61           |
//!
//! The price of a call site counts the routine at its narrow floor, a lower
//! bound: the chain it competes with costs 20 to 70 clocks, so a trial never
//! keeps a library call for its clocks. Owner m68k-b. Exit 2026-12-20: the
//! constant read from the call's pushes.

/// FNV-1a of the routine's source file; the test below fails when it changes,
/// and the table above is measured again.
pub const MULSI3_SOURCE_STAMP: u64 = 0x01ea_8089_cc03_1f49;

/// `(narrow, wide)` floors of the routine on `cpu`; `None` where no table was
/// measured (the 68030 and later, where a multiply is an instruction).
fn fit(cpu: crate::dagisel::hooks::Cpu) -> Option<(u32, u32)> {
    use crate::dagisel::hooks::Cpu;
    match cpu {
        Cpu::M68000 => Some((106, 244)),
        Cpu::M68010 => Some((74, 152)),
        Cpu::M68020 => Some((61, 61)),
        Cpu::M68030 | Cpu::M68040 | Cpu::M68060 => None,
    }
}

/// Floor clocks of one `__mulsi3(a, b)` call on `cpu`, `bits` the bit length of
/// `b` (0 for 0, 32 for a negative `b`): the narrow path unless `b` alone
/// needs the wide one; 0 where no table was measured.
pub fn mulsi3_clocks(
    cpu: crate::dagisel::hooks::Cpu,
    bits: u32,
) -> u32 {
    match fit(cpu) {
        Some((narrow, wide)) => {
            if bits > 16 {
                wide
            } else {
                narrow
            }
        }
        None => 0,
    }
}

/// Bytes of `__mulsi3` a program that calls it links (assembled `mulsi3.s`,
/// 2026-11-15): 54 at the 68000 and 68010, 12 from the 68020. A trial counts
/// them against a build that calls the routine and another that does not.
pub fn mulsi3_bytes(cpu: crate::dagisel::hooks::Cpu) -> usize {
    if cpu.at_least(crate::dagisel::hooks::Cpu::M68020) { 12 } else { 54 }
}

/// Whether a function of `module` other than `except` multiplies two variables,
/// which a CPU without a long multiply calls `__mulsi3` for: the routine is
/// then linked whatever a trial chooses for `except` (its own calls show in its
/// items; two narrow operands are a `muls`, so this may say yes where the other
/// function does not call).
pub fn another_function_multiplies_variables(
    module: &llrm_mir::module::Module,
    except: &str,
    cpu: crate::dagisel::hooks::Cpu,
) -> bool {
    use llrm_mir::module::Operand;
    use llrm_mir::opcode::{BinaryOp, Opcode};
    if cpu.at_least(crate::dagisel::hooks::Cpu::M68020) {
        return false;
    }
    module
        .functions()
        .filter(|(_, global, function)| !function.is_declaration() && global.name.owned().as_deref() != Some(except))
        .any(|(_, _, function)| {
            function
                .walk()
                .any(
                    |(_, inst)| {
                        let instruction = function.instruction(inst);
                        matches!(instruction.opcode, Opcode::Binary(BinaryOp::Mul))
                            && !instruction.operands.iter().any(|operand| matches!(operand, Operand::Constant(_)))
                            && !is_halfword_product(module, function, instruction)
                    },
                )
        })
}

/// Whether `instruction`, a multiply, is the product of two operands each a
/// sign (or each a zero) extension from at most 16 bits: `muls.w` or `mulu.w`,
/// which never calls `__mulsi3` (`(long) raw * sval` in `qlight`, whose
/// `* 1000000L` then kept the call and linked the routine for nothing else).
fn is_halfword_product(
    module: &llrm_mir::module::Module,
    function: &llrm_mir::module::Function,
    instruction: &llrm_mir::module::Instruction,
) -> bool {
    use llrm_mir::module::{Operand, ValueDef};
    use llrm_mir::opcode::{CastOp, Opcode};
    let extension = |operand: &Operand| {
        let Operand::Value(value) = operand else { return None };
        let ValueDef::Instruction(def) = function.value(*value).def else { return None };
        let cast = function.instruction(def);
        let Opcode::Cast(op @ (CastOp::SExt | CastOp::ZExt)) = cast.opcode else { return None };
        let from = function.operand_type(&module.context, cast.operands[0])?;
        module.context.types.int_bits(from).filter(|&bits| bits <= 16).map(|_| op)
    };
    match instruction.operands.as_slice() {
        [left, right] => extension(left).is_some() && extension(left) == extension(right),
        _ => false,
    }
}

/// The bytes of `__mulsi3` a trial charges the build of `except` that calls it:
/// none where another function multiplies variables (the routine is linked
/// anyway), else the routine shared by this function and every other
/// that multiplies or divides by a constant, which may call it too. Each of six
/// functions of `urem_eq` saw the whole 54 B saved by its own chain and took
/// it: +64 B for the module, the routine still saved only once.
pub fn mulsi3_share(
    module: &llrm_mir::module::Module,
    except: &str,
    cpu: crate::dagisel::hooks::Cpu,
    price: crate::timing::Price,
) -> usize {
    use llrm_mir::module::Operand;
    use llrm_mir::opcode::{BinaryOp, Opcode};
    if cpu.at_least(crate::dagisel::hooks::Cpu::M68020) || another_function_multiplies_variables(module, except, cpu) {
        return 0;
    }
    // where clocks count too, the whole routine is the build's, as before:
    // sharing it flipped `urem_eq` -O1 to the calls, 8.4 M to 12.9 M clocks
    if price != crate::timing::Price::Size {
        return mulsi3_bytes(cpu);
    }
    let by_constant = module
        .functions()
        .filter(|(_, global, function)| !function.is_declaration() && global.name.owned().as_deref() != Some(except))
        .filter(|(_, _, function)| {
            function.walk().any(|(_, inst)| {
                let instruction = function.instruction(inst);
                matches!(
                    instruction.opcode,
                    Opcode::Binary(BinaryOp::Mul | BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::URem | BinaryOp::SRem)
                ) && instruction.operands.iter().any(|operand| matches!(operand, Operand::Constant(_)))
            })
        })
        .count();
    mulsi3_bytes(cpu) / (1 + by_constant)
}

/// [`mulsi3_clocks`] with `b` = 1: the floor a trial counts for a call.
pub fn mulsi3_floor(cpu: crate::dagisel::hooks::Cpu) -> u32 {
    mulsi3_clocks(cpu, 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The source of `__mulsi3`.
    fn body() -> String {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../runtime/compiler-rt/config/m68k/mulsi3.s");
        std::fs::read_to_string(path).expect("the runtime source")
    }

    /// A trial priced `jsr __mulsi3` as the `jsr` alone (18 clocks) against a
    /// routine of 162 or more, so the library call looked cheaper than the
    /// chain it replaces. The table here is the routine's as measured; this
    /// fails when the routine changes and the numbers are stale.
    #[test]
    fn the_measured_routine_is_the_shipped_one() {
        let hash = body()
            .bytes()
            .fold(0xcbf2_9ce4_8422_2325u64, |h, byte| (h ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3));
        assert_eq!(hash, MULSI3_SOURCE_STAMP, "__mulsi3 changed: measure the table in routine.rs again");
    }

    /// The floors are the measured routine less 32 clocks a `mulu.w`.
    #[test]
    fn the_fit_is_the_measured_table() {
        use crate::dagisel::hooks::Cpu;
        for (cpu, narrow, wide) in
            [(Cpu::M68000, 138 - 32, 340 - 96), (Cpu::M68010, 106 - 32, 248 - 96), (Cpu::M68020, 61, 61)]
        {
            assert_eq!((mulsi3_clocks(cpu, 1), mulsi3_clocks(cpu, 17)), (narrow, wide), "{cpu:?}");
        }
    }

    #[test]
    fn a_call_costs_more_than_any_chain() {
        use crate::dagisel::hooks::Cpu;
        assert!([Cpu::M68000, Cpu::M68010].iter().all(|cpu| mulsi3_floor(*cpu) > 70), "the dearest chain is 70");
    }
}
