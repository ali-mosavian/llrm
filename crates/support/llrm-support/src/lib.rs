//! Target-independent support code.

pub mod alloc_count;
pub mod bits;
pub mod codepage;
pub mod debug;
pub mod diagnostic;
pub mod graph;
pub mod hash;
pub mod leaf;
pub mod pyjson;
pub mod pypath;
pub mod pyrepr;
pub mod pyset;
mod register;
pub mod version;

pub use register::PhysicalRegister;

/// Whether the switch `name` is on: set to anything but an off word (empty,
/// `0`, `false`, `off`), from a snapshot of the environment taken at the first
/// ask. `std::env::var_os` takes the environment lock and scans it on every
/// call: the `LLRM_CHECK_*` switches asked in a loop (once for each
/// instruction's liveness effect) were 2% of a compile of rectwo. The
/// variables are those the process started with; none is set afterwards. The
/// parser is `llrm_env`'s, the only one.
pub fn env_set(name: &str) -> bool {
    llrm_env::set(name)
}

/// The value of the variable `name`, from the same snapshot as `env_set`.
pub fn env_var(name: &str) -> Option<&'static std::ffi::OsStr> {
    llrm_env::var(name)
}

/// `env_set` of a literal name, asked once at the call site that names it:
/// `env_set` hashes the name at every call, and a check on a step's path is
/// asked thousands of times.
#[macro_export]
macro_rules! env_flag {
    ($name:literal) => {{
        static ON: ::std::sync::OnceLock<bool> = ::std::sync::OnceLock::new();
        *ON.get_or_init(|| {
            let on = $crate::env_set($name);
            if on {
                $crate::note_reached($name);
            }
            on
        })
    }};
}

static REACHED: std::sync::Mutex<Vec<(&'static str, u32)>> = std::sync::Mutex::new(Vec::new());

/// A call site of `env_flag!` asked `name` and found it on: of the
/// `LLRM_CHECK_*` switches, an oracle whose hook the compile reached.
/// `LLRM_DEBUG=checks` prints, per oracle, how many call sites did (`[checks]
/// REPLAY 4`): an oracle that reports none did not run, so a sweep that finds
/// nothing proves nothing (tools/oracle_sweep.py, without perf counters).
pub fn note_reached(name: &'static str) {
    if let Some(oracle) = name.strip_prefix("LLRM_CHECK_") {
        let mut reached = REACHED.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        match reached.iter_mut().find(|row| row.0 == oracle) {
            Some(row) => row.1 += 1,
            None => reached.push((oracle, 1)),
        }
    }
}

/// The oracles reached so far and the call sites of each, by name.
pub fn reached_checks() -> Vec<(&'static str, u32)> {
    let mut rows = REACHED.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone();
    rows.sort();
    rows
}

/// `LLRM_CHECK_CACHES` on: every identity-cache hit is recomputed and must
/// equal what was cached.
pub fn checking_caches() -> bool {
    env_flag!("LLRM_CHECK_CACHES")
}

#[cfg(test)]
mod decoder_tests {
    use iced_x86::{Code, Decoder, DecoderOptions};

    /// iced built the decoder tables of VEX, EVEX, XOP and 3DNow! instructions
    /// on the first decode, none of which a target emits or reads: ~4.5 M
    /// of the 11.5 M instructions of an empty program's `lir peephole`, whose
    /// first decode it was (the 66 programs spend 12.5% of their compile
    /// there). The workspace builds iced without them, so they decode as
    /// invalid.
    #[test]
    fn test_the_decoder_has_no_tables_for_instructions_no_target_has() {
        for (name, bytes) in [
            ("vex vzeroupper", &[0xc5, 0xf8, 0x77][..]),
            ("evex vmovdqa32", &[0x62, 0xf1, 0x7d, 0x48, 0x6f, 0xc1]),
            ("3dnow pfadd", &[0x0f, 0x0f, 0xc1, 0x9e]),
        ] {
            let mut decoder = Decoder::with_ip(32, bytes, 0, DecoderOptions::NONE);
            assert_eq!(decoder.decode().code(), Code::INVALID, "{name} has a decoder table");
        }
    }
}

#[cfg(test)]
mod reached_tests {
    use super::{note_reached, reached_checks};

    /// An oracle whose hook the compile reached is counted by its name, once
    /// per call site; a switch that is not a check is not.
    #[test]
    fn test_a_reached_check_is_counted_by_oracle_and_other_switches_are_not() {
        note_reached("LLRM_CHECK_ZZ_TEST");
        note_reached("LLRM_CHECK_ZZ_TEST");
        note_reached("LLRM_NOT_A_CHECK");
        let rows = reached_checks();
        assert!(rows.contains(&("ZZ_TEST", 2)), "{rows:?}");
        assert!(!rows.iter().any(|row| row.0.contains("NOT_A_CHECK")), "{rows:?}");
    }
}
