//! Rust implementation of the QBasic 1.1 `buildprs` parser-table generator.

pub mod buildprs_actions;
pub mod buildprs_artifacts;
pub mod buildprs_compare;
pub mod buildprs_dispatch;
pub mod buildprs_emit;
pub mod buildprs_encoder;
pub mod buildprs_generator;
pub mod buildprs_grammar;
pub mod buildprs_graph;
pub mod buildprs_integrate;
pub mod buildprs_layout;
pub mod buildprs_lowering;
pub mod buildprs_micro;
pub mod buildprs_optimize;
pub mod buildprs_prstab;
pub mod buildprs_tokens;

pub use buildprs_artifacts::{
    parse_db_bytes, parse_dw_offsets, parse_equates, parse_irw_equates, ArtifactSet,
    ValidatedArtifacts,
};
pub use buildprs_prstab::{
    compare_to_prstab_h, compare_to_prstab_inc, generate_prstab_constants, PrstabConstants,
    StiOffsetInput,
};
