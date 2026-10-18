//! The one reading of an `LLRM_*` switch: set to anything but an off word is
//! on.
//!
//! # How it works
//! `set(name)` is true when the variable is in the environment the process
//! started with and its value is not empty, `0`, `false` or `off` (any case).
//! Presence alone was the test, so `LLRM_CHECK_X=0` turned the check on. The
//! environment is read once, as `std::env::var_os` takes the environment lock
//! and scans it on every call. `llrm_support::env_set` and `env_flag!` ask
//! this: no switch has a second parser.
//!
//! # Used by
//! `llrm-mir`'s pass manager checks, and `llrm-support`, which every other
//! crate asks. A leaf with no dependencies: `llrm-mir` may depend on nothing
//! else.
//!
//! # Example
//! `LLRM_VERIFY=0 llrm-c f.c` verifies nothing; `LLRM_VERIFY=1` and a bare
//! `LLRM_VERIFY=yes` do.

use std::ffi::{OsStr, OsString};

/// Whether `value` of a switch means on.
pub fn is_on(value: &OsStr) -> bool {
    let value = value.to_string_lossy();
    !(value.is_empty() || value == "0" || value.eq_ignore_ascii_case("false") || value.eq_ignore_ascii_case("off"))
}

/// Whether the switch `name` is on, from a snapshot of the environment taken at
/// the first ask. The variables are those the process started with; none is
/// set afterwards.
pub fn set(name: &str) -> bool {
    static VARS: std::sync::OnceLock<Vec<(OsString, OsString)>> = std::sync::OnceLock::new();
    VARS.get_or_init(|| std::env::vars_os().collect())
        .iter()
        .any(|(one, value)| one == OsStr::new(name) && is_on(value))
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::is_on;

    /// `LLRM_CHECK_STALE=0` ran the oracle, as `=1` did: presence was the test
    /// (#2279).
    #[test]
    fn test_an_off_word_is_off_and_anything_else_is_on() {
        for off in ["", "0", "false", "FALSE", "off", "Off"] {
            assert!(!is_on(OsStr::new(off)), "{off:?}");
        }
        for on in ["1", "yes", "true", "on", "00", "2"] {
            assert!(is_on(OsStr::new(on)), "{on:?}");
        }
    }
}
