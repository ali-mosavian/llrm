//! The flat 32-bit x86 target: CS = DS = SS, base 0, 4 GB, one 32-bit pointer.
//! Its descriptions are the files beside this one; this answers what they
//! cannot say.

include!(concat!(env!("OUT_DIR"), "/register_info.rs"));

use std::sync::LazyLock;

use iced_x86::Register;
use llrm_mir::target::{AddressForm, OperationCosts};
use llrm_target::CostModel;
use llrm_target::machine::Machine;

/// Flat DOS under an extender, and the PC ports it shares.
pub const DOS32: &str =
    concat!(include_str!("machines/dos32.toml"), include_str!("../../llrm-target/src/machines/pc-ports.toml"));

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
    llrm_target::timings::Timings::parse(include_str!("timings.times")).expect("timings.times parses")
});

static ADDRESS_FORMS: LazyLock<Vec<AddressForm>> = LazyLock::new(|| {
    llrm_target::addressing::forms(include_str!("machines/datalayout.toml")).expect("datalayout.toml parses")
});

impl llrm_target::Target for M32 {
    fn name(&self) -> &'static str {
        "x86-m32"
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

    fn frame_register(&self) -> Register {
        llrm_x86::calling::frame(CALLING.native())
    }

    fn frame_optional(&self) -> bool {
        CALLING.native().frame_optional
    }

    fn frame_enter(&self) -> bool {
        CALLING.native().frame_enter
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
        llrm_target::object::ObjectFormat::parse(include_str!("machines/object.toml")).expect("flat object.toml parses")
    }

    fn return_address_bytes(
        &self,
        far: bool,
    ) -> i64 {
        llrm_x86::calling::return_address_bytes(CALLING.native(), far)
    }

    fn stack_pointer(&self) -> Register {
        llrm_x86::calling::stack(CALLING.native())
    }

    /// What cdecl32 keeps, but the frame register.
    fn callee_saved(&self) -> Vec<(Register, Register)> {
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
        4
    }

    fn default_cpu(&self) -> &'static str {
        TIMINGS.default_cpu().expect("timings.times states a default CPU")
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
        |prices| llrm_target::described_by_size(prices, Some(OPCOSTS.size_costs()))
    }

    fn results(
        &self,
        width: u32,
    ) -> Vec<Register> {
        llrm_x86::calling::results(CALLING.native(), width)
    }
}

#[cfg(test)]
mod tests {
    use iced_x86::Register::{EAX, EBP, EBX, ECX, EDI, EDX, ESI, ESP};
    use llrm_target::Target;

    use super::*;

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
            address_forms: Vec::new(),
            operations: OperationCosts { load: 1, ..Default::default() },
            spaces: layout().spaces.roles,
            private: None,
            calling: None,
            foreign: llrm_target::Foreign::none(),
        };
        let model = (M32.cost_model())(&prices);
        let sizes = model.size_costs();
        assert_eq!((sizes.load, sizes.float_release, sizes.call, sizes.r#move), (6, 2, 5, 4));
        assert_eq!(model.costs().load, 1, "the clock prices are the CPU's");
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
            address_forms: forms.clone(),
            operations: OperationCosts::default(),
            spaces: layout().spaces.roles,
            private: None,
            calling: None,
            foreign: llrm_target::Foreign::none(),
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
        assert_eq!((one.name.as_str(), CONVENTIONS.as_slice()), ("watcall32", &["watcall32", "cdecl32", "sysv32"][..]));
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
    /// argument at [ebp+8]; cdecl32 keeps EBX/ESI/EDI.
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
            [(EBX, EBX), (ESI, ESI), (EDI, EDI)]
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
