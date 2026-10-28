//! Test support: the procedure boundary other crates check their calls against.

pub mod boundary;
#[cfg(test)]
mod boundary_tests;

/// `text` with `@m16-datalayout@` replaced by the m16 target's datalayout
/// string (its `datalayout.toml`, stated once), kept for the life of the test
/// run.
#[cfg(test)]
pub fn m16_ir(text: &str) -> &'static str {
    Box::leak(
        text.replace(
            "@m16-datalayout@",
            &llrm_x86_m16::DATALAYOUT_TOML
                .lines()
                .find_map(|line| line.strip_prefix("datalayout = \"")?.strip_suffix('"'))
                .expect("datalayout.toml states a datalayout"),
        )
        .into_boxed_str(),
    )
}
