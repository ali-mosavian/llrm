//! The arm64 target. Its descriptions are the files beside this one.

pub mod cc;
pub mod data;
pub mod divconst;
pub mod driver;
pub mod emit;
pub mod expand_imm;
pub mod flagcse;
pub mod frame;
pub mod isel;
pub mod layout;
pub mod machine;
pub mod mc;
pub mod mfunc;
pub mod pairs;
pub mod prepass;
pub mod regalloc;
pub mod shrinkwrap;
pub mod sibcall;
pub mod storecluster;
pub mod tti;
#[cfg(test)]
mod tti_tests;

use llrm_lir::registers::{RegId, Row};

/// The register ids, numbered by `registers.regs`'s lines: `reg::X0`.
pub mod reg {
    use super::RegId;
    include!(concat!(env!("OUT_DIR"), "/register_ids.rs"));
}

/// Instruction effect rows: none until arm64's forms are described.
fn rows(
    _name: &str,
    _dests: usize,
    _sources: usize,
) -> &'static [Row] {
    &[]
}

include!(concat!(env!("OUT_DIR"), "/register_info.rs"));

#[cfg(test)]
mod tests {
    use llrm_lir::registers::{Info, class};

    use super::{REGISTER_INFO, reg};

    fn info() -> &'static Info {
        &REGISTER_INFO
    }

    /// An arm64 id is no iced register: the file must answer for it, not iced.
    #[test]
    fn the_file_answers_for_arm64_registers() {
        let info = info();
        assert_eq!(info.name(reg::X0), Some("x0"));
        assert_eq!(info.bytes(reg::W0), Some(4));
        assert_eq!(info.root(reg::W5), reg::X5);
        assert_eq!(info.view(reg::X7, 32), Some(reg::W7));
        assert_eq!(info.view(reg::V3, 64), Some(reg::D3));
        assert_eq!(info.frame, reg::X29);
        assert_eq!(info.stack, reg::SP);
        assert!(info.in_class(reg::D0, class::FPR));
        assert!(info.in_class(reg::XZR, class::ZERO));
    }
}
