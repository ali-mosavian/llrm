//! Port of `qbopt/frontend`'s BC object raise.

pub mod addressfacts;
pub mod arrayfacts;
pub mod blocks;
pub mod declen;
pub mod escaped;
pub mod extent;
pub mod fppatches;
pub mod fpstack;
pub mod pairs;
pub mod raising_address_state;
pub mod raising_addresses;
pub mod raising_array_access;
pub mod raising_array_bounds;
pub mod raising_arrays;
pub mod raising_bytes;
pub mod raising_call_memory;
pub mod raising_calls;
pub mod raising_carried;
pub mod raising_conditions;
pub mod raising_control;
pub mod raising_copies;
pub mod raising_defseg;
pub mod raising_dispatch;
pub mod raising_division;
pub mod raising_fields;
pub mod raising_float_calls;
pub mod raising_float_results;
pub mod raising_float_values;
pub mod raising_floats;
pub mod raising_frame;
pub mod raising_literals;
pub mod raising_longs;
pub mod raising_numeric_policy;
pub mod raising_returns;
pub mod raising_words;
pub mod stack;

#[cfg(test)]
mod addends_tests;
