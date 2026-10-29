//! The 16-bit x86 target: what its instructions cost, as `Dos` prices them
//! for the passes, and the machine description a program is built for.

use llrm_lir::registers::RegId;
include!(concat!(env!("OUT_DIR"), "/effects.rs"));
include!(concat!(env!("OUT_DIR"), "/register_info.rs"));

pub mod cycles;
pub mod instructions;
pub mod machine;
pub mod target;
pub mod timings;

pub use cycles::report;
pub use target::{Dos, ENCODABLE_BASES, FRAME, GENERAL, PRESERVED, WORD_INDEXES, word_bases};
pub use timings::ARCHS;

/// Real mode's data layout and address spaces: `machines/datalayout.toml`.
pub const DATALAYOUT_TOML: &str = include_str!("machines/datalayout.toml");

/// The calling conventions: `calling.toml`.
static CALLING: std::sync::LazyLock<llrm_target::calling::Calling> = std::sync::LazyLock::new(|| {
    llrm_target::calling::Calling::parse(include_str!("machines/calling.toml")).expect("calling.toml parses")
});

/// Their names, the first being the language's own.
static CONVENTIONS: std::sync::LazyLock<Vec<&'static str>> = std::sync::LazyLock::new(|| CALLING.names());

/// `DATALAYOUT_TOML`, parsed once.
static LAYOUT: std::sync::LazyLock<llrm_target::layout::Layout> = std::sync::LazyLock::new(|| {
    llrm_target::layout::Layout::parse(DATALAYOUT_TOML).expect("real mode's datalayout.toml parses")
});

/// `DATALAYOUT_TOML`, parsed.
pub fn layout() -> llrm_target::layout::Layout {
    LAYOUT.clone()
}

/// Its address spaces by role.
pub fn spaces() -> llrm_mir::spaces::Spaces {
    LAYOUT.spaces.roles
}

/// The 16-bit x86 target as `llrm-driver` names it.
pub struct M16;

/// The registers the description states.
static REGISTERS: std::sync::LazyLock<Vec<llrm_target::registers::Register>> = std::sync::LazyLock::new(|| {
    llrm_target::registers::parse(include_str!("registers.regs")).expect("registers.regs parses")
});

/// The pin's register pressure sets for the registers described above:
/// `pressure.sets` (the pin's x86 tables are the same for 16 and 32 bits).
static PRESSURE: std::sync::LazyLock<llrm_target::pressure::PressureSets> = std::sync::LazyLock::new(|| {
    llrm_target::pressure::parse(include_str!("pressure.sets")).expect("pressure.sets parses")
});

impl M16 {
    /// The limit of pressure set `set` here (`computePSetLimit`): a register
    /// may be allocated if its root is a `gpr`.
    fn pressure_limit(set: usize) -> u32 {
        PRESSURE.limit(set, |name| {
            REGISTERS
                .iter()
                .find(|one| one.name == name)
                .and_then(|one| REGISTERS.iter().find(|root| root.name == one.root))
                .is_some_and(|root| root.is("gpr"))
        })
    }
}

fn cost_model(prices: &llrm_target::CpuPrices) -> std::rc::Rc<dyn llrm_mir::target::Machine> {
    let mut dos = Dos::priced(
        &prices.costs,
        prices.prefix,
        prices.address_stall,
        prices.registers,
        prices.call_registers,
        prices.float_registers,
    )
    .private(prices.private.clone())
    .calling(prices.calling);
    // What the CPU's table states beside its clocks.
    dos.costs.jump_table = prices.operations.jump_table;
    dos.costs.cache_line = prices.operations.cache_line;
    std::rc::Rc::new(dos)
}

impl llrm_target::Target for M16 {
    fn registers(&self) -> &'static llrm_lir::registers::Info {
        &REGISTER_INFO
    }

    fn register_capacity(&self) -> i64 {
        GENERAL.len() as i64
    }

    fn pressure(&self) -> Option<(&'static llrm_target::pressure::PressureSets, Vec<u32>)> {
        Some((
            &PRESSURE,
            (0..PRESSURE.sets.len())
                // A set whose largest class the table leaves out holds no value
                // here: its raw limit stands.
                .map(|set| {
                    if PRESSURE.class(&PRESSURE.sets[set].largest).is_some() {
                        M16::pressure_limit(set)
                    } else {
                        PRESSURE.sets[set].limit
                    }
                })
                .collect(),
        ))
    }

    fn march(
        &self,
        name: &str,
    ) -> Option<&'static str> {
        timings::TABLE.march(name)
    }

    fn marches(&self) -> Vec<&'static str> {
        timings::TABLE.marches()
    }

    fn registers_text(&self) -> String {
        include_str!("registers.regs").to_owned()
    }

    fn forms_text(&self) -> String {
        instructions::TEXT.clone()
    }

    fn dialect(&self) -> &'static llrm_target::instr::Dialect {
        llrm_x86::instructions::dialect()
    }

    fn operand_bytes(&self) -> i64 {
        2
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
        llrm_x86::helpers::divide(signed, wide_divisor, 16).map_err(|refused| refused.message)
    }

    fn divide_routine_used(
        &self,
        signed: bool,
        wide_divisor: bool,
        used: llrm_target::routines::Used,
    ) -> Result<llrm_target::routines::Divide, String> {
        llrm_x86::helpers::divide_used(signed, wide_divisor, 16, used).map_err(|refused| refused.message)
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
        llrm_x86::encoding::register_bytes(width, 2)
    }

    fn shift_op_bytes(
        &self,
        count: i64,
        width: i64,
    ) -> i64 {
        llrm_x86::encoding::shift_bytes(count, width, 2)
    }

    fn multiply_immediate_bytes(
        &self,
        number: i64,
        width: i64,
    ) -> i64 {
        llrm_x86::encoding::imul_immediate_bytes(number, width, 2)
    }

    fn default_cpu(&self) -> &'static str {
        timings::TABLE.default_cpu().expect("timings.times states a default CPU")
    }

    /// DEVIATE E2-1: the pin sets `setPrefFunctionAlignment(Align(16))`
    /// (X86ISelLowering.cpp); m16 aligns no function, loop or block, for
    /// code size (user decision 2026-10-18). Only segments are aligned, as
    /// their definitions require.
    fn pref_function_alignment(&self) -> u8 {
        0
    }

    /// DEVIATE E2-1: no loop is aligned (the pin's is 16 bytes).
    fn pref_loop_alignment(&self) -> u8 {
        0
    }

    /// 16-bit mode has the four forms of `Nops16Bit`.
    fn max_nop_bytes(&self) -> u8 {
        4
    }

    fn cpu_table(
        &self,
        name: &str,
    ) -> Option<llrm_target::timings::CpuTable> {
        timings::TABLE.cpu(name).cloned()
    }

    fn operation_costs(
        &self,
        price: &dyn Fn(&str) -> i64,
        prefix: i64,
    ) -> llrm_mir::target::OperationCosts {
        target::DESCRIPTION.operations(price, prefix)
    }

    fn address_forms(
        &self,
        costs: &llrm_mir::target::OperationCosts,
        address_stall: i64,
    ) -> Vec<llrm_mir::target::AddressForm> {
        target::address_forms(costs, address_stall)
    }

    fn cost_model(&self) -> llrm_target::CostModel {
        cost_model
    }

    fn selects_by_dag(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "x86-m16"
    }

    fn machine(&self) -> machine::Machine {
        machine::BUILT_IN.clone()
    }

    fn cpus(&self) -> &'static [&'static str] {
        &machine::CPUS
    }

    fn layout(&self) -> llrm_target::layout::Layout {
        layout()
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
                directory: concat!(env!("CARGO_MANIFEST_DIR"), "/../../../runtime/nib/x86-m16"),
                text: include_str!("../../../../runtime/nib/x86-m16/nib.toml"),
                files: &[("stack.toml", include_str!("../../../../runtime/nib/x86-m16/stack.toml"))],
            }),
            "c" => Some(llrm_target::runtime::Description {
                directory: concat!(env!("CARGO_MANIFEST_DIR"), "/../../../runtime/c/x86-m16"),
                text: include_str!("../../../../runtime/c/x86-m16/c.toml"),
                files: &[("stack.toml", include_str!("../../../../runtime/c/x86-m16/stack.toml"))],
            }),
            _ => None,
        }
    }

    fn os_layer(&self) -> Option<llrm_target::os::Layer> {
        Some(llrm_target::os::Layer {
            directory: concat!(env!("CARGO_MANIFEST_DIR"), "/../../../runtime/shared/dos/m16"),
            text: include_str!("../../../../runtime/shared/dos/m16/os.toml"),
            facts: llrm_x86::DOS_FACTS,
        })
    }

    /// A byte is pushed as a word: `calling.toml`'s.
    fn stack_slot_bytes(&self) -> i64 {
        CALLING.native().slot_bytes
    }

    fn frame_register(&self) -> RegId {
        llrm_x86::calling::frame(CALLING.native())
    }

    /// Past BP and a 2-byte return address, or a 4-byte far one.
    fn first_argument_offset(
        &self,
        far: bool,
    ) -> i64 {
        llrm_x86::calling::first_argument_offset(CALLING.native(), far)
    }

    fn object(&self) -> llrm_target::object::ObjectFormat {
        static OBJECT: std::sync::LazyLock<llrm_target::object::ObjectFormat> = std::sync::LazyLock::new(|| {
            llrm_target::object::ObjectFormat::parse(include_str!("machines/object.toml"))
                .expect("real mode's object.toml parses")
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

    fn string_registers(&self) -> Option<llrm_target::StringRegisters> {
        Some(llrm_target::StringRegisters {
            count: RegId::CX,
            source: RegId::SI,
            destination: RegId::DI,
            source_segment: RegId::DS,
            destination_segment: RegId::ES,
            tail: RegId::DX,
        })
    }

    /// A Borland caller keeps SI and DI, not their upper halves.
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

    /// Where a routine with no convention of its own answers (BASIC's, the
    /// runtime's): the stack convention's, cdecl16's. A function with a
    /// convention answers as its description says.
    fn results(
        &self,
        width: u32,
    ) -> Vec<RegId> {
        llrm_x86::calling::results(CALLING.by_cc("cdecl").expect("the stack convention"), width)
    }
}

#[cfg(test)]
mod tests {
    /// `object()` and the runtime tables are parsed once and kept; each must
    /// equal a fresh parse of its toml (35 parses of object.toml and 4 of the
    /// runtime table cost 3.3 M Ir of every compile).
    #[test]
    fn test_cached_tomls_equal_a_fresh_parse() {
        use llrm_target::Target;
        let fresh = llrm_target::object::ObjectFormat::parse(include_str!("machines/object.toml")).unwrap();
        assert_eq!(M16.object(), fresh);
        assert_eq!(M16.object(), fresh, "a second ask");
        for language in ["c", "nib"] {
            let Some(description) = M16.runtime(language) else { continue };
            let fresh: toml::Table = description.text.parse().unwrap();
            assert_eq!(*description.table().unwrap(), fresh, "{language}");
            assert!(std::sync::Arc::ptr_eq(&description.table().unwrap(), &description.table().unwrap()));
        }
    }

    /// E2 put 16-byte padding between every two functions of a 16-bit object,
    /// +2% of QCport's code; m16 asks for none (DEVIATE E2-1).
    /// Greedy allocates the x87 values into the RFP registers FP0..FP6, LLVM's
    /// seven (FP7 is the stackifier's scratch); they are no stack position,
    /// so the float stack stays eight deep.
    #[test]
    fn test_seven_rfp_registers_and_an_eight_deep_float_stack() {
        use llrm_target::Target;
        let file = llrm_target::registers::parse(&M16.registers_text()).unwrap();
        let rfp: Vec<&str> = file.iter().filter(|one| one.is("rfp")).map(|one| one.name.as_str()).collect();
        assert_eq!(rfp, ["mm0", "mm1", "mm2", "mm3", "mm4", "mm5", "mm6"]);
        assert!(file.iter().filter(|one| one.is("rfp")).all(|one| !one.is("x87") && !one.is("gpr")));
        assert_eq!(M16.float_stack(), 8);
    }

    #[test]
    fn test_m16_aligns_no_function_or_loop() {
        use llrm_target::Target;
        assert_eq!(super::M16.pref_function_alignment(), 0);
        assert_eq!(super::M16.pref_loop_alignment(), 0);
    }

    /// gcc's by-pieces ratios of a constant memset where its algorithm is `rep
    /// stos` (`i386_cost`, `i486_cost`, `pentium_cost`); none where it is a
    /// loop or a libcall (`pentiumpro_memset`).
    #[test]
    fn test_the_memset_ratios_are_gccs_per_cpu() {
        let ratio = |name: &str| {
            let table = timings::TABLE.cpu(name).expect("a CPU");
            (table.set_ratio, table.clear_ratio)
        };
        assert_eq!(
            (ratio("386"), ratio("486"), ratio("P5")),
            ((Some(3), Some(3)), (Some(3), Some(3)), (Some(6), Some(6)))
        );
        assert_eq!(ratio("P6"), (None, None));
    }

    const EAX: RegId = RegId::EAX;
    const EDX: RegId = RegId::EDX;
    use llrm_target::Target;

    use super::*;

    /// isel read these as literals: a 2-byte slot, BP, the first argument at
    /// [bp+4] (near) or [bp+6] (far), a dword result in DX:AX and an i64 in
    /// EDX:EAX. The conventions a program may name are `calling.toml`'s,
    /// the language's own first; BASIC's are Pascal's.
    #[test]
    fn test_calling_toml_gives_the_conventions_their_names() {
        assert_eq!(
            M16.conventions(),
            [
                "cdecl16",
                "pascal16",
                "qb45",
                "pds71",
                "vbdos",
                "interrupt16",
                "watcall16",
                "ia16",
                "regparm3",
                "regparm3pop"
            ]
        );
        let pascal = CALLING.named("pascal16").unwrap();
        for name in ["qb45", "pds71", "vbdos"] {
            assert_eq!(CALLING.named(name).unwrap().cleanup, llrm_target::calling::Cleanup::Callee);
            assert_eq!(
                (CALLING.named(name).unwrap().order, CALLING.named(name).unwrap().slot_bytes),
                (pascal.order, pascal.slot_bytes)
            );
        }
        assert_eq!(CALLING.named("interrupt16").unwrap().return_address_bytes, 6);
    }

    /// `like = "cdecl16"` carried Borland's `_name` into Pascal's: a Nib
    /// `pascal16` extern asked the linker for `_span`, the library defined
    /// `SPAN` (examples/pascal/levels.nib: four undefined references).
    #[test]
    fn test_pascal_symbols_are_capitals_whatever_they_were_derived_from() {
        for name in ["pascal16", "qb45", "pds71", "vbdos"] {
            assert_eq!(CALLING.named(name).unwrap().decorated("omf", "span").as_deref(), Some("SPAN"), "{name}");
        }
        assert_eq!(CALLING.named("cdecl16").unwrap().decorated("omf", "span").as_deref(), Some("_span"));
    }

    #[test]
    fn test_m16_answers_the_literals_isel_had() {
        assert_eq!((M16.stack_slot_bytes(), M16.frame_register()), (2, RegId::BP));
        assert_eq!((M16.first_argument_offset(false), M16.first_argument_offset(true)), (4, 6));
        assert_eq!(
            (M16.object().bitness, M16.object().header),
            (16, vec![".model medium".to_owned(), ".386".to_owned()])
        );
        assert_eq!((M16.return_address_bytes(false), M16.return_address_bytes(true)), (2, 4));
        assert_eq!(M16.stack_pointer(), RegId::SP);
        assert_eq!(M16.callee_saved(), PRESERVED.to_vec());
        assert_eq!(
            [1, 2, 4, 8].map(|width| M16.results(width)),
            [vec![EAX], vec![EAX], vec![EAX, EDX], vec![EAX, EDX]]
        );
    }

    /// Real mode has no psABI DWARF register map: its registers are numbered
    /// as Open Watcom's debugger and jwlink number them (dwregx86.h: AX is 27,
    /// DX 30, BP 33). A 16-bit register with no number made `-g -O2` refuse the
    /// compile ("register ax has no DWARF number").
    /// CodeView's ids are there for BP and AX.
    #[test]
    fn real_mode_numbers_codeview_and_dwarf_as_open_watcom_does() {
        let file = llrm_target::registers::parse(&M16.registers_text()).unwrap();
        let dwarf = |name: &str| file.iter().find(|one| one.name == name).unwrap().dwarf;
        assert_eq!(
            (dwarf("ax"), dwarf("dx"), dwarf("bp"), dwarf("sp"), dwarf("es")),
            (Some(27), Some(30), Some(33), Some(34), Some(38))
        );
        // FP0..FP6 (`rfp`) never reach the output: the stackifier makes each an
        // st(i), which has its number.
        let emitted = |one: &&llrm_target::registers::Register| !one.is("rfp");
        assert!(
            file.iter().filter(emitted).all(|one| one.dwarf.is_some()),
            "{:?}",
            file.iter().filter(emitted).filter(|one| one.dwarf.is_none()).map(|one| &one.name).collect::<Vec<_>>()
        );
        let cv = |name: &str| file.iter().find(|one| one.name == name).unwrap().codeview;
        assert_eq!((cv("bp"), cv("ax"), cv("st0")), (Some(14), Some(9), Some(128)));
    }

    /// gcc-ia16 keeps ES, which it uses as a register and saves; Borland's
    /// convention clobbers it: a call that kept a value in ES across a
    /// cdecl16 routine lost it.
    #[test]
    fn test_ia16_keeps_es_and_cdecl16_clobbers_it() {
        let kept = |name: &str| CALLING.named(name).unwrap().preserved.iter().any(|one| one.full == "es");
        assert!(kept("ia16") && !kept("cdecl16"));
        assert!(
            CALLING.named("cdecl16").unwrap().clobbered.iter().any(|one| one == "es")
                && !CALLING.named("ia16").unwrap().clobbered.iter().any(|one| one == "es")
        );
    }
}

#[cfg(test)]
mod pressure_tests {
    use llrm_target::Target;

    /// The scheduler counted every m16 value against one set of six registers
    /// (`pressure` was `None`, DEVIATE MS5): a byte value and a word value took
    /// the same room, and the pin's per-set limits (GR8 and GR16
    /// ten) never applied.
    #[test]
    fn test_m16_has_the_pins_pressure_sets_and_their_limits() {
        let (sets, limits) = super::M16.pressure().expect("m16 has pressure sets");
        assert_eq!(sets.sets.len(), limits.len());
        let limit = |name: &str| limits[sets.sets.iter().position(|set| set.name == name).expect(name)];
        assert_eq!((limit("GR8"), limit("GR16"), limit("GR32_BPSP")), (10, 10, 6));
    }
}

#[cfg(test)]
mod tuning_tests {
    use llrm_mir::target::{Machine, Tuning};

    /// m16's table overrides the size limits and the clock price of a byte and
    /// nothing else: before the table they were sentinels in the DAG builder
    /// (4 and 2) and a trait method (10_000), the pin's numbers literals beside
    /// them.
    #[test]
    fn m16_overrides_only_the_size_limits_and_the_rate() {
        let table = super::Dos::default().tuning();
        assert_eq!(table.max_stores_per_memset_opt_size, 4);
        assert_eq!((table.max_stores_per_memcpy_opt_size, table.max_stores_per_memmove_opt_size), (2, 2));
        assert_eq!(table.size_milliclocks_per_byte, 10_000);
        assert_eq!((table.unroll_threshold, table.unroll_max_percent_threshold_boost), (Some(0), Some(100)));
        assert_eq!(
            Tuning {
                max_stores_per_memset_opt_size: 8,
                max_stores_per_memcpy_opt_size: 4,
                max_stores_per_memmove_opt_size: 4,
                size_milliclocks_per_byte: 0,
                unroll_threshold: None,
                unroll_max_percent_threshold_boost: None,
                ..table
            },
            Tuning::PIN
        );
    }
}
