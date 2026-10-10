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

/// Every integer register by `bytes` wide, by iced's number.
pub fn entries() -> impl Iterator<Item = (RegId, &'static Entry)> {
    info().entries()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::target;

    /// The tables the allocator has always read (`WIDTHS`, `AT_WIDTH`, `LANES`,
    /// `NAMES`, `ir::root`) and the description's queries say the same of every
    /// register the tables list: the description is the source and the tables
    /// are on their way out (row 11).
    #[test]
    fn the_description_says_what_the_hand_tables_say() {
        for (register, width) in target::WIDTHS.iter() {
            assert_eq!(bytes(*register), Some(*width), "{register:?}");
            assert_eq!(root(*register), crate::model::ir::root(*register), "{register:?}");
            assert_eq!(lanes(*register), target::lanes(*register), "{register:?}");
            assert_eq!(name(*register).map(str::to_owned), Some(target::name_of(*register)), "{register:?}");
        }
        for (root_register, views) in target::AT_WIDTH.iter() {
            for (width, register) in views {
                assert_eq!(view(*root_register, *width as u32 * 8), Some(*register), "{root_register:?} at {width}");
            }
        }
    }

    /// A register the description does not list is its own root: the segment
    /// registers and the extended ones.
    #[test]
    fn a_register_the_description_omits_is_its_own() {
        for register in [RegId::DS, RegId::R8, RegId::XMM0] {
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
}
