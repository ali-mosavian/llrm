//! The 16-bit x86 target: what its instructions cost, as `Dos` prices them
//! for the passes, and the machine description a program is built for.

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
static CALLING: std::sync::LazyLock<llrm_target::calling::Calling> = std::sync::LazyLock::new(|| llrm_target::calling::Calling::parse(include_str!("machines/calling.toml")).expect("calling.toml parses"));

/// Their names, the first being the language's own.
static CONVENTIONS: std::sync::LazyLock<Vec<&'static str>> = std::sync::LazyLock::new(|| CALLING.names());

/// `DATALAYOUT_TOML`, parsed once.
static LAYOUT: std::sync::LazyLock<llrm_target::layout::Layout> = std::sync::LazyLock::new(|| llrm_target::layout::Layout::parse(DATALAYOUT_TOML).expect("real mode's datalayout.toml parses"));

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

fn cost_model(prices: &llrm_target::CpuPrices) -> std::rc::Rc<dyn llrm_mir::target::Machine> {
    std::rc::Rc::new(Dos::priced(&prices.costs, prices.prefix, prices.address_stall, prices.registers, prices.call_registers))
}

impl llrm_target::Target for M16 {
    fn register_capacity(&self) -> i64 {
        GENERAL.len() as i64
    }

    fn march(&self, name: &str) -> Option<&'static str> {
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

    fn operand_bytes(&self) -> i64 {
        2
    }

    fn default_cpu(&self) -> &'static str {
        timings::TABLE.default_cpu().expect("timings.times states a default CPU")
    }

    fn cpu_table(&self, name: &str) -> Option<llrm_target::timings::CpuTable> {
        timings::TABLE.cpu(name).cloned()
    }

    fn operation_costs(&self, price: &dyn Fn(&str) -> i64, prefix: i64) -> llrm_mir::target::OperationCosts {
        target::DESCRIPTION.operations(price, prefix)
    }

    fn address_forms(&self, costs: &llrm_mir::target::OperationCosts, address_stall: i64) -> Vec<llrm_mir::target::AddressForm> {
        target::address_forms(costs, address_stall)
    }

    fn cost_model(&self) -> llrm_target::CostModel {
        cost_model
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

    fn runtime(&self, language: &str) -> Option<llrm_target::runtime::Description> {
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
        Some(llrm_target::os::Layer { directory: concat!(env!("CARGO_MANIFEST_DIR"), "/../../../runtime/shared/dos/m16"), text: include_str!("../../../../runtime/shared/dos/m16/os.toml"), facts: llrm_x86::DOS_FACTS })
    }

    /// A byte is pushed as a word: `calling.toml`'s.
    fn stack_slot_bytes(&self) -> i64 {
        CALLING.native().slot_bytes
    }

    fn frame_register(&self) -> iced_x86::Register {
        llrm_x86::calling::frame(CALLING.native())
    }

    /// Past BP and a 2-byte return address, or a 4-byte far one.
    fn first_argument_offset(&self, far: bool) -> i64 {
        llrm_x86::calling::first_argument_offset(CALLING.native(), far)
    }

    fn object(&self) -> llrm_target::object::ObjectFormat {
        llrm_target::object::ObjectFormat::parse(include_str!("machines/object.toml")).expect("real mode's object.toml parses")
    }

    fn return_address_bytes(&self, far: bool) -> i64 {
        llrm_x86::calling::return_address_bytes(CALLING.native(), far)
    }

    fn stack_pointer(&self) -> iced_x86::Register {
        llrm_x86::calling::stack(CALLING.native())
    }

    /// A Borland caller keeps SI and DI, not their upper halves.
    fn callee_saved(&self) -> Vec<(iced_x86::Register, iced_x86::Register)> {
        llrm_x86::calling::callee_saved(CALLING.native())
    }

    fn results(&self, width: u32) -> Vec<iced_x86::Register> {
        llrm_x86::calling::results(CALLING.native(), width)
    }
}

#[cfg(test)]
mod tests {
    use iced_x86::Register::{EAX, EDX};
    use llrm_target::Target;

    use super::*;

    /// isel read these as literals: a 2-byte slot, BP, the first argument at
    /// [bp+4] (near) or [bp+6] (far), a dword result in DX:AX and an i64 in EDX:EAX.
    /// The conventions a program may name are `calling.toml`'s, the language's own first; BASIC's are Pascal's.
    #[test]
    fn test_calling_toml_gives_the_conventions_their_names() {
        assert_eq!(M16.conventions(), ["cdecl16", "pascal16", "qb45", "pds71", "vbdos", "interrupt16", "watcall16", "ia16"]);
        let pascal = CALLING.named("pascal16").unwrap();
        for name in ["qb45", "pds71", "vbdos"] {
            assert_eq!(CALLING.named(name).unwrap().cleanup, llrm_target::calling::Cleanup::Callee);
            assert_eq!((CALLING.named(name).unwrap().order, CALLING.named(name).unwrap().slot_bytes), (pascal.order, pascal.slot_bytes));
        }
        assert_eq!(CALLING.named("interrupt16").unwrap().return_address_bytes, 6);
    }

    /// `like = "cdecl16"` carried Borland's `_name` into Pascal's: a Nib `pascal16` extern asked the linker for `_span`, the
    /// library defined `SPAN` (examples/pascal/levels.nib: four undefined references).
    #[test]
    fn test_pascal_symbols_are_capitals_whatever_they_were_derived_from() {
        for name in ["pascal16", "qb45", "pds71", "vbdos"] {
            assert_eq!(CALLING.named(name).unwrap().decorated("omf", "span").as_deref(), Some("SPAN"), "{name}");
        }
        assert_eq!(CALLING.named("cdecl16").unwrap().decorated("omf", "span").as_deref(), Some("_span"));
    }

    #[test]
    fn test_m16_answers_the_literals_isel_had() {
        assert_eq!((M16.stack_slot_bytes(), M16.frame_register()), (2, iced_x86::Register::BP));
        assert_eq!((M16.first_argument_offset(false), M16.first_argument_offset(true)), (4, 6));
        assert_eq!((M16.object().bitness, M16.object().header), (16, vec![".model medium".to_owned(), ".386".to_owned()]));
        assert_eq!((M16.return_address_bytes(false), M16.return_address_bytes(true)), (2, 4));
        assert_eq!(M16.stack_pointer(), iced_x86::Register::SP);
        assert_eq!(M16.callee_saved(), PRESERVED.to_vec());
        assert_eq!([1, 2, 4, 8].map(|width| M16.results(width)), [vec![EAX], vec![EAX], vec![EAX, EDX], vec![EAX, EDX]]);
    }

    /// Real mode has no psABI DWARF register map: none is numbered, so a DWARF location in m16
    /// code is refused instead of written with i386's. CodeView's ids are there for BP and AX.
    #[test]
    fn real_mode_numbers_codeview_but_not_dwarf() {
        let file = llrm_target::registers::parse(&M16.registers_text()).unwrap();
        assert!(file.iter().all(|one| one.dwarf.is_none()));
        let cv = |name: &str| file.iter().find(|one| one.name == name).unwrap().codeview;
        assert_eq!((cv("bp"), cv("ax"), cv("st0")), (Some(14), Some(9), Some(128)));
    }

    /// gcc-ia16 keeps ES, which it uses as a register and saves; Borland's convention clobbers it: a call that kept a value in ES
    /// across a cdecl16 routine lost it.
    #[test]
    fn test_ia16_keeps_es_and_cdecl16_clobbers_it() {
        let kept = |name: &str| CALLING.named(name).unwrap().preserved.iter().any(|one| one.full == "es");
        assert!(kept("ia16") && !kept("cdecl16"));
        assert!(CALLING.named("cdecl16").unwrap().clobbered.iter().any(|one| one == "es") && !CALLING.named("ia16").unwrap().clobbered.iter().any(|one| one == "es"));
    }
}
