//! Target-independent support code.

pub mod bits;
pub mod codepage;
pub mod debug;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
pub mod diagnostic;
pub mod hash;
pub mod pyjson;
pub mod pypath;
pub mod pyrepr;
pub mod pyset;
mod register;

pub use register::PhysicalRegister;

/// `LLRM_CHECK_CACHES=1`: every identity-cache hit is recomputed and must equal what was cached.
pub fn checking_caches() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("LLRM_CHECK_CACHES").is_some_and(|value| value == "1"))
}
