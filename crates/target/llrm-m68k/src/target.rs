//! m68k as an `llrm_target::Target`: its descriptions read through the same
//! parsers as every target's. What the x86 routines (huge pointers, string
//! registers, the divide helper) ask is refused or defaulted, as the target has
//! no such thing.

use std::sync::LazyLock;

use llrm_lir::registers::{Info, RegId};
use llrm_mir::target::{AddressForm, OperationCosts};
use llrm_target::calling::{Calling, Convention};
use llrm_target::instr::Dialect;
use llrm_target::layout::Layout;
use llrm_target::machine::Machine;
use llrm_target::object::ObjectFormat;
use llrm_target::{CostModel, Target, routines, timings};

use crate::{REGISTER_INFO, abi};

const CPUS: &[&str] = &["m68000", "m68010", "m68020", "m68030", "m68040", "m68060"];

static TIMINGS: LazyLock<timings::Timings> =
    LazyLock::new(|| timings::Timings::parse(include_str!("timings.times")).expect("m68k timings.times parses"));

/// The pin's register pressure sets: `pressure.sets`.
static PRESSURE: LazyLock<llrm_target::pressure::PressureSets> =
    LazyLock::new(|| llrm_target::pressure::parse(include_str!("pressure.sets")).expect("pressure.sets parses"));

static REGISTERS: LazyLock<Vec<llrm_target::registers::Register>> =
    LazyLock::new(|| llrm_target::registers::parse(include_str!("registers.regs")).expect("registers.regs parses"));

/// The limit of every pressure set here (`computePSetLimit`): a register
/// counts if its root is a `gpr`; `a6` and `sp` never.
///
/// DEVIATE M68K-LICM-LIMIT (owner wasm-3, exit 2026-12-10, when
/// early_exit_index 68000 -O2/-O3 is under gcc with the limit at 9): the pin
/// counts `a6` in a function with no frame pointer (`getReservedRegs`, AR16
/// limit 9). With 9 the shared `MachineLICM` hoists one more value than ours
/// keeps in a register: early_exit_index 68000 -O2/-O3 +4 B (over gcc),
/// early_exit_widths +18 B, with `%a6` allocatable either way.
pub fn pressure_limits() -> Vec<u32> {
    (0..PRESSURE.sets.len())
        // A set whose largest class the table leaves out holds no value
        // here: its raw limit stands.
        .map(|set| {
            if PRESSURE.class(&PRESSURE.sets[set].largest).is_none() {
                return PRESSURE.sets[set].limit;
            }
            PRESSURE.limit(set, |name| {
                REGISTERS
                    .iter()
                    .find(|one| one.name == name)
                    .and_then(|one| REGISTERS.iter().find(|root| root.name == one.root))
                    .is_some_and(|root| root.is("gpr"))
            })
        })
        .collect()
}

/// The pin's `MCInstrDesc` of the opcode `name`.
fn described(name: &str) -> Option<&'static llrm_dagisel::instr::InstrDesc> {
    crate::dagisel::local::code_of(name).and_then(crate::dagisel::local::desc)
}

/// The CPU table of `name`: the price of a byte of code each CPU is judged by.
pub fn cpu_table(name: &str) -> Option<timings::CpuTable> {
    TIMINGS.cpu(name).cloned()
}

static LAYOUT: LazyLock<Layout> =
    LazyLock::new(|| Layout::parse(include_str!("machines/datalayout.toml")).expect("m68k datalayout.toml parses"));
static CALLING: LazyLock<Calling> =
    LazyLock::new(|| Calling::parse(include_str!("machines/calling.toml")).expect("m68k calling.toml parses"));
pub(crate) static DIALECT: LazyLock<Dialect> =
    LazyLock::new(|| Dialect::parse(include_str!("instructions/dialect.toml")).expect("m68k dialect.toml parses"));

/// The m68k target.
pub struct M68k;

/// m68k's object writers: ELF32 big-endian (`crate::elf`); the static
/// executable is `crate::object`'s.
struct M68kObjects;

impl llrm_object::ObjectTarget for M68kObjects {
    fn write_object(
        &self,
        format: &str,
        object: &llrm_object::Object,
    ) -> Result<Vec<u8>, llrm_object::Unsupported> {
        match format {
            "elf" => crate::elf::write(object),
            _ => Err(llrm_object::Unsupported(format!("m68k has no {format} object writer"))),
        }
    }

    fn asm_info(
        &self,
        format: &str,
    ) -> Option<llrm_object::AsmInfo> {
        (format == "elf").then(|| llrm_object::AsmInfo { hosted: true, ..llrm_object::AsmInfo::default() })
    }
}

impl llrm_debug::DebugObjectTarget for M68kObjects {
    fn write_described(
        &self,
        format: &str,
        object: &llrm_object::Object,
        _debug: Option<&llrm_debug::Info>,
    ) -> Result<Vec<u8>, llrm_object::Unsupported> {
        llrm_object::ObjectTarget::write_object(self, format, object)
    }

    fn debug_caps(
        &self,
        _format: &str,
        _asked: llrm_debug::Format,
    ) -> llrm_debug::DebugCaps {
        llrm_debug::DebugCaps::default()
    }
}

impl M68k {
    fn convention() -> &'static Convention {
        &CALLING.conventions[0]
    }
}

impl Target for M68k {
    fn name(&self) -> &'static str {
        "m68k"
    }

    /// `SWAP` is `isAsCheapAsAMove` in the pin (`M68kInstrShiftRotate.td`): the
    /// shared `MachineCSE`, `MachineSink` and `MachineLICM` ask it.
    fn cheap_move_row(&self) -> fn(&str) -> bool {
        |row| row == "SWAP"
    }

    /// The pin hoists the `lea` of a symbol out of a loop: `word_array_abs_x`
    /// -O2 ran two a trip (+12% clocks at 68020 against gcc).
    fn hoists_register_free_ops(&self) -> bool {
        true
    }

    fn objects(&self) -> &'static dyn llrm_debug::DebugObjectTarget {
        &M68kObjects
    }

    fn debug_target(&self) -> llrm_debug::DebugTarget {
        llrm_debug::DebugTarget::of(32)
    }

    fn machine(&self) -> Machine {
        Machine::parse(include_str!("machines/platform.toml"), self.default_cpu())
            .expect("m68k platform.toml parses")
            .with_layout(self.layout())
    }

    fn cpus(&self) -> &'static [&'static str] {
        CPUS
    }

    fn march(
        &self,
        name: &str,
    ) -> Option<&'static str> {
        CPUS.iter().copied().find(|cpu| *cpu == name)
    }

    fn marches(&self) -> Vec<&'static str> {
        CPUS.to_vec()
    }

    fn layout(&self) -> Layout {
        LAYOUT.clone()
    }

    fn stack_slot_bytes(&self) -> i64 {
        Self::convention().slot_bytes
    }

    fn frame_register(&self) -> RegId {
        REGISTER_INFO.frame
    }

    fn first_argument_offset(
        &self,
        _far: bool,
    ) -> i64 {
        Self::convention().first_argument_offset
    }

    fn return_address_bytes(
        &self,
        _far: bool,
    ) -> i64 {
        Self::convention().return_address_bytes
    }

    fn registers(&self) -> &'static Info {
        &REGISTER_INFO
    }

    fn results(
        &self,
        width: u32,
    ) -> Vec<RegId> {
        self.convention_results(Self::convention(), width)
    }

    fn stack_pointer(&self) -> RegId {
        REGISTER_INFO.stack
    }

    fn callee_saved(&self) -> Vec<(RegId, RegId)> {
        self.convention_callee_saved(Self::convention())
    }

    fn register_named(
        &self,
        name: &str,
    ) -> RegId {
        abi::register(name)
    }

    fn convention_results(
        &self,
        convention: &Convention,
        width: u32,
    ) -> Vec<RegId> {
        convention
            .result_registers(i64::from(width))
            .map(|names| names.iter().map(|name| abi::register(name)).collect())
            .unwrap_or_default()
    }

    fn convention_callee_saved(
        &self,
        convention: &Convention,
    ) -> Vec<(RegId, RegId)> {
        convention.preserved.iter().map(|kept| (abi::register(&kept.full), abi::register(&kept.pushed))).collect()
    }

    fn cpu_table(
        &self,
        name: &str,
    ) -> Option<timings::CpuTable> {
        cpu_table(name)
    }

    fn registers_text(&self) -> String {
        include_str!("registers.regs").to_owned()
    }

    fn forms_text(&self) -> String {
        include_str!("instructions/m68k.instr").to_owned()
    }

    fn dialect(&self) -> &'static Dialect {
        &DIALECT
    }

    fn operand_bytes(&self) -> i64 {
        4
    }

    fn huge_fill_routine(
        &self,
        _cell: u32,
    ) -> Result<routines::HugeString, String> {
        Err("m68k has no huge pointers".to_owned())
    }

    fn huge_copy_routine(
        &self,
        _down: bool,
    ) -> Result<routines::HugeString, String> {
        Err("m68k has no huge pointers".to_owned())
    }

    fn divide_routine(
        &self,
        _signed: bool,
        _wide_divisor: bool,
    ) -> Result<routines::Divide, String> {
        Err("m68k divides inline or by a library call".to_owned())
    }

    fn float_width(
        &self,
        bytes: u32,
    ) -> u32 {
        bytes
    }

    fn register_op_bytes(
        &self,
        _width: i64,
    ) -> i64 {
        2
    }

    fn shift_op_bytes(
        &self,
        _count: i64,
        _width: i64,
    ) -> i64 {
        2
    }

    fn multiply_immediate_bytes(
        &self,
        _number: i64,
        _width: i64,
    ) -> i64 {
        4
    }

    fn default_cpu(&self) -> &'static str {
        CPUS[0]
    }

    fn operation_costs(
        &self,
        _price: &dyn Fn(&str) -> i64,
        _prefix: i64,
    ) -> OperationCosts {
        OperationCosts::default()
    }

    fn register_capacity(&self) -> i64 {
        llrm_target::registers::allocatable(
            &llrm_target::registers::parse(include_str!("registers.regs")).expect("registers.regs parses"),
        ) as i64
    }

    fn pressure(&self) -> Option<(&'static llrm_target::pressure::PressureSets, Vec<u32>)> {
        Some((&PRESSURE, pressure_limits()))
    }

    /// An opcode this target adds above the pin's table is not described: it
    /// counts as flagged.
    fn rematerializable_opcode(&self) -> fn(&str) -> bool {
        |name| {
            described(name)
                .is_none_or(|desc| desc.flags & crate::dagisel::generated::instrinfo::flag::REMATERIALIZABLE != 0)
        }
    }

    fn def_register_class(&self) -> fn(&str) -> Option<&'static str> {
        |name| {
            use crate::dagisel::generated::ids::reg;
            let desc = described(name)?;
            let operand = desc.operands.first().filter(|_| desc.num_defs == 1)?;
            match u16::try_from(operand.reg_class).ok()? {
                reg::AR32RegClassID => Some("AR32"),
                reg::DR32RegClassID => Some("DR32"),
                reg::XR32RegClassID => Some("XR32"),
                reg::AR16RegClassID => Some("AR16"),
                reg::DR16RegClassID => Some("DR16"),
                reg::DR8RegClassID => Some("DR8"),
                reg::XR16RegClassID => Some("XR16"),
                _ => None,
            }
        }
    }

    fn address_forms(
        &self,
        _costs: &OperationCosts,
        _address_stall: i64,
    ) -> Vec<AddressForm> {
        Vec::new()
    }

    fn cost_model(&self) -> CostModel {
        |prices| llrm_target::described_with(prices, None, None, None)
    }

    fn calling(&self) -> &'static Calling {
        &CALLING
    }

    fn conventions(&self) -> &'static [&'static str] {
        static NAMES: LazyLock<Vec<&'static str>> =
            LazyLock::new(|| CALLING.conventions.iter().map(|one| one.name.as_str()).collect());
        &NAMES
    }

    fn object(&self) -> ObjectFormat {
        ObjectFormat::parse(include_str!("machines/object.toml")).expect("m68k object.toml parses")
    }
}

#[cfg(test)]
mod stack_facts_tests {
    /// The pin's string, from its `opt -mtriple=m68k-unknown-linux-gnu`: S16 is
    /// two bytes, the frames keep four (M68K-STACK-ALIGN).
    const PIN: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

    /// An `S` above what the frames keep lets a pass raise an alloca past the
    /// frame's alignment: silent misalignment.
    #[test]
    fn the_datalayout_states_a_stack_alignment_the_frames_keep_and_the_pin_has() {
        llrm_target::testing::assert_stack_facts(
            "m68k",
            crate::kernels::DATALAYOUT,
            crate::frame::SHIPPED_STACK_ALIGN as u64,
            Some(PIN),
        );
    }

    /// The check itself: an `S` above the kept alignment must be refused, so a
    /// green run above means something.
    #[test]
    #[should_panic(expected = "S above what the frames keep")]
    fn a_stack_alignment_above_what_the_frames_keep_is_refused() {
        llrm_target::testing::assert_stack_facts("m68k", crate::kernels::DATALAYOUT, 1, Some(PIN));
    }

    #[test]
    fn the_toml_and_the_driver_state_one_datalayout() {
        let toml = include_str!("machines/datalayout.toml");
        let stated = toml.lines().find_map(|line| line.strip_prefix("datalayout = \"")?.strip_suffix('"')).unwrap();
        assert_eq!(stated, crate::kernels::DATALAYOUT);
    }
}
