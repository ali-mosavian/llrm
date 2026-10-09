//! The runtime routines compiled code calls, besides the formatters
//! (crates/frontends/llrm-nib/src/runtime/*.nib).

use llrm_core::abi::nib as rt;

use super::*;

/// Name, parameter types, and result of each.
pub(super) fn routines(types: &mut TypeRegistry) -> Vec<(&'static str, Vec<u32>, TypeName)> {
    use TypeName::{I8, String, U8, Void};
    // Lengths, capacities and counts are the target's usize.
    let word = types.word();
    let text = text_view(types);
    vec![
        (rt::BUFFER_DROP, scalars(&[String]), Void),
        (rt::BUFFER_RESERVE, scalars(&[String, word, word]), String),
        (rt::TEXT_CONCAT, scalars(&[String, String]), String),
        (rt::TEXT_APPEND, scalars(&[String, String]), String),
        (rt::BUFFER_GROW, scalars(&[String, word, word]), String),
        (rt::BUFFER_SHRINK, scalars(&[String, word]), word),
        (rt::BUFFER_CLONE, scalars(&[String, word]), String),
        (rt::DICT_RESERVE, scalars(&[String, word]), String),
        (rt::PRINT_BEGIN, Vec::new(), Void),
        (rt::PRINT_END, Vec::new(), String),
        (rt::PRINT_FIELD, scalars(&[U8, U8, U8, U8]), Void),
        (rt::VIEW_COPY, vec![text], String),
        (rt::VIEW_COMPARE, vec![text, text], I8),
        (rt::ERROR_BOUNDS, Vec::new(), Void),
        (rt::ERROR_SHIFT, Vec::new(), Void),
        (rt::ERROR_CONVERT, Vec::new(), Void),
        (rt::ERROR_KEY, Vec::new(), Void),
    ]
}

/// The type a routine takes a `&string` as: the far pointer to its
/// descriptor, as a Nib function takes one.
pub(super) fn text_view(types: &mut TypeRegistry) -> u32 {
    types.slice_pointer(ElementType::Scalar(TypeName::Char), 1)
}

pub(super) fn scalars(names: &[TypeName]) -> Vec<u32> {
    names.iter().copied().map(type_id).collect()
}
