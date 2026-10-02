//! Port of `qbopt/cfront/raise_hir.py`: one procedure's trees as a MirBody.
//!
//! Every C variable is a frame cell and every tree node a fresh value, so the
//! body is in SSA by construction and promotion is left to the passes. The
//! ABI is Borland's medium model.
//!
//! Python's `eval` returns one of a dozen types; that union is `Got`.

use std::collections::BTreeSet;


use llrm_core::abi::runtime;

/// `WIDTHS.get(type_)`.
pub fn widths(type_: &str) -> Option<u32> {
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

/// A call's contract in Borland's medium model: stack arguments, the
/// result in AX or DX:AX, and `pushed` bytes its caller or it pops.
pub(crate) fn medium_model(name: String, caller_pops: bool, pushed: i64) -> runtime::Contract {
    runtime::Contract {
        name,
        cleanup: Some(if caller_pops { 0 } else { pushed }),
        control: runtime::Control::Returns,
        enters_user_code: false,
        raises_error: false,
        error_handling: false,
        writes: runtime::Memory::Any,
        reads: runtime::Memory::Any,
        clobbers: BTreeSet::from([
            runtime::Reg::Ax,
            runtime::Reg::Bx,
            runtime::Reg::Cx,
            runtime::Reg::Dx,
            runtime::Reg::Es,
            runtime::Reg::Flags,
        ]),
        established: true,
        evidence: "Borland medium model: stack arguments, result in AX or DX:AX; \
                   SI, DI, BP and DS kept as 16-bit registers"
            .to_owned(),
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
