//! The runtime routines compiled code calls, besides the formatters
//! (runtime/nib/*.nbl).

use crate::abi::nib as rt;
use super::*;

/// Name, parameters, and result of each.
pub(super) fn routines() -> Vec<(&'static str, Vec<TypeName>, TypeName)> {
    use TypeName::{Addr, I8, String, U8, U16, Void};
    vec![
        (rt::BUFFER_DROP, vec![String], Void),
        (rt::BUFFER_RESERVE, vec![String, U16, U16], String),
        (rt::TEXT_CONCAT, vec![String, String], String),
        (rt::TEXT_APPEND, vec![String, String], String),
        (rt::BUFFER_GROW, vec![String, U16, U16], String),
        (rt::BUFFER_SHRINK, vec![String, U16], U16),
        (rt::BUFFER_CLONE, vec![String, U16], String),
        (rt::DICT_RESERVE, vec![String, U16], String),
        (rt::TEXT_COMPARE, vec![String, String], I8),
        (rt::PRINT_BEGIN, Vec::new(), Void),
        (rt::PRINT_END, Vec::new(), String),
        (rt::PRINT_FIELD, vec![U8, U8, U8, U8], Void),
        (rt::VIEW_COPY, vec![Addr, U16], String),
        (rt::VIEW_COMPARE, vec![Addr, U16, Addr, U16], I8),
        (rt::ERROR_BOUNDS, Vec::new(), Void),
        (rt::ERROR_SHIFT, Vec::new(), Void),
        (rt::ERROR_CONVERT, Vec::new(), Void),
        (rt::ERROR_KEY, Vec::new(), Void),
    ]
}
