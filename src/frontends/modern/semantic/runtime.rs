//! The runtime routines compiled code calls, besides the formatters
//! (runtime/modern/*.c).

use super::*;

/// Name, parameters, and result of each.
pub(super) fn routines() -> Vec<(&'static str, Vec<TypeName>, TypeName)> {
    use TypeName::{Addr, I8, String, U8, U16, Void};
    vec![
        ("_rt_drop", vec![String], Void),
        ("_rt_reserve", vec![String, U16, U16], String),
        ("_rt_concat", vec![String, String], String),
        ("_rt_append", vec![String, String], String),
        ("_rt_grow", vec![String, U16, U16], String),
        ("_rt_shrink", vec![String, U16], U16),
        ("_rt_clone", vec![String, U16], String),
        ("_rt_dict_reserve", vec![String, U16], String),
        ("_rt_compare", vec![String, String], I8),
        ("_rt_begin", Vec::new(), Void),
        ("_rt_end", Vec::new(), String),
        ("_rt_field", vec![U8, U8, U8, U8], Void),
        ("_rt_view_copy", vec![Addr, U16], String),
        ("_rt_view_compare", vec![Addr, U16, Addr, U16], I8),
        ("_rt_panic_bounds", Vec::new(), Void),
        ("_rt_panic_shift", Vec::new(), Void),
        ("_rt_panic_convert", Vec::new(), Void),
        ("_rt_panic_key", Vec::new(), Void),
    ]
}
