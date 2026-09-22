//! Port of `qbopt/frontend/qb`: QB HIR to a BASIC-envelope OMF object.
//!
//! `qbc`, not `qb`, only because `src/frontend/qb` holds the frozen parser
//! fork; the cutover moves this to `src/frontends/qb`.

pub mod abi;
pub mod driver;
pub mod inline_x87;
pub mod stage_text;
