//! How this build was made, for the tools that count instructions: [`MARK`] is
//! in the binary, and `llrm-c --build-info` prints it.
//!
//! # How it works
//! `build.rs` sets `LLRM_BUILD_KIND` to the Cargo profile and the incremental
//! flag. A count from an incremental build moves by half a point an edit
//! (inlining of unrelated code), so `tools/llrmbin.py` refuses to measure with
//! a binary that is not the `dist` profile built without it.

/// `llrm-build:profile=<name>;incremental=<0|1>`: the Cargo profile (`dist`,
/// `release`, ...) and whether it was built incrementally.
pub const MARK: &str = concat!("llrm-build:", env!("LLRM_BUILD_KIND"));

#[cfg(test)]
mod tests {
    #[test]
    fn test_the_mark_names_the_profile_and_whether_the_build_was_incremental() {
        let fields = super::MARK
            .strip_prefix("llrm-build:profile=")
            .expect("the mark")
            .split(";incremental=")
            .collect::<Vec<_>>();
        assert!(fields.len() == 2 && !fields[0].is_empty() && matches!(fields[1], "0" | "1"), "{}", super::MARK);
    }
}
