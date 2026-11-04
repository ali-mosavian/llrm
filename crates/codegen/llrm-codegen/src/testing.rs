//! Test support: the procedure boundary other crates check their calls against.

pub mod boundary;
#[cfg(test)]
mod boundary_tests;

/// `text` with `@m16-datalayout@`, `@m32-datalayout@` and `@m64-datalayout@`
/// replaced by that target's datalayout string (its `datalayout.toml`, stated
/// once), kept for the life of the test run.
#[cfg(test)]
pub fn target_ir(text: &str) -> &'static str {
    let stated = |toml: &str| {
        toml.lines()
            .find_map(|line| line.strip_prefix("datalayout = \"")?.strip_suffix('"'))
            .expect("datalayout.toml states a datalayout")
            .to_owned()
    };
    Box::leak(
        text.replace("@m16-datalayout@", &stated(llrm_x86_m16::DATALAYOUT_TOML))
            .replace("@m32-datalayout@", &stated(llrm_x86_m32::DATALAYOUT_TOML))
            .replace("@m64-datalayout@", &stated(llrm_x86_m64::DATALAYOUT_TOML))
            .into_boxed_str(),
    )
}

/// `target_ir` for an m16 test, which names only its own placeholder.
#[cfg(test)]
pub fn m16_ir(text: &str) -> &'static str {
    target_ir(text)
}

/// `llrm_mir::parse::module` of `target_ir(text)`.
#[cfg(test)]
pub fn module(text: &str) -> Result<llrm_mir::module::Module, impl std::fmt::Debug> {
    llrm_mir::parse::module(target_ir(text))
}
