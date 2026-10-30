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

static PRESET: std::sync::Mutex<Vec<&'static str>> = std::sync::Mutex::new(Vec::new());

/// Turns the switch `name` on as if the process had started with it, for a
/// command-line option that stands for one. Only before the first ask: the
/// snapshot is taken once.
pub fn preset(name: &'static str) {
    PRESET.lock().unwrap().push(name);
}

fn vars() -> &'static [(OsString, OsString)] {
    static VARS: std::sync::OnceLock<Vec<(OsString, OsString)>> = std::sync::OnceLock::new();
    VARS.get_or_init(|| {
        let mut all: Vec<_> = std::env::vars_os().collect();
        all.extend(PRESET.lock().unwrap().iter().map(|name| (OsString::from(name), OsString::from("1"))));
        all
    })
}

/// The value of `name`, from a snapshot of the environment taken at the first
/// ask; for a switch that carries a value (a function name, a path).
pub fn var(name: &str) -> Option<&'static OsStr> {
    vars().iter().find(|(one, _)| one == OsStr::new(name)).map(|(_, value)| value.as_os_str())
}

/// Whether the switch `name` is on, from the same snapshot. The variables are
/// those the process started with; none is set afterwards.
pub fn set(name: &str) -> bool {
    var(name).is_some_and(is_on)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::{is_on, var};

    /// A value is read from the snapshot, and a name not in the environment
    /// is `None`, not an empty value.
    #[test]
    fn test_var_reads_the_snapshot() {
        assert!(var("PATH").is_some());
        assert!(var("LLRM_NO_SUCH_SWITCH").is_none());
    }

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
