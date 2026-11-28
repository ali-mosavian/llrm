//! What a driver flag asks of the mid-end past the level, as `llrm-cc` passes
//! it on to a backend's `llc`: one parser for every backend, so none refuses a
//! flag another takes.

use llrm_mir::program::Program;
use llrm_transforms::pipeline::Options as MidEnd;

/// What a driver flag asks of the mid-end past the level: gcc's `-f` pass
/// flags (`-fno-inline-functions`, `-fno-unroll-loops`...) and
/// `-fno-inline=NAME`, as `llrm-cc` passes them on.
#[derive(Clone, Debug, Default)]
pub struct Policy {
    passes: Vec<String>,
    no_inline: Vec<String>,
}

impl Policy {
    /// An empty policy, usable in a `static`.
    pub const fn new() -> Self {
        Self { passes: Vec::new(), no_inline: Vec::new() }
    }

    /// Takes `flag` when it is one of these; false leaves it to the caller.
    /// `-fexcess-precision` and `-ffloat-store` are taken and change nothing:
    /// a backend that rounds every float operation to its type (software float,
    /// SSE, NEON, wasm) has no wider register to keep a value in. The x87
    /// backends read them themselves and do not use this.
    pub fn take(
        &mut self,
        flag: &str,
    ) -> bool {
        if let Some(name) = flag.strip_prefix("-fno-inline=") {
            self.no_inline.push(name.to_owned());
        } else if matches!(
            flag,
            "-fexcess-precision=standard" | "-fexcess-precision=fast" | "-ffloat-store" | "-fno-float-store"
        ) {
        } else if super::flags::apply_pass_flag(&mut MidEnd::default(), flag) {
            self.passes.push(flag.to_owned());
        } else {
            return false;
        }
        true
    }

    pub fn apply(
        &self,
        options: &mut MidEnd,
    ) {
        for flag in &self.passes {
            super::flags::apply_pass_flag(options, flag);
        }
    }

    /// Each function of `program` that `-fno-inline=` names marked `noinline`;
    /// a name that no function defines is an error, not a flag that did
    /// nothing.
    pub fn mark(
        &self,
        program: &mut Program,
    ) -> Result<(), String> {
        for name in &self.no_inline {
            let mut found = false;
            for global in program.modules.iter_mut().flat_map(|module| &mut module.globals) {
                if global.name.as_deref() != Some(name.as_str()) {
                    continue;
                }
                if let llrm_mir::GlobalKind::Function(function) = &mut global.kind
                    && !function.is_declaration()
                {
                    function.attrs.push(llrm_mir::facts::Fact::Inline(llrm_mir::facts::Inlining::Never).carrier());
                    found = true;
                }
            }
            if !found {
                return Err(format!("-fno-inline={name}: the program defines no function of that name"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Policy;

    /// What `llrm-cc` hands to an `llc` that clang does not take, and what
    /// stays the caller's. A backend's `llc` that calls `Policy::take`
    /// accepts the first column: arm64 and wasm refused `-fexcess-precision=`
    /// and `-fno-inline=` as unrecognized arguments.
    #[test]
    fn the_flags_the_driver_passes_on_are_taken_and_others_are_left_to_the_caller() {
        let passed_on = [
            "-fexcess-precision=standard",
            "-fexcess-precision=fast",
            "-ffloat-store",
            "-fno-float-store",
            "-fno-inline=sum5",
            "-fno-unroll-loops",
            "-funroll-loops",
        ];
        let not_ours = ["-fsanitize=bounds", "-fnot-a-flag", "-O2", "-S", "--isel=dag"];
        let mut policy = Policy::default();
        for flag in passed_on {
            assert!(policy.take(flag), "{flag}");
        }
        for flag in not_ours {
            assert!(!policy.take(flag), "{flag}");
        }
    }
}
