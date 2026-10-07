//! Port of `qbopt/cfront/raise_hir.py`: one procedure's trees as a MirBody.
//!
//! Every C variable is a frame cell and every tree node a fresh value, so the
//! body is in SSA by construction and promotion is left to the passes. The
//! ABI is Borland's medium model.
//!
//! Python's `eval` returns one of a dozen types; that union is `Got`.

use llrm_target::calling::Convention;
use std::collections::BTreeSet;


use llrm_core::abi::runtime;

/// `WIDTHS.get(type_)`.
pub fn widths(type_: &str) -> Option<u32> {
    widths_for(false, type_)
}

/// `widths`, where flat code's `int` and pointers are 4 bytes.
pub fn widths_for(flat: bool, type_: &str) -> Option<u32> {
    if flat && matches!(type_, "TY_INTEGER" | "TY_UNSIGNED" | "TY_BOOLEAN" | "TY_POINTER" | "TY_NEAR_POINTER" | "TY_CODE_PTR" | "TY_NEAR_CODE_PTR") {
        return Some(4);
    }
    Some(match type_ {
        "TY_UINT_1" | "TY_INT_1" => 1,
        "TY_UINT_2" | "TY_INT_2" => 2,
        "TY_UINT_4" | "TY_INT_4" => 4,
        "TY_UINT_8" | "TY_INT_8" => 8,
        "TY_INTEGER" | "TY_UNSIGNED" | "TY_BOOLEAN" => 2,
        "TY_NEAR_POINTER" | "TY_NEAR_CODE_PTR" => 2,
        "TY_LONG_POINTER" | "TY_HUGE_POINTER" | "TY_LONG_CODE_PTR" => 4,
        // A float moves as its bits; only arithmetic and conversion need the x87.
        "TY_SINGLE" => 4,
        "TY_DOUBLE" => 8,
        "TY_LONG_DOUBLE" => 10,
        _ => return None,
    })
}

pub(crate) fn is_float(type_: &str) -> bool {
    matches!(type_, "TY_SINGLE" | "TY_DOUBLE" | "TY_LONG_DOUBLE")
}

/// Operators OW's front end has a node for and Borland's library a routine.
pub(crate) fn library_routine(cg_op: &str) -> Option<&'static str> {
    Some(match cg_op {
        "O_SQRT" => "sqrt",
        "O_COS" => "cos",
        "O_SIN" => "sin",
        "O_TAN" => "tan",
        "O_ACOS" => "acos",
        "O_ASIN" => "asin",
        "O_ATAN" => "atan",
        "O_LOG" => "log",
        "O_LOG10" => "log10",
        "O_EXP" => "exp",
        "O_POW" => "pow",
        "O_ATAN2" => "atan2",
        "O_FMOD" => "fmod",
        _ => return None,
    })
}

/// Borland's pseudo-function laying its constant arguments down as code.
pub(crate) const EMITTED: [&str; 1] = ["__emit__"];

pub(crate) fn signed(type_: &str) -> bool {
    matches!(type_, "TY_INT_1" | "TY_INT_2" | "TY_INT_4" | "TY_INT_8" | "TY_INTEGER")
}

pub(crate) fn far_pointers(type_: &str) -> bool {
    matches!(type_, "TY_LONG_POINTER" | "TY_HUGE_POINTER")
}

pub(crate) fn pointers(type_: &str) -> bool {
    matches!(type_, "TY_POINTER" | "TY_NEAR_POINTER" | "TY_LONG_POINTER" | "TY_HUGE_POINTER")
}

/// C's aliasing classes.
pub(crate) fn classes(type_: &str) -> Option<&'static str> {
    classes_for(false, type_)
}

/// `classes`, where flat code's `int` is a dword and its pointers 4 bytes.
pub(crate) fn classes_for(flat: bool, type_: &str) -> Option<&'static str> {
    if flat && matches!(type_, "TY_INTEGER" | "TY_UNSIGNED") {
        return Some("int4");
    }
    if flat && matches!(type_, "TY_NEAR_POINTER" | "TY_POINTER") {
        return Some("pointer4");
    }
    Some(match type_ {
        "TY_INT_2" | "TY_UINT_2" | "TY_INTEGER" | "TY_UNSIGNED" => "int2",
        "TY_INT_4" | "TY_UINT_4" => "int4",
        "TY_INT_8" | "TY_UINT_8" => "int8",
        "TY_SINGLE" => "float4",
        "TY_DOUBLE" => "float8",
        "TY_LONG_DOUBLE" => "float10",
        "TY_NEAR_POINTER" => "pointer2",
        "TY_LONG_POINTER" | "TY_HUGE_POINTER" => "pointer4",
        _ => return None,
    })
}

/// A call's contract under `convention`, a target's C ABI (`calling.toml`): stack arguments, what it
/// clobbers, what it keeps, and `pushed` bytes its caller or it pops. `Reg` names the 16-bit
/// registers; each stands for its family, so `eax` is `ax`, and the x87 stack is not one.
pub(crate) fn contract(convention: &Convention, name: String, caller_pops: bool, pushed: i64) -> runtime::Contract {
    let family = |register: &str| -> Option<runtime::Reg> {
        let word = if register.len() == 3 && register.starts_with('e') { &register[1..] } else { register };
        runtime::Reg::from_value(word).ok()
    };
    let kept = |kept: &llrm_target::calling::Kept| kept.full.to_ascii_uppercase();
    let results = |class: &str| convention.results.get(class).map(|one| one.join(":").to_ascii_uppercase());
    runtime::Contract {
        name,
        cleanup: Some(if caller_pops { 0 } else { pushed }),
        control: runtime::Control::Returns,
        enters_user_code: false,
        raises_error: false,
        error_handling: false,
        writes: runtime::Memory::Any,
        reads: runtime::Memory::Any,
        clobbers: convention.clobbered.iter().filter_map(|register| family(register)).collect(),
        established: true,
        evidence: format!(
            "{}: stack arguments, result in {} or {}; {} kept",
            convention.name,
            results("1").unwrap_or_default(),
            results("8").unwrap_or_default(),
            convention.preserved.iter().map(kept).collect::<Vec<_>>().join(", ")
        ),
        documented: None,
        inputs: Some(BTreeSet::new()),
        direct_inputs: None,
        clobbers_reached: false,
        caller_cleanup: if caller_pops { pushed } else { 0 },
        i386: true,
        direct_writes: None,
        flags_result: false,
        direct_reads: None,
    }
}

// Addresses the raise holds before any of them is a value.

pub use llrm_core::backend::masm::InlinePart;

#[cfg(test)]
mod tests {
    use super::*;

    /// The clobber lists were `medium_model`'s and `cdecl32`'s, written in Rust beside `calling.toml`: a convention that
    /// clobbers SI and keeps the rest says so, whichever target it is.
    #[test]
    fn a_contract_clobbers_what_the_targets_convention_says() {
        let text = r#"default = "c"
[abi.c]
convention = "c"
[c]
slot_bytes = 2
order = "right-to-left"
cleanup = "caller"
argument_registers = []
return_address_bytes = 2
first_argument_offset = 4
frame = "bp"
stack = "sp"
preserved = ["bp"]
clobbered = ["esi", "st0", "flags"]
entry_state = []
promotion = "slot"
wide_slots = 2
variadic_float = "double"
[c.result]
1 = ["eax"]
8 = ["eax", "edx"]
"#;
        let calling = llrm_target::calling::Calling::parse(&text).unwrap();
        let contract = contract(calling.native(), "f".to_owned(), true, 0);
        assert_eq!(contract.clobbers, BTreeSet::from([runtime::Reg::Si, runtime::Reg::Flags]));
        assert!(runtime::preserves(&contract).contains(&runtime::Reg::Ax));
        assert!(contract.evidence.starts_with("c: stack arguments"), "{}", contract.evidence);
    }

    /// Both real targets: m16 clobbers AX, BX, CX, DX, ES and the flags; m32 AX, CX, DX and the flags (EBX, ESI, EDI, EBP kept).
    #[test]
    fn the_real_targets_clobber_what_they_did() {
        use llrm_target::Target;
        use runtime::Reg::*;
        let clobbers = |target: &dyn Target| contract(target.calling().named(&crate::compile::Profile::of(target).unwrap().convention).unwrap(), String::new(), true, 0).clobbers;
        assert_eq!(clobbers(&llrm_x86_m16::M16), BTreeSet::from([Ax, Bx, Cx, Dx, Es, Flags]));
        assert_eq!(clobbers(&llrm_x86_m32::M32), BTreeSet::from([Ax, Cx, Dx, Flags]));
    }
}
