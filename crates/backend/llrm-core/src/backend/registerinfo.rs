//! The bound target's register file, as its description states it
//! (`registers.regs`) and the target generated it: queries on the `Info` the
//! driver bound with the target's selector. A `RegId` is iced's `Register`
//! until the newtype (row 11).

use std::sync::OnceLock;

pub use llrm_lir::registers::{Entry, FRAME, Info, RegId, STACK, class};

static BOUND: OnceLock<&'static Info> = OnceLock::new();

/// Binds the register file of the target the driver builds for; the first
/// binding stands. Targets that share a register file bind the same facts.
pub fn bind(info: &'static Info) {
    let _ = BOUND.set(info);
}

fn info() -> &'static Info {
    #[cfg(test)]
    return BOUND.get_or_init(|| &llrm_x86_m16::REGISTER_INFO);
    #[cfg(not(test))]
    BOUND.get().expect("no target's register file is bound: the driver binds it with the selector")
}

/// The entry for `register`, if the description lists it.
pub fn get(register: RegId) -> Option<&'static Entry> {
    info().get(register)
}

/// Whether the description lists `register`.
pub fn known(register: RegId) -> bool {
    info().known(register)
}

/// The width of `register`, in bytes.
pub fn bytes(register: RegId) -> Option<i64> {
    info().bytes(register)
}

/// The register `register` is a view of (itself for a root, and for one the
/// description does not list).
pub fn root(register: RegId) -> RegId {
    info().root(register)
}

/// Which of a root's four bytes `register` names, one bit each: the lane mask.
pub fn lanes(register: RegId) -> i64 {
    info().lanes(register)
}

/// `register`'s own name, lowercase.
pub fn name(register: RegId) -> Option<&'static str> {
    info().name(register)
}

/// Whether the description gives `register` every class in `mask`.
pub fn in_class(
    register: RegId,
    mask: u32,
) -> bool {
    info().in_class(register, mask)
}

/// The register of `root` that is `bits` wide: the first by iced's number where
/// several share it (AL and AH are both EAX's byte; AL is the one named).
pub fn view(
    root: RegId,
    bits: u32,
) -> Option<RegId> {
    info().view(root, bits)
}

/// Whether `register` is a view of the frame register's root.
pub fn is_frame(register: RegId) -> bool {
    info().is_frame(register)
}

/// Whether `register` is a view of the stack pointer's root.
pub fn is_stack(register: RegId) -> bool {
    info().is_stack(register)
}

/// The root of the frame register.
pub fn frame_root() -> RegId {
    info().frame
}

/// The root of the stack pointer.
pub fn stack_root() -> RegId {
    info().stack
}

/// The segment register an address space of a pair kind means, `None` where
/// the target has no segments.
pub fn data_segment() -> Option<RegId> {
    info().data_segment
}

pub fn stack_segment() -> Option<RegId> {
    info().stack_segment
}

pub fn code_segment() -> Option<RegId> {
    info().code_segment
}

/// The one a far pointer's selector is loaded into.
pub fn far_segment() -> Option<RegId> {
    info().far_segment
}

/// Whether `register` is a segment register.
pub fn load_form(segment: RegId) -> Option<&'static str> {
    info().load_form(segment)
}

pub fn loaded_by(form: &str) -> Option<RegId> {
    info().loaded_by(form)
}

/// Whether `form` loads a far pointer into a segment register an address may
/// be held in: any but the data segment, which is the default.
pub fn loads_a_selector(form: &str) -> bool {
    loaded_by(form).is_some_and(|segment| !is_data_segment(segment))
}

/// The segment an address through `base` reads without a prefix: the stack's
/// through the stack pointer or the frame register, the data segment's
/// through any other. None where the target has no segments.
pub fn default_segment(base: RegId) -> Option<RegId> {
    if is_stack(base) || is_frame(base) { stack_segment() } else { data_segment() }
}

/// Whether a word register can hold the offset of an address read through a
/// segment: a base or index of the description that is neither the frame
/// register nor the stack pointer, which select their own segment.
pub fn holds_a_segment_offset(
    register: RegId,
    offset_bytes: i64,
) -> bool {
    let root = root(register);
    bytes(register) == Some(offset_bytes)
        && (in_class(root, class::BASE) || in_class(root, class::INDEX))
        && !is_frame(register)
        && !is_stack(register)
}

/// The registers a pass may borrow for a moment, most preferred first: the
/// description's scratch order.
pub fn scratch_order() -> &'static [RegId] {
    info().scratch
}

pub fn is_segment(register: RegId) -> bool {
    in_class(register, class::SEGMENT)
}

pub fn is_data_segment(register: RegId) -> bool {
    data_segment() == Some(register)
}

pub fn is_stack_segment(register: RegId) -> bool {
    stack_segment() == Some(register)
}

pub fn is_code_segment(register: RegId) -> bool {
    code_segment() == Some(register)
}

/// The segment registers for code that exists only where the target has
/// address spaces of a pair kind: a target without them never reaches it.
pub mod segments {
    use super::RegId;

    fn named(
        role: Option<RegId>,
        what: &str,
    ) -> RegId {
        role.unwrap_or_else(|| panic!("the target's register file names no {what} segment"))
    }

    pub fn data() -> RegId {
        named(super::data_segment(), "data")
    }

    pub fn stack() -> RegId {
        named(super::stack_segment(), "stack")
    }

    pub fn far() -> RegId {
        named(super::far_segment(), "far")
    }
}

/// Every integer register by `bytes` wide, by iced's number.
pub fn entries() -> impl Iterator<Item = (RegId, &'static Entry)> {
    info().entries()
}

#[cfg(test)]
mod tests {
    use super::*;

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
