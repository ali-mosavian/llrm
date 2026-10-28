//! The x86-64 target: flat, one 64-bit pointer, System V AMD64, ELF.
//! Its descriptions are the files beside this one; this answers what they
//! cannot say.

include!(concat!(env!("OUT_DIR"), "/effects.rs"));
include!(concat!(env!("OUT_DIR"), "/register_info.rs"));

use std::sync::LazyLock;

use llrm_lir::registers::RegId;
use llrm_mir::target::{AddressForm, OperationCosts};
use llrm_target::CostModel;
use llrm_target::machine::Machine;

/// A Linux user program: flat, no ports.
pub const LINUX64: &str = include_str!("../../../../machine/platform/linux/flat64.toml");

/// The processors flat code is priced for: the columns of `timings.times`.
pub static CPUS: LazyLock<Vec<&'static str>> = LazyLock::new(|| TIMINGS.cpus());

/// The data layout and address spaces: `machines/datalayout.toml`.
pub const DATALAYOUT_TOML: &str = include_str!("machines/datalayout.toml");

/// `DATALAYOUT_TOML`, parsed once.
static LAYOUT: LazyLock<llrm_target::layout::Layout> =
    LazyLock::new(|| llrm_target::layout::Layout::parse(DATALAYOUT_TOML).expect("x86-64 datalayout.toml parses"));

/// The calling conventions: `calling.toml`.
static CALLING: LazyLock<llrm_target::calling::Calling> = LazyLock::new(|| {
    llrm_target::calling::Calling::parse(include_str!("machines/calling.toml")).expect("calling.toml parses")
});

/// Their names, the first being the language's own.
static CONVENTIONS: LazyLock<Vec<&'static str>> = LazyLock::new(|| CALLING.names());

/// The x86-64 target's layout.
pub fn layout() -> llrm_target::layout::Layout {
    LAYOUT.clone()
}

/// The x86-64 target as `llrm-driver` names it.
pub struct M64;

/// The registers and the address forms the descriptions state.
static REGISTERS: LazyLock<Vec<llrm_target::registers::Register>> =
    LazyLock::new(|| llrm_target::registers::parse(include_str!("registers.regs")).expect("registers.regs parses"));

/// How flat operations are priced from the instruction forms: `opcosts.txt`.
static OPCOSTS: LazyLock<llrm_target::opcosts::Description> = LazyLock::new(|| {
    llrm_target::opcosts::Description::parse(include_str!("opcosts.txt")).expect("opcosts.txt parses")
});

/// The CPUs' timings: `timings.times`.
static TIMINGS: LazyLock<llrm_target::timings::Timings> = LazyLock::new(|| {
    llrm_target::timings::Timings::parse_tuned(include_str!("timings.times"), llrm_target::tune::X86)
        .expect("timings.times parses")
});

static ADDRESS_FORMS: LazyLock<Vec<AddressForm>> = LazyLock::new(|| {
    llrm_target::addressing::forms(include_str!("machines/datalayout.toml")).expect("datalayout.toml parses")
});

impl llrm_target::Target for M64 {
    fn dialect(&self) -> &'static llrm_target::instr::Dialect {
        llrm_x86::instructions::dialect()
    }

    /// SSE holds a float at its own size.
    fn float_width(
        &self,
        bytes: u32,
    ) -> u32 {
        bytes
    }

    fn register_op_bytes(
        &self,
        width: i64,
    ) -> i64 {
        llrm_x86::encoding::register_bytes(width, 4)
    }

    fn shift_op_bytes(
        &self,
        count: i64,
        width: i64,
    ) -> i64 {
        llrm_x86::encoding::shift_bytes(count, width, 4)
    }

    fn multiply_immediate_bytes(
        &self,
        number: i64,
        width: i64,
    ) -> i64 {
        llrm_x86::encoding::imul_immediate_bytes(number, width, 4)
    }

    fn register_named(
        &self,
        name: &str,
    ) -> llrm_lir::registers::RegId {
        llrm_x86::calling::register(name)
    }

    fn convention_results(
        &self,
        convention: &llrm_target::calling::Convention,
        width: u32,
    ) -> Vec<llrm_lir::registers::RegId> {
        llrm_x86::calling::results(convention, width)
    }

    fn convention_callee_saved(
        &self,
        convention: &llrm_target::calling::Convention,
    ) -> Vec<(llrm_lir::registers::RegId, llrm_lir::registers::RegId)> {
        llrm_x86::calling::callee_saved(convention)
    }

    fn registers(&self) -> &'static llrm_lir::registers::Info {
        &REGISTER_INFO
    }

    fn selects_by_dag(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "x86-m64"
    }

    fn machine(&self) -> Machine {
        Machine::parse(LINUX64, TIMINGS.default_cpu().expect("timings.times states a default CPU"))
            .expect("the Linux description parses")
            .with_layout(layout())
    }

    fn flat_foreign(&self) -> llrm_target::Foreign {
        static FOREIGN: LazyLock<Vec<(i64, i64)>> = LazyLock::new(|| M64.machine().flat_foreign());
        llrm_target::Foreign(|| &FOREIGN)
    }

    fn flat_ports(&self) -> llrm_target::PortTable {
        static PORTS: LazyLock<Vec<llrm_target::machine::Port>> = LazyLock::new(|| M64.machine().ports);
        llrm_target::PortTable(|| &PORTS)
    }

    fn cpus(&self) -> &'static [&'static str] {
        &CPUS
    }

    fn conventions(&self) -> &'static [&'static str] {
        &CONVENTIONS
    }

    fn runtime(
        &self,
        language: &str,
    ) -> Option<llrm_target::runtime::Description> {
        match language {
            "c" => Some(llrm_target::runtime::Description {
                directory: concat!(env!("CARGO_MANIFEST_DIR"), "/../../../runtime/hosted/x86-m64"),
                text: include_str!("../../../../runtime/hosted/x86-m64/c.toml"),
                files: &[],
            }),
            _ => None,
        }
    }

    fn calling(&self) -> &'static llrm_target::calling::Calling {
        &CALLING
    }

    fn layout(&self) -> llrm_target::layout::Layout {
        LAYOUT.clone()
    }

    /// An argument takes a dword at least: `calling.toml`'s.
    fn stack_slot_bytes(&self) -> i64 {
        CALLING.native().slot_bytes
    }

    fn aligned_calls(&self) -> bool {
        CALLING.native().aligned_calls
    }

    fn stack_alignment(&self) -> i64 {
        CALLING.native().stack_alignment.unwrap_or(CALLING.native().slot_bytes)
    }

    fn frame_register(&self) -> RegId {
        llrm_x86::calling::frame(CALLING.native())
    }

    fn callee_pops(&self) -> bool {
        CALLING.conventions.iter().any(|one| one.cleanup == llrm_target::calling::Cleanup::Callee)
    }

    fn frame_optional(&self) -> bool {
        CALLING.native().frame_optional
    }

    fn frame_enter(&self) -> bool {
        CALLING.native().frame_enter
    }

    fn frame_slot_scratch(&self) -> Option<(RegId, RegId)> {
        let (push, pop) = CALLING.native().frame_slot_scratch.as_ref()?;
        Some((llrm_x86::calling::register(push), llrm_x86::calling::register(pop)))
    }

    fn frame_wrap_split_size(&self) -> bool {
        CALLING.native().frame_wrap_split_size
    }

    /// Past EBP and the return address; there is no far call.
    fn first_argument_offset(
        &self,
        far: bool,
    ) -> i64 {
        llrm_x86::calling::first_argument_offset(CALLING.native(), far)
    }

    /// Flat: no segments, one model.
    fn object(&self) -> llrm_target::object::ObjectFormat {
        static OBJECT: std::sync::LazyLock<llrm_target::object::ObjectFormat> = std::sync::LazyLock::new(|| {
            llrm_target::object::ObjectFormat::parse(include_str!("machines/object.toml"))
                .expect("x86-64 object.toml parses")
        });
        OBJECT.clone()
    }

    fn return_address_bytes(
        &self,
        far: bool,
    ) -> i64 {
        llrm_x86::calling::return_address_bytes(CALLING.native(), far)
    }

    fn stack_pointer(&self) -> RegId {
        llrm_x86::calling::stack(CALLING.native())
    }

    /// What cdecl32 keeps, but the frame register.
    fn callee_saved(&self) -> Vec<(RegId, RegId)> {
        llrm_x86::calling::callee_saved(CALLING.native())
    }

    fn march(
        &self,
        name: &str,
    ) -> Option<&'static str> {
        TIMINGS.march(name)
    }

    fn marches(&self) -> Vec<&'static str> {
        TIMINGS.marches()
    }

    fn registers_text(&self) -> String {
        include_str!("registers.regs").to_owned()
    }

    fn forms_text(&self) -> String {
        llrm_x86::instructions::joined(include_str!("instructions/x86.instr"))
    }

    fn operand_bytes(&self) -> i64 {
        8
    }

    fn huge_fill_routine(
        &self,
        _cell: u32,
    ) -> Result<llrm_target::routines::HugeString, String> {
        Err("long mode has no huge pointers".to_owned())
    }

    fn huge_copy_routine(
        &self,
        _down: bool,
    ) -> Result<llrm_target::routines::HugeString, String> {
        Err("long mode has no huge pointers".to_owned())
    }

    fn divide_routine(
        &self,
        _signed: bool,
        _wide_divisor: bool,
    ) -> Result<llrm_target::routines::Divide, String> {
        Err("long mode divides 64 bits natively".to_owned())
    }

    fn default_cpu(&self) -> &'static str {
        TIMINGS.default_cpu().expect("timings.times states a default CPU")
    }

    /// X86ISelLowering.cpp: `setPrefFunctionAlignment(Align(16))`.
    fn pref_function_alignment(&self) -> u8 {
        4
    }

    /// Every x86-64 has `nopl` (`FeatureNOPL`): X86AsmBackend uses up to ten
    /// bytes.
    fn max_nop_bytes(&self) -> u8 {
        10
    }

    fn cpu_table(
        &self,
        name: &str,
    ) -> Option<llrm_target::timings::CpuTable> {
        TIMINGS.cpu(name).cloned()
    }

    fn operation_costs(
        &self,
        price: &dyn Fn(&str) -> i64,
        prefix: i64,
    ) -> OperationCosts {
        OPCOSTS.operations(price, prefix)
    }

    fn register_capacity(&self) -> i64 {
        llrm_target::registers::allocatable(&REGISTERS) as i64
    }

    fn address_forms(
        &self,
        _: &OperationCosts,
        _: i64,
    ) -> Vec<AddressForm> {
        ADDRESS_FORMS.clone()
    }

    fn cost_model(&self) -> CostModel {
        |prices| {
            llrm_target::described_with(
                prices,
                Some(OPCOSTS.size_costs()),
                llrm_x86::tti::X86::of_cpu("x86-64")
                    .map(|one| std::rc::Rc::new(one) as std::rc::Rc<dyn llrm_mir::tti::Tti>),
                Some(std::rc::Rc::new(llrm_x86::lowering::X86 { is_64bit: true })),
            )
        }
    }

    fn results(
        &self,
        width: u32,
    ) -> Vec<RegId> {
        llrm_x86::calling::results(CALLING.native(), width)
    }
}

#[cfg(test)]
mod tests {
    /// x86-64 described no x87 registers, so an x87 value (a QB `fatan`, a
    /// `long double`) had no register class and the DAG path refused it.
    #[test]
    fn test_the_x87_stack_and_its_seven_rfp_registers_are_described() {
        use llrm_target::Target;
        let file = llrm_target::registers::parse(&M64.registers_text()).unwrap();
        let named = |class: &str| -> Vec<&str> {
            file.iter().filter(|one| one.is(class)).map(|one| one.name.as_str()).collect()
        };
        assert_eq!(named("x87"), ["st0", "st1", "st2", "st3", "st4", "st5", "st6", "st7"]);
        assert_eq!(named("rfp"), ["mm0", "mm1", "mm2", "mm3", "mm4", "mm5", "mm6"]);
        assert_eq!(M64.float_stack(), 8);
    }

    /// `object()` and the runtime tables are parsed once and kept; each must
    /// equal a fresh parse of its toml (35 parses of object.toml and 4 of the
    /// runtime table cost 3.3 M Ir of every compile).
    #[test]
    fn test_cached_tomls_equal_a_fresh_parse() {
        use llrm_target::Target;
        let fresh = llrm_target::object::ObjectFormat::parse(include_str!("machines/object.toml")).unwrap();
        assert_eq!(M64.object(), fresh);
        assert_eq!(M64.object(), fresh, "a second ask");
        for language in ["c", "nib"] {
            let Some(description) = M64.runtime(language) else { continue };
            let fresh: toml::Table = description.text.parse().unwrap();
            assert_eq!(*description.table().unwrap(), fresh, "{language}");
            assert!(std::sync::Arc::ptr_eq(&description.table().unwrap(), &description.table().unwrap()));
        }
    }

    use llrm_target::Target;

    use super::*;

    /// The roles the description names: RBP frames, RSP is the stack, and SysV
    /// keeps rbx, r12-r15 (rbp is the frame).
    #[test]
    fn the_register_file_names_sysv_amd64s_roles() {
        assert_eq!((M64.frame_register(), M64.stack_pointer()), (RegId::RBP, RegId::RSP));
        // Each is the root its family is named by and the quadword a prologue
        // pushes.
        let (roots, pushed): (Vec<RegId>, Vec<RegId>) = M64.callee_saved().into_iter().unzip();
        assert_eq!(roots, [RegId::EBX, RegId::R12D, RegId::R13D, RegId::R14D, RegId::R15D]);
        assert_eq!(pushed, [RegId::RBX, RegId::R12, RegId::R13, RegId::R14, RegId::R15]);
        assert_eq!((M64.stack_slot_bytes(), M64.stack_alignment(), M64.return_address_bytes(false)), (8, 16, 8));
        assert_eq!(M64.first_argument_offset(false), 16);
    }

    /// The pointer is a quadword and the layout string is the pin's clang's for
    /// x86_64-linux-gnu.
    #[test]
    fn the_layout_is_the_pins_clangs() {
        let layout = M64.layout();
        assert_eq!(layout.mode, 64);
        assert_eq!(
            layout.datalayout,
            "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
        );
    }

    /// A `double` argument went to the stack (floats were not placed in
    /// registers): it must take XMM0-7 beside the integer registers, and a
    /// ninth leave on the stack.
    #[test]
    fn sysv_places_floats_in_xmm_registers_and_returns_one_in_xmm0() {
        use llrm_target::calling::{Kind, Place};
        let convention = M64.calling().named("sysv64").unwrap();
        let mut arguments = vec![Kind::Word, Kind::Float(8), Kind::Float(8), Kind::Word];
        arguments.extend([Kind::Float(8); 7]);
        let placement = convention.place(&arguments);
        let register = |name: &str| Place::Registers(vec![name.to_owned()]);
        assert_eq!(placement.places[..4], [register("rdi"), register("xmm0"), register("xmm1"), register("rsi")]);
        assert_eq!(placement.places[10], Place::Stack(0));
        assert_eq!(convention.float_result_register(), Some("xmm0"));
        assert!(convention.clobbered.iter().any(|one| one == "xmm15"));
    }
}
