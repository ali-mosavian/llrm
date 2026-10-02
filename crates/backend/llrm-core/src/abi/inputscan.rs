//! Port of `qbopt/abi/inputscan.py`: register inputs and preserved registers
//! discovered from linked OMF definitions. The Python module docstring is the
//! full account.
//!
//! A `frozenset` of entry lanes is `Lanes`, a bit per `root:byte`; nothing
//! iterates one into output.

use std::cell::RefCell;
use std::fmt;

use iced_x86::InstructionInfoFactory;

use crate::frontends::bc::declen::instruction_info_factory;
use crate::objectfile::omf::ValueError;

/// A library routine cannot be decoded into a conservative graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Unrecognized(pub String);

impl fmt::Display for Unrecognized {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Unrecognized {}

/// `Unrecognized` subclasses `ValueError`.
impl From<Unrecognized> for ValueError {
    fn from(error: Unrecognized) -> ValueError {
        ValueError(error.0)
    }
}

thread_local! {
    static INFO: RefCell<InstructionInfoFactory> = RefCell::new(instruction_info_factory());
}
