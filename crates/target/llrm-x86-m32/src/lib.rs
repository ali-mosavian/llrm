//! The flat 32-bit x86 target: CS = DS = SS, base 0, 4 GB, one 32-bit pointer.
//! Its descriptions are the files beside this one; this answers what they
//! cannot say.

include!(concat!(env!("OUT_DIR"), "/effects.rs"));
include!(concat!(env!("OUT_DIR"), "/register_info.rs"));

use std::sync::LazyLock;

use llrm_lir::registers::RegId;
use llrm_mir::target::{AddressForm, OperationCosts};
use llrm_target::CostModel;
use llrm_target::machine::Machine;

/// Flat DOS under an extender, and the PC ports it shares.
pub const DOS32: &str = concat!(
    include_str!("../../../../machine/platform/dos/flat32.toml"),
    include_str!("../../../../machine/platform/dos/ports.toml")
);

/// The processors flat code is priced for: the columns of `timings.times`.
pub static CPUS: LazyLock<Vec<&'static str>> = LazyLock::new(|| TIMINGS.cpus());

/// The data layout and address spaces: `machines/datalayout.toml`.
pub const DATALAYOUT_TOML: &str = include_str!("machines/datalayout.toml");

/// `DATALAYOUT_TOML`, parsed once.
static LAYOUT: LazyLock<llrm_target::layout::Layout> =
    LazyLock::new(|| llrm_target::layout::Layout::parse(DATALAYOUT_TOML).expect("flat datalayout.toml parses"));

/// The calling conventions: `calling.toml`.
static CALLING: LazyLock<llrm_target::calling::Calling> = LazyLock::new(|| {
    llrm_target::calling::Calling::parse(include_str!("machines/calling.toml")).expect("calling.toml parses")
});

/// Their names, the first being the language's own.
static CONVENTIONS: LazyLock<Vec<&'static str>> = LazyLock::new(|| CALLING.names());

/// The flat target's layout.
pub fn layout() -> llrm_target::layout::Layout {
    LAYOUT.clone()
}

/// The flat 32-bit x86 target as `llrm-driver` names it.
pub struct M32;

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

/// The pin's register pressure sets for the registers described above:
/// `pressure.sets`.
static PRESSURE: LazyLock<llrm_target::pressure::PressureSets> =
    LazyLock::new(|| llrm_target::pressure::parse(include_str!("pressure.sets")).expect("pressure.sets parses"));

static ADDRESS_FORMS: LazyLock<Vec<AddressForm>> = LazyLock::new(|| {
    llrm_target::addressing::forms(include_str!("machines/datalayout.toml")).expect("datalayout.toml parses")
});

/// The target's tuning. DEVIATE: a multiply by a constant the pin leaves an
/// `imul` (13-42 clocks on a 486, Intel 240440-002 Table 10.1) is, at -O1..-O3,
/// the shift/`lea` chain of fewest clocks on the CPU's table with no byte term,
/// as gcc's `synth_mult` does (`arithmetic::chain_by_clocks`, SD446); -Os keeps
/// bytes. x_iir was 2.17x gcc's clocks. Owner llrm-target-x86-m32.
fn tuning() -> llrm_mir::target::Tuning {
    llrm_mir::target::Tuning {
        multiply_by_clocks: true,
        // DEVIATE (full unroll): the pin's -O2/-O3 thresholds unroll the 8-trip
        // loops of matmul, tile, fpbench and nbody to 2188, 489, 1724 and 827 B
        // where gcc -O2, which does not unroll, has 382, 161, 503 and 394 B.
        // The pass takes the size threshold (a full unroll must not grow the
        // code), no boost, as m16 does. At
        // -O2 bytes vs the best of gcc/clang: geomean 1.230 -> 1.059, sum
        // 28114 -> 20618 B; clocks 0.915 -> 0.973. Owner llrm-target-x86-m32.
        unroll_threshold: Some(0),
        unroll_max_percent_threshold_boost: Some(100),
        // -O3 takes the pin's: gcc's -O3 lets a complete peel grow the code
        // (a 3-trip search copied out), -O2 does not. Owner
        // llrm-target-x86-m32.
        unroll_override_max_level: Some(2),
        ..llrm_mir::target::Tuning::PIN
    }
}

impl M32 {
    /// The pressure sets and classes of the pin's x86.
    pub fn pressure_sets() -> &'static llrm_target::pressure::PressureSets {
        &PRESSURE
    }

    /// The limit of pressure set `set` here (`computePSetLimit`): a register
    /// may be allocated if its root is a `gpr`.
    pub fn pressure_limit(set: usize) -> u32 {
        PRESSURE.limit(set, |name| {
            REGISTERS
                .iter()
                .find(|one| one.name == name)
                .and_then(|one| REGISTERS.iter().find(|root| root.name == one.root))
                .is_some_and(|root| root.is("gpr"))
        })
    }
}

impl llrm_target::Target for M32 {
    fn objects(&self) -> &'static dyn llrm_debug::DebugObjectTarget {
        &llrm_x86::mc::X86Objects
    }

    fn debug_target(&self) -> llrm_debug::DebugTarget {
        llrm_x86::mc::fixup::debug_target(32)
    }

    fn registers(&self) -> &'static llrm_lir::registers::Info {
        &REGISTER_INFO
    }

    fn selects_by_dag(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "x86-m32"
    }

    fn pressure(&self) -> Option<(&'static llrm_target::pressure::PressureSets, Vec<u32>)> {
        Some((
            M32::pressure_sets(),
            (0..PRESSURE.sets.len())
                // A set whose largest class the table leaves out holds no value
                // here (a class is described when a register of the target is
                // in it): its raw limit stands.
                .map(|set| {
                    if PRESSURE.class(&PRESSURE.sets[set].largest).is_some() {
                        M32::pressure_limit(set)
                    } else {
                        PRESSURE.sets[set].limit
                    }
                })
                .collect(),
        ))
    }

    fn machine(&self) -> Machine {
        Machine::parse(DOS32, TIMINGS.default_cpu().expect("timings.times states a default CPU"))
            .expect("the flat DOS description parses")
            .with_layout(layout())
    }

    fn flat_foreign(&self) -> llrm_target::Foreign {
        static FOREIGN: LazyLock<Vec<(i64, i64)>> = LazyLock::new(|| M32.machine().flat_foreign());
        llrm_target::Foreign(|| &FOREIGN)
    }

    fn flat_ports(&self) -> llrm_target::PortTable {
        static PORTS: LazyLock<Vec<llrm_target::machine::Port>> = LazyLock::new(|| M32.machine().ports);
        llrm_target::PortTable(|| &PORTS)
    }

    fn cpus(&self) -> &'static [&'static str] {
        &CPUS
    }

    fn conventions(&self) -> &'static [&'static str] {
        &CONVENTIONS
    }

    fn calling(&self) -> &'static llrm_target::calling::Calling {
        &CALLING
    }

    fn physical_addresses(&self) -> Vec<(String, u64)> {
        llrm_x86::physical_addresses()
    }

    fn runtime(
        &self,
        language: &str,
    ) -> Option<llrm_target::runtime::Description> {
        match language {
            "nib" => Some(llrm_target::runtime::Description {
                directory: concat!(env!("CARGO_MANIFEST_DIR"), "/../../../runtime/nib/x86-m32"),
                text: include_str!("../../../../runtime/nib/x86-m32/nib.toml"),
                files: &[("stack.toml", include_str!("../../../../runtime/nib/x86-m32/stack.toml"))],
            }),
            "c" => Some(llrm_target::runtime::Description {
                directory: concat!(env!("CARGO_MANIFEST_DIR"), "/../../../runtime/c/x86-m32"),
                text: include_str!("../../../../runtime/c/x86-m32/c.toml"),
                files: &[("stack.toml", include_str!("../../../../runtime/c/x86-m32/stack.toml"))],
            }),
            _ => None,
        }
    }

    fn os_layer(&self) -> Option<llrm_target::os::Layer> {
        Some(llrm_target::os::Layer {
            directory: concat!(env!("CARGO_MANIFEST_DIR"), "/../../../runtime/shared/dos/m32"),
            text: include_str!("../../../../runtime/shared/dos/m32/os.toml"),
            facts: llrm_x86::DOS_FACTS,
        })
    }

    fn layout(&self) -> llrm_target::layout::Layout {
        LAYOUT.clone()
    }

    /// An argument takes a dword at least: `calling.toml`'s.
    fn stack_slot_bytes(&self) -> i64 {
        CALLING.native().slot_bytes
    }

    fn stack_alignment(&self) -> i64 {
        CALLING.native().stack_alignment.unwrap_or(CALLING.native().slot_bytes)
    }

    fn frame_register(&self) -> RegId {
        llrm_x86::calling::frame(CALLING.native())
    }

    fn frame_optional(&self) -> bool {
        CALLING.native().frame_optional
    }

    fn frame_enter(&self) -> bool {
        CALLING.native().frame_enter
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
                .expect("flat object.toml parses")
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

    fn register_named(
        &self,
        name: &str,
    ) -> RegId {
        llrm_x86::calling::register(name)
    }

    fn convention_results(
        &self,
        convention: &llrm_target::calling::Convention,
        width: u32,
    ) -> Vec<RegId> {
        llrm_x86::calling::results(convention, width)
    }

    fn convention_callee_saved(
        &self,
        convention: &llrm_target::calling::Convention,
    ) -> Vec<(RegId, RegId)> {
        llrm_x86::calling::callee_saved(convention)
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

    fn dialect(&self) -> &'static llrm_target::instr::Dialect {
        llrm_x86::instructions::dialect()
    }

    fn operand_bytes(&self) -> i64 {
        4
    }

    fn huge_fill_routine(
        &self,
        cell: u32,
    ) -> Result<llrm_target::routines::HugeString, String> {
        llrm_x86::helpers::huge_fill(cell).map_err(|refused| refused.message)
    }

    fn huge_copy_routine(
        &self,
        down: bool,
    ) -> Result<llrm_target::routines::HugeString, String> {
        llrm_x86::helpers::huge_copy(down).map_err(|refused| refused.message)
    }

    fn divide_routine(
        &self,
        signed: bool,
        wide_divisor: bool,
    ) -> Result<llrm_target::routines::Divide, String> {
        llrm_x86::helpers::divide(signed, wide_divisor, 32).map_err(|refused| refused.message)
    }

    fn divide_routine_used(
        &self,
        signed: bool,
        wide_divisor: bool,
        used: llrm_target::routines::Used,
    ) -> Result<llrm_target::routines::Divide, String> {
        llrm_x86::helpers::divide_used(signed, wide_divisor, 32, used).map_err(|refused| refused.message)
    }

    fn float_width(
        &self,
        _: u32,
    ) -> u32 {
        llrm_x86::X87_WIDTH
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

    fn default_cpu(&self) -> &'static str {
        TIMINGS.default_cpu().expect("timings.times states a default CPU")
    }

    /// X86ISelLowering.cpp: `setPrefFunctionAlignment(Align(16))`.
    fn pref_function_alignment(&self) -> u8 {
        4
    }

    /// X86ISelLowering.cpp: `setPrefLoopAlignment(Align(16))`.
    fn pref_loop_alignment(&self) -> u8 {
        4
    }

    /// The i486 and Pentium have no `nopl` (`FeatureNOPL`): one byte.
    fn max_nop_bytes(&self) -> u8 {
        1
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
                // Every CPU the target accepts (i486, pentium) prices alike:
                // the x87 tables.
                layout()
                    .tti
                    .zip(layout().llvm_cpu)
                    .and_then(|(tti, cpu)| llrm_x86::tti::X86::described(&tti, &cpu))
                    .map(|one| std::rc::Rc::new(one.with_tuning(tuning())) as std::rc::Rc<dyn llrm_mir::tti::Tti>),
                Some(std::rc::Rc::new(llrm_x86::lowering::X86 { is_64bit: false })),
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
    /// The GPRs were ordered eax ecx edx ebx esi edi, so the callee-saved ebx
    /// was the first of them taken; the pin's GR32 order
    /// (X86RegisterInfo.td) is eax ecx edx esi edi ebx.
    #[test]
    fn test_general_registers_are_allocated_in_the_pins_order() {
        let order: Vec<&str> = super::REGISTERS
            .iter()
            .map(|r| r.name.as_str())
            .filter(|n| ["eax", "ecx", "edx", "esi", "edi", "ebx"].contains(n))
            .collect();
        assert_eq!(order, ["eax", "ecx", "edx", "esi", "edi", "ebx"]);
    }

    /// The limits the pin's `computePSetLimit` gives the x86-32 sets with the
    /// six general registers allocatable, worked by hand from the table:
    /// set 35 (GR16) is 66 - 2 * (34 - 6) of its largest class,
    /// LOW32_ADDR_ACCESS_RBP; GR32_BPSP (ebp, esp) has none free, so its
    /// raw 6; GR32_BSI has both; GR8_NOREX has its eight; GR8 is 38 - (36 -
    /// 8). Without the reserved-register step every set read its raw limit
    /// and the scheduler saw 66 words of room.
    #[test]
    fn pressure_set_limits_come_off_the_reserved_registers() {
        let limits: Vec<(usize, u32)> =
            [35, 1, 3, 13, 26].iter().map(|&set| (set, super::M32::pressure_limit(set))).collect();
        assert_eq!(limits, [(35, 10), (1, 6), (3, 6), (13, 10), (26, 10)]);
    }

    /// `Target::pressure()` panicked ("the largest class of a set is
    /// described") on the sets whose largest class the table leaves out, so
    /// every m32 compile with a tracked region failed. It gives one limit
    /// per set.
    #[test]
    fn pressure_gives_a_limit_to_every_set() {
        use llrm_target::Target;
        let (sets, limits) = super::M32.pressure().expect("m32 has pressure sets");
        assert_eq!(limits.len(), sets.sets.len());
        assert_eq!(limits[35], 10);
    }

    /// `object()` and the runtime tables are parsed once and kept; each must
    /// equal a fresh parse of its toml (35 parses of object.toml and 4 of the
    /// runtime table cost 3.3 M Ir of every compile).
    #[test]
    fn test_cached_tomls_equal_a_fresh_parse() {
        use llrm_target::Target;
        let fresh = llrm_target::object::ObjectFormat::parse(include_str!("machines/object.toml")).unwrap();
        assert_eq!(M32.object(), fresh);
        assert_eq!(M32.object(), fresh, "a second ask");
        for language in ["c", "nib"] {
            let Some(description) = M32.runtime(language) else { continue };
            let fresh: toml::Table = description.text.parse().unwrap();
            assert_eq!(*description.table().unwrap(), fresh, "{language}");
            assert!(std::sync::Arc::ptr_eq(&description.table().unwrap(), &description.table().unwrap()));
        }
    }

    /// gcc's by-pieces ratios of a constant memset, read from the description:
    /// `i486_cost` and `pentium_cost` (x86-tune-costs.h, gcc 13.4.0), and the
    /// size table every CPU takes.
    /// Greedy allocates the x87 values into the RFP registers FP0..FP6, LLVM's
    /// seven (FP7 is the stackifier's scratch); they are no stack position,
    /// so the float stack stays eight deep.
    #[test]
    fn test_seven_rfp_registers_and_an_eight_deep_float_stack() {
        use llrm_target::Target;
        let file = llrm_target::registers::parse(&M32.registers_text()).unwrap();
        let rfp: Vec<&str> = file.iter().filter(|one| one.is("rfp")).map(|one| one.name.as_str()).collect();
        assert_eq!(rfp, ["mm0", "mm1", "mm2", "mm3", "mm4", "mm5", "mm6"]);
        assert!(file.iter().filter(|one| one.is("rfp")).all(|one| !one.is("x87") && !one.is("gpr")));
        assert_eq!(M32.float_stack(), 8);
    }

    #[test]
    fn test_the_memset_ratios_are_gccs_per_cpu() {
        let (i486, p5) = (TIMINGS.cpu("486").expect("a 486"), TIMINGS.cpu("P5").expect("a P5"));
        assert_eq!(
            (i486.set_ratio, i486.clear_ratio, p5.set_ratio, p5.clear_ratio),
            (Some(3), Some(3), Some(6), Some(6))
        );
        assert_eq!((i486.set_ratio_size, i486.clear_ratio_size), (Some(3), Some(2)));
        assert_eq!((p5.set_ratio_size, p5.clear_ratio_size), (Some(3), Some(2)));
    }

    /// Loop interchange prices a walk by the cache line: 16 bytes on the 486's
    /// 8 KB cache, 32 on the Pentium's (the 64 it would take for a CPU that
    /// states none made a 486 walk of 32 bytes cost as two lines).
    #[test]
    fn test_the_cache_line_is_the_cpus() {
        let (i486, p5) = (TIMINGS.cpu("486").expect("a 486"), TIMINGS.cpu("P5").expect("a P5"));
        assert_eq!((i486.cache_line, p5.cache_line), (Some(16), Some(32)));
    }

    const EAX: RegId = RegId::EAX;
    const EBP: RegId = RegId::EBP;
    const EBX: RegId = RegId::EBX;
    const ECX: RegId = RegId::ECX;
    const EDI: RegId = RegId::EDI;
    const EDX: RegId = RegId::EDX;
    const ESI: RegId = RegId::ESI;
    const ESP: RegId = RegId::ESP;
    use llrm_target::Target;

    use super::*;

    /// The flat target's machine answered every port with "may touch any
    /// memory" (the trait's default) though its platform lists the PC's
    /// devices, so a load of a cell before `in al, dx` was reloaded after it:
    /// the table is the platform's, whatever the CPU mode.
    #[test]
    fn test_m32_answers_whether_a_port_reaches_memory_from_its_platforms_table() {
        let prices = llrm_target::CpuPrices {
            costs: Vec::new(),
            prefix: 1,
            address_stall: 0,
            registers: 6,
            call_registers: 3,
            float_registers: 0,
            address_forms: Vec::new(),
            operations: OperationCosts::default(),
            spaces: layout().spaces.roles,
            private: None,
            calling: None,
            foreign: llrm_target::Foreign::none(),
            ports: M32.flat_ports(),
        };
        let model = (M32.cost_model())(&prices);
        assert!(!model.port_touches_memory((0x3DA, 0x3DA)), "the VGA status register");
        assert!(model.port_touches_memory((0x0B, 0x0B)), "the 8237's mode register may start a transfer");
        assert!(model.port_touches_memory((0x300, 0x300)), "a port the platform does not list");
    }

    /// The pin's full-unroll thresholds made matmul 2188 B against gcc's 382 at
    /// m32 -O2 (tile 489 vs 161): m32 takes the size threshold, so a full
    /// unroll never grows code.
    #[test]
    fn test_m32_full_unroll_may_not_grow_the_code() {
        let prices = llrm_target::CpuPrices {
            costs: Vec::new(),
            prefix: 1,
            address_stall: 0,
            registers: 6,
            call_registers: 3,
            float_registers: 0,
            address_forms: Vec::new(),
            operations: OperationCosts::default(),
            spaces: layout().spaces.roles,
            private: None,
            calling: None,
            foreign: llrm_target::Foreign::none(),
            ports: M32.flat_ports(),
        };
        let tti = (M32.cost_model())(&prices).tti().expect("m32 states its cost model");
        assert_eq!(tti.unroll_threshold_overrides(2), (Some(0), Some(100)));
        assert_eq!(tti.unroll_threshold_overrides(3), (None, None));
    }

    /// The flat 32-bit target hands a pass that translates LLVM's the x87 X86
    /// cost model (`Machine::tti`): i486 and pentium price alike there.
    #[test]
    fn test_m32_states_llvms_cost_model_for_the_passes_that_translate_it() {
        use llrm_mir::tti::{ArithOp, Cost, CostKind, OperandInfo, Ty};
        let prices = llrm_target::CpuPrices {
            costs: Vec::new(),
            prefix: 1,
            address_stall: 0,
            registers: 6,
            call_registers: 3,
            float_registers: 0,
            address_forms: Vec::new(),
            operations: OperationCosts::default(),
            spaces: layout().spaces.roles,
            private: None,
            calling: None,
            foreign: llrm_target::Foreign::none(),
            ports: M32.flat_ports(),
        };
        let tti = (M32.cost_model())(&prices).tti().expect("m32 states its cost model");
        let divide = tti.arithmetic_cost(
            ArithOp::SDiv,
            Ty::Int(32),
            CostKind::RecipThroughput,
            OperandInfo::ANY,
            OperandInfo::ANY,
        );
        assert_eq!(divide, Cost::of(25));
    }

    /// -Os on m32 priced in clocks: the description had no byte table, so
    /// `size_costs` was the clock costs and a pass asked what a hoisted
    /// float costs in bytes was told 3, a load's clocks, not the 2 bytes of its
    /// release.
    #[test]
    fn test_m32_prices_code_size_in_bytes() {
        let prices = llrm_target::CpuPrices {
            costs: vec![("mov_rm".into(), 1)],
            prefix: 1,
            address_stall: 0,
            registers: 6,
            call_registers: 3,
            float_registers: 0,
            address_forms: Vec::new(),
            operations: OperationCosts { load: 1, ..Default::default() },
            spaces: layout().spaces.roles,
            private: None,
            calling: None,
            foreign: llrm_target::Foreign::none(),
            ports: llrm_target::PortTable::none(),
        };
        let model = (M32.cost_model())(&prices);
        let sizes = model.size_costs();
        assert_eq!((sizes.load, sizes.float_release, sizes.call, sizes.r#move), (6, 2, 5, 4));
        assert_eq!(model.costs().load, 1, "the clock prices are the CPU's");
    }

    /// A condition held as a value had no price: `phiopt` priced it as the
    /// `movzx` after it (3 clocks) and made x_switch 59% slower (45,159 clocks
    /// to 71,757) where `setcc` is 4 clocks on a 486 and 3 bytes.
    #[test]
    fn test_m32_prices_a_condition_held_as_a_value() {
        let table = M32.cpu_table("486").expect("the 486");
        let clocks = table.clocks.iter().find(|(form, _)| form == "setcc").map(|&(_, clocks)| clocks);
        assert_eq!(clocks, Some(4));
        let priced = OPCOSTS.operations(&|form| table.clocks.iter().find(|(one, _)| one == form).map_or(1, |p| p.1), 0);
        assert_eq!((priced.set, OPCOSTS.size_costs().set), (4, 3));
    }

    /// Flat code indexes by dwords natively: one form, no prefix, any scale.
    #[test]
    fn test_m32_has_one_native_dword_address_form() {
        let forms = M32.address_forms(&OperationCosts::default(), 0);
        assert_eq!(forms.len(), 1);
        assert!(
            !forms[0].secondary
                && forms[0].index_width == 4
                && forms[0].scales == std::collections::BTreeSet::from([1, 2, 4, 8])
        );
        let model = (M32.cost_model())(&llrm_target::CpuPrices {
            costs: Vec::new(),
            prefix: 1,
            address_stall: 0,
            registers: M32.register_capacity(),
            call_registers: M32.callee_saved().len() as i64,
            float_registers: 0,
            address_forms: forms.clone(),
            operations: OperationCosts::default(),
            spaces: layout().spaces.roles,
            private: None,
            calling: None,
            foreign: llrm_target::Foreign::none(),
            ports: llrm_target::PortTable::none(),
        });
        assert_eq!((model.registers(), model.call_registers()), (6, 5));
        assert_eq!(model.address_forms(), forms);
    }

    #[test]
    fn test_dos32_is_a_flat_machine_with_the_pc_ports() {
        let machine = M32.machine();
        assert_eq!(machine.addressing, llrm_target::machine::Addressing::Flat);
        assert!(machine.segments.is_none());
        assert!(!machine.ports.is_empty());
    }

    /// One address space of 32-bit pointers: `near` and `far` are space 0, an
    /// unmarked dword pointer is near, and the pair kinds are none.
    #[test]
    fn test_m32_layout_is_one_32_bit_space() {
        let layout = M32.layout();
        assert!(layout.datalayout.starts_with("e-p:32:32"));
        let spaces = layout.spaces;
        assert_eq!(
            (spaces.near, spaces.far, spaces.segment, spaces.huge, spaces.fixed, spaces.unmarked(4)),
            (0, 0, None, None, None, Ok(0))
        );
    }

    /// The register default comes first: the language's own functions use it,
    /// and `cdecl32` is named after it.
    #[test]
    fn test_calling_toml_states_watcall32_at_the_top_of_its_table() {
        let one = CALLING.native();
        assert_eq!(
            (one.name.as_str(), CONVENTIONS.as_slice()),
            ("watcall32", &["watcall32", "cdecl32", "fastcc32", "sysv32"][..])
        );
        assert_eq!(one.aggregate.as_ref().map(|one| one.style.as_str()), Some("hidden-pointer"));
        assert_eq!(
            (one.promotion.as_str(), one.wide_slots, one.results.keys().cloned().collect::<Vec<_>>()),
            ("slot", 2, vec!["1", "2", "4", "8", "float", "pointer"].into_iter().map(String::from).collect::<Vec<_>>())
        );
        assert_eq!(CALLING.by_cc("cdecl").map(|one| one.name.as_str()), Some("cdecl32"));
    }

    /// The aggregate return was marked provisional while the C ABI was open: it
    /// is Open Watcom's flat ABI with this one stated difference, and the
    /// description says so.
    #[test]
    fn test_the_c_abi_is_open_watcoms_with_one_stated_difference() {
        let text = include_str!("machines/calling.toml");
        assert!(!text.to_lowercase().contains("provisional"), "the aggregate rule is decided");
        assert!(
            text.contains("Open Watcom's 386 flat ABI")
                && text.contains("THE DIFFERENCE FROM OPEN WATCOM")
                && text.contains("aggregate = \"hidden-pointer\"")
        );
    }

    /// watcall32 (calling.toml): EBP and ESP frame, every register but EAX kept
    /// by a callee that is not given one in an argument, first stack
    /// argument at [ebp+8]; cdecl32 keeps ESI/EDI/EBX (the pin's `CSR_32`).
    #[test]
    fn test_m32_answers_watcall32() {
        let frame = M32.frame_registers();
        assert_eq!(
            (M32.object().bitness, M32.object().header),
            (32, vec![".386".to_owned(), ".model flat".to_owned()])
        );
        assert_eq!((frame.pointer, frame.stack), (EBP, ESP));
        assert_eq!(frame.saved, [(EBX, EBX), (ECX, ECX), (EDX, EDX), (ESI, ESI), (EDI, EDI)]);
        assert_eq!((M32.stack_slot_bytes(), M32.first_argument_offset(false)), (4, 8));
        // No far call: the return address is a dword either way.
        assert_eq!((M32.return_address_bytes(false), M32.return_address_bytes(true)), (4, 4));
        assert_eq!([4, 8].map(|width| M32.results(width)), [vec![EAX], vec![EAX, EDX]]);
        assert_eq!(
            llrm_x86::calling::callee_saved(CALLING.named("cdecl32").unwrap()),
            [(ESI, ESI), (EDI, EDI), (EBX, EBX)]
        );
    }

    /// Every convention that decorates a symbol for OMF says how for COFF: Open
    /// Watcom's own COFF objects (wcc386 -eoc) name `_c` for cdecl and `w_`
    /// for its register convention, as its OMF ones do. A convention with
    /// no `coff` entry leaves its COFF symbols undecorated.
    #[test]
    fn a_convention_that_decorates_for_omf_decorates_the_same_for_coff() {
        let names = CALLING.names();
        assert!(names.len() >= 2);
        for name in names {
            let symbol = &CALLING.named(name).unwrap().symbol;
            assert_eq!(symbol.get("coff"), symbol.get("omf"), "{name}");
        }
    }

    /// The debug numbers of the file: the i386 psABI's DWARF registers (eax 0
    /// .. edi 7, st0 at 11) and CodeView's (ebp 22, ax 9), checked against
    /// llvm's CodeViewRegisters.def. A view at lane 0 is read from its
    /// root's low bytes, so it has the root's DWARF number; one at lane 8 has
    /// none.
    #[test]
    fn the_register_file_numbers_each_debug_format() {
        let file = &*REGISTERS;
        let find = |name: &str| file.iter().find(|one| one.name == name).unwrap_or_else(|| panic!("no {name}"));
        let numbers = |name: &str| (find(name).dwarf, find(name).codeview);
        assert_eq!(numbers("eax"), (Some(0), Some(17)));
        assert_eq!(numbers("ebp"), (Some(5), Some(22)));
        assert_eq!(numbers("esp"), (Some(4), Some(21)));
        assert_eq!(numbers("ax"), (Some(0), Some(9)));
        assert_eq!(numbers("ah"), (None, Some(5)));
        assert_eq!(numbers("st3"), (Some(14), Some(131)));
        assert!(
            file.iter().filter(|one| one.lane != 0 && one.name != one.root).all(|one| one.dwarf.is_none()),
            "a view past lane 0 has no DWARF number"
        );
    }
}
