//! The register file as the target description states it (`registers.regs`),
//! asked through queries, as LLVM's generated `MCRegisterInfo` is: the width,
//! root, lane, name and classes of a register, and the view of a root at a
//! width. A `RegId` is iced's `Register` until the newtype (row 11).

pub type RegId = iced_x86::Register;

/// One register of the file.
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub name: &'static str,
    pub bits: u32,
    pub root: RegId,
    /// The bit offset inside the root.
    pub lane: u32,
    /// The classes every target gives it, as `class` bits.
    pub classes: u32,
}

include!(concat!(env!("OUT_DIR"), "/register_info.rs"));

/// The file's entry for `register`, if the description lists it.
pub fn get(register: RegId) -> Option<&'static Entry> {
    TABLE.get(register as usize).and_then(Option::as_ref)
}

/// Whether the description lists `register`.
pub fn known(register: RegId) -> bool {
    get(register).is_some()
}

/// The width of `register`, in bytes.
pub fn bytes(register: RegId) -> Option<i64> {
    get(register).map(|one| i64::from(one.bits / 8))
}

/// The register `register` is a view of (itself for a root, and for one the
/// description does not list).
pub fn root(register: RegId) -> RegId {
    get(register).map_or(register, |one| one.root)
}

/// Which of a root's four bytes `register` names, one bit each: the lane mask.
pub fn lanes(register: RegId) -> i64 {
    get(register).map_or(0b1111, |one| {
        let width = (one.bits / 8).min(4);
        ((1_i64 << width) - 1) << (one.lane / 8)
    })
}

/// `register`'s own name, lowercase.
pub fn name(register: RegId) -> Option<&'static str> {
    get(register).map(|one| one.name)
}

/// Whether the description gives `register` every class in `mask` (`class`
/// bits) in every target that lists it.
pub fn in_class(
    register: RegId,
    mask: u32,
) -> bool {
    get(register).is_some_and(|one| one.classes & mask == mask)
}

/// The register of `root` that is `bits` wide: the first by iced's number where
/// several share it (AL and AH are both EAX's byte; AL is the one named).
pub fn view(
    root: RegId,
    bits: u32,
) -> Option<RegId> {
    VIEWS
        .iter()
        .filter(|(of, width, _)| *of == root && *width == bits)
        .map(|(_, _, one)| *one)
        .min_by_key(|one| *one as usize)
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
}
