//! The arm64 target. Its descriptions are the files beside this one.

pub mod cc;
pub mod cfi;
pub mod cmpzero;
pub mod combine;
pub mod condbrtuning;
pub mod copyprop;
pub mod dag;
pub mod data;
pub mod debug;
pub mod description;
pub mod divconst;
pub mod driver;
pub mod emit;
pub mod expand_pseudo;
pub mod expand_imm;
pub mod facts;
pub mod flagcse;
pub mod frame;
pub mod ipra;
pub mod jumptable;
pub mod layout;
pub mod lir_place;
pub mod ldst_update;
pub mod licm;
pub mod lir_body;
pub mod lir_view;
pub mod machine;
pub mod mc;
pub mod memcluster;
pub mod mfunc;
pub mod pairs;
pub mod postrasink;
pub mod prepass;
pub mod regalloc;
pub mod schedmodel;
pub mod shrinkwrap;
pub mod sibcall;
pub mod sinkfold;
pub mod taildup;
pub mod trial;
pub mod tti;
mod tti_sink;
#[cfg(test)]
mod tti_tests;
mod tti_vector;
pub mod vector_forms;

use llrm_lir::registers::RegId;

/// The register ids, numbered by `registers.regs`'s lines: `reg::X0`.
pub mod reg {
    use super::RegId;
    include!(concat!(env!("OUT_DIR"), "/register_ids.rs"));
}

/// Instruction effect rows, generated from `instructions/opcodes.instr`.
mod opcode_rows {
    include!(concat!(env!("OUT_DIR"), "/opcode_rows.rs"));
}
use opcode_rows::rows;

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

    /// A tuple is its members' registers: `v0_v1` collides with `v1` and `d1`
    /// (the old root-and-lane test read it as its own root and let `ld2` clobber
    /// a live `v1`), not with `v2`; the wrap past `v31` is a tuple too.
    #[test]
    fn a_tuple_overlaps_what_its_members_do() {
        let info = info();
        let pair = info.by_name("v0_v1").expect("tuple row");
        assert!(info.is_composite(pair) && !info.is_composite(reg::V1));
        assert!(info.overlaps(pair, reg::V1) && info.overlaps(reg::D0, pair));
        assert!(!info.overlaps(pair, reg::V2));
        assert!(info.overlaps(pair, info.by_name("v1_v2").unwrap()));
        assert!(!info.overlaps(pair, info.by_name("v2_v3").unwrap()));
        assert_eq!(info.subreg(pair, 1), Some(reg::V1));
        assert_eq!(info.subreg(pair, 2), None);
        let wrap = info.by_name("v31_v0_v1").expect("wraps");
        assert_eq!(info.subreg(wrap, 1), Some(reg::V0));
        assert!(info.overlaps(wrap, reg::V31) && info.overlaps(wrap, pair));
        assert!(info.in_class(pair, class::TUPLE) && !info.in_class(pair, class::FPR));
    }
}
