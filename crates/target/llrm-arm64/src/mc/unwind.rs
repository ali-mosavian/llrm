//! arm64's `.eh_frame` facts (`AArch64MCTargetDesc.cpp`'s initial frame state,
//! the AArch64 DWARF register numbers: `x0`..`x30` are 0..30, `sp` 31 and
//! `v0`..`v31` 64..95).

use llrm_dwarf::eh_frame::EhTarget;
use llrm_object::debug::FrameRow;

/// The DWARF number of the frame pointer, `x29`.
pub const FP: u16 = 29;
/// The DWARF number of the link register, `x30`.
pub const LR: u16 = 30;
/// The DWARF number of the stack pointer.
pub const SP: u16 = 31;
const V0: u16 = 64;

/// A register's name by its DWARF number.
pub fn register_label(number: u16) -> String {
    match number {
        0..=30 => format!("x{number}"),
        SP => "sp".to_owned(),
        _ => format!("d{}", number - V0),
    }
}

/// The DWARF number of a register by name.
pub fn register(name: &str) -> Option<u64> {
    match name {
        "sp" => Some(u64::from(SP)),
        _ => {
            let (class, index) = name.split_at(1);
            let index: u16 = index.parse().ok()?;
            match class {
                "x" if index <= 30 => Some(u64::from(index)),
                "d" if index <= 31 => Some(u64::from(V0 + index)),
                _ => None,
            }
        }
    }
}

/// At a function's entry the frame address is the stack pointer and the return
/// address is in `x30`.
pub fn eh_target() -> EhTarget<'static> {
    EhTarget {
        address: 8,
        return_column: u64::from(LR),
        return_saved: false,
        code_alignment: 4,
        entry: FrameRow { offset: 0, cfa_register: "sp".into(), cfa_offset: 0, saved: Vec::new() },
        register: &register,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_round_trip_through_their_dwarf_numbers() {
        for number in (0..=SP).chain(V0 + 8..V0 + 16) {
            assert_eq!(register(&register_label(number)), Some(u64::from(number)));
        }
        assert_eq!(register("x31"), None);
    }
}
