//! What an IR operation costs in clocks on m68k: the clocks of the instructions
//! it becomes (the forms of [`crate::opcosts::forms`]), from the manuals'
//! tables ([`crate::timing`]).
//!
//! DEVIATE M68K-CLOCKS (owner m68k, exit 2026-12-20): the pin's M68k has no
//! `TargetTransformInfo`, so `BasicTTIImpl` prices every operation at
//! `TCC_Basic` (1) and the decisions that read a cost compare instruction
//! counts. This target's `OperationCosts` channel is its own; filling it with
//! clocks is the deviation, removed when the decisions that read it are
//! translated against the pin's costs.
//!
//! `Machine::costs` was `OperationCosts::default()`: one clock an operation, so
//! a clock decision (unroll, inline, if-conversion) priced a library multiply
//! at a `jsr` and a loop's saving at its instruction count against bytes
//! that were real (`logical_value` -O3: 17 clocks saved for 54 bytes added, a
//! 3-trip loop around a 160-clock `__mulsi3`, kept rolled).
//!
//! A branch is priced taken (a loop's back edge and the branch a decision
//! removes). An operation that is a library call adds the routine's clocks: a
//! 32-bit multiply on a CPU without the instruction from [`crate::routine`]
//! (m68k-b's table, the one of `__mulsi3`) for a multiplier of
//! [`MULTIPLIER_BITS`]; the divide and the soft-float operations
//! from [`MEASURED`], each the mean over the operand sets it was run with (the
//! sets differ by 3x: `__divsi3` 832 to 2216 clocks at the 68000, and
//! `__muldf3` 6616 to 18808).

use llrm_mir::target::OperationCosts;

use crate::dagisel::hooks::Cpu;
use crate::mc::Inst;

/// Clocks per call of the shipped routines, measured 2026-11-09 with `cycles
/// --exe` (every executed instruction outside `main`, per call of 64) on
/// `llrm-cc -O2` objects of `runtime/compiler-rt` at each CPU, the tables of
/// [`crate::timing`] with the data-dependent terms at their bound (a multiply
/// or divide instruction at its maximum, a shift by a register at no places):
/// one operand set a value. `divide` is the `__divsi3`/`__udivsi3`/`__modsi3`/
/// `__umodsi3` sets (n = 1e6, 65535, 1e9, 100 over d = 10, 255, 1000, 7), kept
/// for the CPUs without `divs.l`. `add` is `__adddf3` and `__subdf3`,
/// `multiply` `__muldf3`, `divide_double` `__divdf3`, `compare` `__ltdf2`,
/// `to_int` `__fixdfsi`. Re-measure when [`SOURCE_STAMP`] fails.
pub struct Measured {
    pub cpu: Cpu,
    pub divide: &'static [u32],
    pub add: &'static [u32],
    pub multiply: &'static [u32],
    pub divide_double: &'static [u32],
    pub compare: &'static [u32],
    pub to_int: &'static [u32],
}

pub const MEASURED: &[Measured] = &[
    Measured {
        cpu: Cpu::M68000,
        divide: &[1846, 634, 2022, 634, 2040, 832, 2216, 832, 1862, 650, 1972, 764],
        add: &[2220, 2208, 2616, 2116],
        multiply: &[6616, 18808],
        divide_double: &[20462, 17742],
        compare: &[1284, 956],
        to_int: &[1022, 608],
    },
    Measured {
        cpu: Cpu::M68010,
        divide: &[1734, 568, 1900, 568, 1916, 752, 2082, 752, 1750, 584, 1852, 688],
        add: &[2138, 2128, 2502, 2026],
        multiply: &[6310, 16990],
        divide_double: &[19238, 16692],
        compare: &[1220, 926],
        to_int: &[960, 584],
    },
    Measured {
        cpu: Cpu::M68020,
        divide: &[],
        add: &[998, 992, 1173, 948],
        multiply: &[2260, 2260],
        divide_double: &[9020, 8051],
        compare: &[604, 464],
        to_int: &[495, 300],
    },
    Measured {
        cpu: Cpu::M68030,
        divide: &[],
        add: &[934, 928, 1106, 898],
        multiply: &[2163, 2163],
        divide_double: &[8597, 7743],
        compare: &[575, 429],
        to_int: &[461, 261],
    },
    Measured {
        cpu: Cpu::M68040,
        divide: &[],
        add: &[333, 329, 402, 332],
        multiply: &[789, 789],
        divide_double: &[3141, 2880],
        compare: &[218, 159],
        to_int: &[169, 94],
    },
    Measured {
        cpu: Cpu::M68060,
        divide: &[],
        add: &[234, 232, 276, 232],
        multiply: &[387, 387],
        divide_double: &[2225, 1954],
        compare: &[156, 120],
        to_int: &[112, 66],
    },
];

/// FNV-1a over `runtime/compiler-rt/lib/int/int32.c`, `lib/fp/*.inc` and
/// `lib/fp/f64/*` (each sorted by name, contents concatenated): the sources of
/// the routines [`MEASURED`] ran. `tests/clock_costs.rs` fails when it changes.
pub const SOURCE_STAMP: u64 = 0xc66f_2499_531f_0ef8;

fn mean(clocks: &[u32]) -> i64 {
    if clocks.is_empty() { 0 } else { clocks.iter().map(|one| i64::from(*one)).sum::<i64>() / clocks.len() as i64 }
}

fn measured(cpu: Cpu) -> &'static Measured {
    MEASURED.iter().find(|row| row.cpu == cpu).expect("a row for every CPU of the family")
}

/// The bit length of a multiplier that is not known: the `i` of `a[i] * 27` and
/// of a field's scale is a byte or less, so the routine's clocks are counted at
/// 8 bits (the 68000: 624).
pub const MULTIPLIER_BITS: u32 = 8;

/// The clocks of `form` at `cpu`, a branch taken.
fn clocks(
    cpu: Cpu,
    form: &[Inst],
) -> Result<i64, String> {
    form.iter().try_fold(0, |sum, one| {
        let bytes = crate::encode::encode(one)?.bytes.len() as u32;
        let text = crate::mc::print(one)?;
        Ok(sum + i64::from(crate::timing::cost(cpu, &text, bytes, true)?.clocks))
    })
}

/// The operations in clocks at `cpu`.
pub fn clock_costs(cpu: Cpu) -> Result<OperationCosts, String> {
    let mut costs = OperationCosts::default();
    // The routine is called only where there is no instruction.
    let routine =
        if cpu.at_least(Cpu::M68020) { 0 } else { i64::from(crate::routine::mulsi3_clocks(cpu, MULTIPLIER_BITS)) };
    for (class, form) in crate::opcosts::forms(cpu) {
        let clock = clocks(cpu, &form)?;
        match class {
            "add" => costs.add = clock,
            "shift" => costs.shift = clock,
            "multiply" => (costs.multiply, costs.multiply_long) = (clock + routine, clock + routine),
            "divide" => {
                (costs.divide, costs.divide_long) =
                    (clock + mean(measured(cpu).divide), clock + mean(measured(cpu).divide))
            }
            "address" => costs.address = clock,
            "load" => costs.load = clock,
            "store" => costs.store = clock,
            "store_immediate" => costs.store_immediate = clock,
            "memory_update" => costs.memory_update = clock,
            "move" => costs.r#move = clock,
            "call" => costs.call = clock,
            "return_" => costs.return_ = clock,
            "adjust" => costs.adjust = clock,
            "extend" => costs.extend = clock,
            "set" => costs.set = clock,
            "branch" => costs.branch = clock,
            "argument" => costs.argument = clock,
            "pop" => costs.pop = clock,
            "float_add" => costs.float_add = clock + mean(measured(cpu).add),
            "float_multiply" => costs.float_multiply = clock + mean(measured(cpu).multiply),
            "float_divide" => costs.float_divide = clock + mean(measured(cpu).divide_double),
            "float_compare" => costs.float_compare = clock + mean(measured(cpu).compare),
            "float_to_int" => costs.float_to_int = clock + mean(measured(cpu).to_int),
            "float_load" => costs.float_load = clock,
            "float_store" => costs.float_store = clock,
            "function" => costs.function = clock,
            other => return Err(format!("no field for {other}")),
        }
    }
    Ok(costs)
}
