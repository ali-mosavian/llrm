//! How a target names its [`ISD::Target`](super::isd::ISD::Target) codes
//! (`getTargetNodeName`) and maps them to the pin's numbers; installed once by
//! the target, so this crate never names one.

use std::sync::OnceLock;

pub trait TargetNodeNames: Sync + Send {
    /// The node's name, or none for a code the target does not know.
    fn name(
        &self,
        code: u16,
    ) -> Option<&'static str>;

    /// The pin's number of a private `code`, or none for a code that already
    /// is the pin's.
    fn pin_id(
        &self,
        code: u16,
    ) -> Option<u16>;
}

static INSTALLED: OnceLock<&'static dyn TargetNodeNames> = OnceLock::new();

/// The first install wins; a repeat is a no-op.
pub fn install(names: &'static dyn TargetNodeNames) {
    let _ = INSTALLED.set(names);
}

pub fn installed() -> Option<&'static dyn TargetNodeNames> {
    INSTALLED.get().copied()
}

/// `getTargetNodeName`, or `Target#code` when no target knows the code.
pub fn name(code: u16) -> String {
    installed().and_then(|names| names.name(code)).map_or_else(|| format!("Target#{code}"), str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A target-private code printed as `X86ISD#n` from shared code; with no
    /// target installed it must print generically.
    #[test]
    fn test_unknown_code_prints_without_a_target_name() {
        assert_eq!(name(7), "Target#7");
    }
}
