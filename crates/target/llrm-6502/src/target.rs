//! mos as an `llrm_target::Target`: its descriptions read through the same
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

const CPUS: &[&str] = &["mos6502", "w65c02"];

static LAYOUT: LazyLock<Layout> =
    LazyLock::new(|| Layout::parse(include_str!("machines/datalayout.toml")).expect("mos datalayout.toml parses"));
static CALLING: LazyLock<Calling> =
    LazyLock::new(|| Calling::parse(include_str!("machines/calling.toml")).expect("mos calling.toml parses"));
static DIALECT: LazyLock<Dialect> =
    LazyLock::new(|| Dialect::parse(include_str!("instructions/dialect.toml")).expect("mos dialect.toml parses"));

/// The mos target.
pub struct Mos;

impl Mos {
    fn convention() -> &'static Convention {
        &CALLING.conventions[0]
    }
}

/// The 6502 writes assembly for ca65, not objects: every format is
/// unsupported.
struct MosObjects;

impl llrm_object::ObjectTarget for MosObjects {
    fn write_object(
        &self,
        format: &str,
        _object: &llrm_object::Object,
    ) -> Result<Vec<u8>, llrm_object::Unsupported> {
        Err(llrm_object::Unsupported(format!("the 6502 writes assembly, not a {format} object")))
    }

    fn asm_info(
        &self,
        _format: &str,
    ) -> Option<llrm_object::AsmInfo> {
        None
    }
}

impl llrm_debug::DebugObjectTarget for MosObjects {
    fn write_described(
        &self,
        format: &str,
        _object: &llrm_object::Object,
        _debug: Option<&llrm_debug::Info>,
    ) -> Result<Vec<u8>, llrm_object::Unsupported> {
        Err(llrm_object::Unsupported(format!("the 6502 writes assembly, not a {format} object")))
    }

    fn debug_caps(
        &self,
        _format: &str,
        _asked: llrm_debug::Format,
    ) -> llrm_debug::DebugCaps {
        llrm_debug::DebugCaps::default()
    }
}

impl Target for Mos {
    fn name(&self) -> &'static str {
        "mos"
    }

    fn objects(&self) -> &'static dyn llrm_debug::DebugObjectTarget {
        &MosObjects
    }

    fn debug_target(&self) -> llrm_debug::DebugTarget {
        llrm_debug::DebugTarget { segmented: false, ..llrm_debug::DebugTarget::of(16) }
    }

    fn machine(&self) -> Machine {
        Machine::parse(include_str!("machines/machine.toml"), self.default_cpu())
            .expect("mos platform.toml parses")
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
        _name: &str,
    ) -> Option<timings::CpuTable> {
        None
    }

    fn registers_text(&self) -> String {
        include_str!("registers.regs").to_owned()
    }

    fn forms_text(&self) -> String {
        include_str!("instructions/mos.instr").to_owned()
    }

    fn dialect(&self) -> &'static Dialect {
        &DIALECT
    }

    fn operand_bytes(&self) -> i64 {
        1
    }

    fn huge_fill_routine(
        &self,
        _cell: u32,
    ) -> Result<routines::HugeString, String> {
        Err("mos has no huge pointers".to_owned())
    }

    fn huge_copy_routine(
        &self,
        _down: bool,
    ) -> Result<routines::HugeString, String> {
        Err("mos has no huge pointers".to_owned())
    }

    fn divide_routine(
        &self,
        _signed: bool,
        _wide_divisor: bool,
    ) -> Result<routines::Divide, String> {
        Err("mos divides inline or by a library call".to_owned())
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
        1
    }

    fn shift_op_bytes(
        &self,
        _count: i64,
        _width: i64,
    ) -> i64 {
        1
    }

    fn multiply_immediate_bytes(
        &self,
        _number: i64,
        _width: i64,
    ) -> i64 {
        1
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
        ObjectFormat::parse(include_str!("machines/object.toml")).expect("mos object.toml parses")
    }
}
