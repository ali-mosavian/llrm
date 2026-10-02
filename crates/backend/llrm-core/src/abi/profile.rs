//! Port of `qbopt/abi/profile.py`: hash-checked external call profiles.

use crate::abi::runtime::Contract;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Profile {
    pub rules: Vec<Contract>,
    pub fingerprint: String,
}
