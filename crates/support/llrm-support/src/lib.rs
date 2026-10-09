//! Target-independent support code.

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

pub use register::PhysicalRegister;

/// Whether the environment variable `name` is set, from a snapshot of the environment taken at the first ask. `std::env::var_os` takes
/// the environment lock and scans it on every call: the `LLRM_CHECK_*` switches asked in a loop (once for each instruction's liveness
/// effect) were 2% of a compile of rectwo. The variables are those the process started with; none is set afterwards.
pub fn env_set(name: &str) -> bool {
    static NAMES: std::sync::OnceLock<hash::HashSet<std::ffi::OsString>> = std::sync::OnceLock::new();
    NAMES.get_or_init(|| std::env::vars_os().map(|(name, _)| name).collect()).contains(std::ffi::OsStr::new(name))
}

/// `LLRM_CHECK_CACHES=1`: every identity-cache hit is recomputed and must equal what was cached.
pub fn checking_caches() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("LLRM_CHECK_CACHES").is_some_and(|value| value == "1"))
}

#[cfg(test)]
mod decoder_tests {
    use iced_x86::{Code, Decoder, DecoderOptions};

    /// iced built the decoder tables of VEX, EVEX, XOP and 3DNow! instructions on the first decode, none of which a target emits
    /// or reads: ~4.5 M of the 11.5 M instructions of an empty program's `lir peephole`, whose first decode it was (the 66
    /// programs spend 12.5% of their compile there). The workspace builds iced without them, so they decode as invalid.
    #[test]
    fn test_the_decoder_has_no_tables_for_instructions_no_target_has() {
        for (name, bytes) in [("vex vzeroupper", &[0xc5, 0xf8, 0x77][..]), ("evex vmovdqa32", &[0x62, 0xf1, 0x7d, 0x48, 0x6f, 0xc1]), ("3dnow pfadd", &[0x0f, 0x0f, 0xc1, 0x9e])] {
            let mut decoder = Decoder::with_ip(32, bytes, 0, DecoderOptions::NONE);
            assert_eq!(decoder.decode().code(), Code::INVALID, "{name} has a decoder table");
        }
    }
}
