//! A target's register file, as its description states it
//! (`registers.regs`) and the target generated it: the `Info` a `Target` hands
//! out and every query of a register asks. A `RegId` is iced's `Register`
//! until the newtype (row 11).

pub use llrm_lir::registers::{Entry, FRAME, Info, RegId, Regs, STACK, class};

/// m16's register file, for the tests of this crate that compile for no target.
#[cfg(test)]
pub fn test_regs() -> Regs {
    Regs(&llrm_x86_m16::REGISTER_INFO)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// m16's file, which these tests ask directly.
    fn info() -> &'static Info {
        &llrm_x86_m16::REGISTER_INFO
    }
    fn known(register: RegId) -> bool {
        info().known(register)
    }
    fn bytes(register: RegId) -> Option<i64> {
        info().bytes(register)
    }
    fn root(register: RegId) -> RegId {
        info().root(register)
    }
    fn name(register: RegId) -> Option<&'static str> {
        info().name(register)
    }
    fn view(
        root: RegId,
        bits: u32,
    ) -> Option<RegId> {
        info().view(root, bits)
    }
    fn is_stack(register: RegId) -> bool {
        info().is_stack(register)
    }
    fn is_frame(register: RegId) -> bool {
        info().is_frame(register)
    }
    fn stack_root() -> RegId {
        info().stack
    }
    fn frame_root() -> RegId {
        info().frame
    }
    fn data_segment() -> Option<RegId> {
        info().data_segment
    }
    fn stack_segment() -> Option<RegId> {
        info().stack_segment
    }
    fn code_segment() -> Option<RegId> {
        info().code_segment
    }
    fn far_segment() -> Option<RegId> {
        info().far_segment
    }
    fn is_segment(register: RegId) -> bool {
        info().is_segment(register)
    }
    fn is_data_segment(register: RegId) -> bool {
        info().is_data_segment(register)
    }
    fn is_stack_segment(register: RegId) -> bool {
        info().is_stack_segment(register)
    }
    fn is_code_segment(register: RegId) -> bool {
        info().is_code_segment(register)
    }
    fn load_form(segment: RegId) -> Option<&'static str> {
        info().load_form(segment)
    }
    fn loaded_by(form: &str) -> Option<RegId> {
        info().loaded_by(form)
    }
    fn loads_a_selector(form: &str) -> bool {
        info().loads_a_selector(form)
    }
    fn default_segment(base: RegId) -> Option<RegId> {
        info().default_segment(base)
    }
    fn holds_a_segment_offset(
        register: RegId,
        offset_bytes: i64,
    ) -> bool {
        info().holds_a_segment_offset(register, offset_bytes)
    }
    fn scratch_order() -> &'static [RegId] {
        info().scratch
    }

    /// The tables the allocator has always read (`WIDTHS`, `AT_WIDTH`, `LANES`,
    /// `NAMES`, `ir::root`) and the description's queries say the same of every
    /// register the tables list: the description is the source and the tables
    /// are on their way out (row 11).
    #[test]
    fn the_description_says_what_the_hand_tables_say() {
        for (register, width) in llrm_x86::registers::WIDTHS.iter() {
            assert_eq!(bytes(*register), Some(*width), "{register:?}");
            assert_eq!(root(*register), crate::model::ir::root(*register), "{register:?}");
            assert_eq!(
                name(*register).map(str::to_owned),
                Some(format!("{register:?}").to_lowercase()),
                "{register:?}"
            );
        }
        for (root_register, views) in llrm_x86::registers::AT_WIDTH.iter() {
            for (width, register) in views {
                assert_eq!(view(*root_register, *width as u32 * 8), Some(*register), "{root_register:?} at {width}");
            }
        }
    }

    /// A register the description does not list is its own root: the extended
    /// ones.
    #[test]
    fn a_register_the_description_omits_is_its_own() {
        for register in [RegId::R8, RegId::XMM0] {
            assert!(!known(register));
            assert_eq!(root(register), register);
        }
        assert_eq!((bytes(RegId::ST3), root(RegId::ST3)), (Some(10), RegId::ST3));
    }

    /// The stack and the frame are the roots the description gives those
    /// classes, whatever view is asked about; no other register is either.
    #[test]
    fn the_stack_and_the_frame_are_the_roots_the_description_names() {
        assert_eq!((stack_root(), frame_root()), (RegId::ESP, RegId::EBP));
        assert!([RegId::SP, RegId::ESP].into_iter().all(is_stack));
        assert!([RegId::BP, RegId::EBP].into_iter().all(is_frame));
        for other in [RegId::AX, RegId::EAX, RegId::SI, RegId::DS, RegId::ST0] {
            assert!(!is_stack(other) && !is_frame(other), "{other:?}");
        }
        assert_eq!((FRAME, STACK), (RegId::BP, RegId::SP));
    }

    /// The segment each address space of a pair kind means is the register the
    /// description gives that class (m16's default); any other segment is none.
    #[test]
    fn the_segments_are_the_registers_the_description_names() {
        assert_eq!(
            (data_segment(), stack_segment(), code_segment(), far_segment()),
            (Some(RegId::DS), Some(RegId::SS), Some(RegId::CS), Some(RegId::ES))
        );
        assert!([RegId::ES, RegId::CS, RegId::SS, RegId::DS, RegId::FS, RegId::GS].into_iter().all(is_segment));
        assert!(![RegId::AX, RegId::EBP, RegId::ST0].into_iter().any(is_segment));
        assert!(!is_stack_segment(RegId::DS) && !is_data_segment(RegId::FS) && !is_code_segment(RegId::None));
    }

    /// The load that fills each segment register is the description's, and
    /// agrees with the form table's `d1=<segment>` row of that mnemonic.
    #[test]
    fn the_far_loads_are_the_ones_the_description_states() {
        let forms = include_str!("../../../../target/llrm-x86-m16/src/instructions/x86.instr");
        for (segment, form) in [(RegId::ES, "les"), (RegId::DS, "lds"), (RegId::FS, "lfs"), (RegId::GS, "lgs")] {
            assert_eq!((load_form(segment), loaded_by(form)), (Some(form), Some(segment)));
            let row = forms.lines().find(|line| line.starts_with(&format!("{form} "))).expect("a form row");
            assert!(row.contains(&format!("d1={}", info().name(segment).unwrap())), "{row}");
        }
        assert_eq!((load_form(RegId::CS), loaded_by("mov")), (None, None));
        assert!(loads_a_selector("les") && !loads_a_selector("lds") && !loads_a_selector("mov"));
    }

    /// An address through the stack or frame register reads the stack segment
    /// and any other the data segment: what the text of an address leaves
    /// unprefixed.
    #[test]
    fn an_address_reads_the_segment_its_base_implies() {
        for base in [RegId::BP, RegId::EBP, RegId::SP, RegId::ESP] {
            assert_eq!(default_segment(base), Some(RegId::SS), "{base:?}");
        }
        for base in [RegId::BX, RegId::SI, RegId::EDI, RegId::None] {
            assert_eq!(default_segment(base), Some(RegId::DS), "{base:?}");
        }
    }

    /// BX, SI and DI hold the offset of a data-group address; BP and SP select
    /// the stack segment themselves, and AX is no address register.
    #[test]
    fn only_the_unframed_bases_and_indexes_hold_a_segment_offset() {
        assert!([RegId::BX, RegId::SI, RegId::DI].into_iter().all(|one| holds_a_segment_offset(one, 2)));
        assert!(
            ![RegId::BP, RegId::SP, RegId::AX, RegId::EBX, RegId::ESI, RegId::None]
                .into_iter()
                .any(|one| holds_a_segment_offset(one, 2))
        );
        // The width is the segmented space's offset, not a word.
        assert!([RegId::EBX, RegId::ESI].into_iter().all(|one| holds_a_segment_offset(one, 4)));
        assert!(!holds_a_segment_offset(RegId::BX, 4));
    }

    /// The scratch order is the description's `@scratch` line: what a pass
    /// borrows first is CX, and the result register AX last.
    #[test]
    fn the_scratch_order_is_the_descriptions() {
        assert_eq!(scratch_order(), [RegId::CX, RegId::DX, RegId::BX, RegId::AX]);
    }
}
