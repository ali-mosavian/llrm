//! The pin's opcode names of the vector compares and bit selects, read by the
//! DAG's machine nodes (`dag::emit`) and the LIR bridge (`lir_body`).
//!
//! `CMGTv4i32`, `CMLTv8i8rz`, `FCMGEv2f32`, `FCMLEv4i32rz` (the pin names a
//! float compare against zero by its integer lanes), `BSPv16i8`, `BSLv8i8`.

use std::sync::LazyLock;

use rustc_hash::FxHashMap as HashMap;

use crate::mc::inst::{Arrangement, BitSelect, VectorCmp, VectorCmpZero};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VectorForm {
    Compare { op: VectorCmp, lanes: Arrangement, quad: bool },
    CompareZero { op: VectorCmpZero, lanes: Arrangement, quad: bool },
    BitSelect { op: BitSelect, quad: bool },
    Bsp { quad: bool },
}

const SHAPES: [(Arrangement, bool); 7] = [
    (Arrangement::B16, true),
    (Arrangement::B16, false),
    (Arrangement::H8, true),
    (Arrangement::H8, false),
    (Arrangement::S4, true),
    (Arrangement::S4, false),
    (Arrangement::D2, true),
];

/// `16i8`, `4i16`, `2i32`: the integer lanes as the pin spells them.
fn int_lanes(
    lanes: Arrangement,
    quad: bool,
) -> &'static str {
    match (lanes, quad) {
        (Arrangement::B16, true) => "16i8",
        (Arrangement::B16, false) => "8i8",
        (Arrangement::H8, true) => "8i16",
        (Arrangement::H8, false) => "4i16",
        (Arrangement::S4, true) => "4i32",
        (Arrangement::S4, false) => "2i32",
        (Arrangement::D2, _) => "2i64",
    }
}

/// `4f32`, `2f32`, `2f64`.
fn float_lanes(
    lanes: Arrangement,
    quad: bool,
) -> Option<&'static str> {
    match (lanes, quad) {
        (Arrangement::S4, true) => Some("4f32"),
        (Arrangement::S4, false) => Some("2f32"),
        (Arrangement::D2, true) => Some("2f64"),
        _ => None,
    }
}

/// The pin's name of `form`, if there is one.
pub fn name_of(form: VectorForm) -> Option<String> {
    Some(match form {
        VectorForm::Compare { op, lanes, quad } => {
            let lane = if op.is_float() { float_lanes(lanes, quad)? } else { int_lanes(lanes, quad) };
            format!("{}v{lane}", op.mnemonic().to_uppercase())
        }
        VectorForm::CompareZero { op, lanes, quad } => {
            if op.is_float() {
                float_lanes(lanes, quad)?;
            }
            format!("{}v{}rz", op.mnemonic().to_uppercase(), int_lanes(lanes, quad))
        }
        VectorForm::BitSelect { op, quad } => {
            format!("{}v{}", op.mnemonic().to_uppercase(), if quad { "16i8" } else { "8i8" })
        }
        VectorForm::Bsp { quad } => format!("BSPv{}", if quad { "16i8" } else { "8i8" }),
    })
}

static FORMS: LazyLock<HashMap<String, VectorForm>> = LazyLock::new(|| {
    let mut forms = HashMap::default();
    for (lanes, quad) in SHAPES {
        for op in VectorCmp::ALL {
            let form = VectorForm::Compare { op, lanes, quad };
            forms.extend(name_of(form).map(|name| (name, form)));
        }
        for op in VectorCmpZero::ALL {
            let form = VectorForm::CompareZero { op, lanes, quad };
            forms.extend(name_of(form).map(|name| (name, form)));
        }
    }
    for quad in [true, false] {
        for op in [BitSelect::Bsl, BitSelect::Bit, BitSelect::Bif] {
            let form = VectorForm::BitSelect { op, quad };
            forms.extend(name_of(form).map(|name| (name, form)));
        }
        let form = VectorForm::Bsp { quad };
        forms.extend(name_of(form).map(|name| (name, form)));
    }
    forms
});

/// The form a pin opcode name is.
pub fn parse(name: &str) -> Option<VectorForm> {
    FORMS.get(name).copied()
}

/// Every form that has a name, for the rows of `opcodes.instr` to be checked
/// against.
pub fn all() -> impl Iterator<Item = (String, VectorForm)> {
    FORMS.iter().map(|(name, form)| (name.clone(), *form))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A name is read back as the form it was made from.
    #[test]
    fn every_name_parses_to_its_form() {
        for (name, form) in all() {
            assert_eq!(parse(&name), Some(form), "{name}");
            assert_eq!(name_of(form).as_deref(), Some(name.as_str()));
        }
        assert_eq!(
            parse("CMLTv2i32rz"),
            Some(VectorForm::CompareZero { op: VectorCmpZero::Lt, lanes: Arrangement::S4, quad: false })
        );
        assert_eq!(
            parse("FCMGEv2f64"),
            Some(VectorForm::Compare { op: VectorCmp::FGe, lanes: Arrangement::D2, quad: true })
        );
    }
}
